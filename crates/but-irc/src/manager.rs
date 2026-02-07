//! IRC connection manager for handling multiple connections.

use crate::client::{IrcClient, IrcConfig};
use crate::error::{IrcError, Result};
use crate::message::IrcEvent;
use crate::message_store::{ChannelInfo, MessageStore, Reaction, StoredMessage};
use crate::state::ConnectionState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc, watch};
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};

/// A unique identifier for an IRC connection.
pub type ConnectionId = String;

/// Manages multiple IRC connections.
///
/// Each connection is identified by a unique ID and can be controlled
/// independently. This allows for separate bot and user connections.
/// Callback type for spawning an event forwarder after reconnection.
///
/// Set via [`IrcManager::set_event_forwarder_spawner`] by the lifecycle layer.
/// Called with `(manager, connection_id)` when a reconnect creates a new event channel.
type ForwarderSpawner = Arc<dyn Fn(IrcManager, String) + Send + Sync>;

#[derive(Clone)]
pub struct IrcManager {
    connections: Arc<RwLock<HashMap<ConnectionId, ManagedConnection>>>,
    store: Arc<RwLock<MessageStore>>,
    /// Channels that should be automatically (re-)joined when a connection becomes ready.
    auto_join_channels: Arc<RwLock<HashMap<ConnectionId, HashSet<String>>>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    /// Optional callback to spawn a full event forwarder after reconnection.
    forwarder_spawner: Arc<RwLock<Option<ForwarderSpawner>>>,
}

impl Default for IrcManager {
    fn default() -> Self {
        Self::new()
    }
}

struct ManagedConnection {
    client: Option<IrcClient>,
    watcher_spawned: bool,
}

impl ManagedConnection {
    /// Take the client out for reconnection. Returns `None` if already taken.
    fn take_client(&mut self) -> Option<IrcClient> {
        self.client.take()
    }

    /// Restore a client after reconnection.
    fn restore_client(&mut self, client: IrcClient) {
        self.client = Some(client);
    }

    /// Get a reference to the client, if present.
    fn client(&self) -> Result<&IrcClient> {
        self.client
            .as_ref()
            .ok_or_else(|| IrcError::other("Connection is currently reconnecting".to_string()))
    }

    /// Get a mutable reference to the client, if present.
    fn client_mut(&mut self) -> Result<&mut IrcClient> {
        self.client
            .as_mut()
            .ok_or_else(|| IrcError::other("Connection is currently reconnecting".to_string()))
    }
}

impl IrcManager {
    /// Create a new IRC connection manager.
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            store: Arc::new(RwLock::new(MessageStore::new())),
            auto_join_channels: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            shutdown_rx,
            forwarder_spawner: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a callback that spawns a full event forwarder for a connection.
    ///
    /// Called by the lifecycle layer so that the reconnection watcher can
    /// re-establish event processing after a channel recreation.
    pub async fn set_event_forwarder_spawner(
        &self,
        spawner: impl Fn(IrcManager, String) + Send + Sync + 'static,
    ) {
        *self.forwarder_spawner.write().await = Some(Arc::new(spawner));
    }

    /// Create a new connection with the given ID and configuration.
    ///
    /// The connection is created but not connected. Call `connect` to start it.
    #[instrument(skip(self, config), fields(connection_id = %id))]
    pub async fn create(&self, id: ConnectionId, config: IrcConfig) -> Result<()> {
        let mut connections = self.connections.write().await;

        if connections.contains_key(&id) {
            return Err(IrcError::other(format!(
                "Connection with id '{}' already exists",
                id
            )));
        }

        let client = IrcClient::new(config, self.shutdown_rx.clone());
        connections.insert(
            id.clone(),
            ManagedConnection {
                client: Some(client),
                watcher_spawned: false,
            },
        );

        info!("Created IRC connection: {}", id);
        Ok(())
    }

    /// Connect an existing connection by ID.
    ///
    /// The connection is temporarily removed from the pool during the network
    /// handshake so that the `connections` lock is **not** held while waiting
    /// for TCP/TLS/IRC negotiation.  This prevents other `IrcManager` methods
    /// (send_message, join, state, …) from stalling on the lock.
    #[instrument(skip(self), fields(connection_id = %id))]
    pub async fn connect(&self, id: &str) -> Result<()> {
        // Take the client out of the ManagedConnection so we can release the
        // write-lock during the (slow) TLS handshake.  The ManagedConnection
        // *stays* in the map so that `exists()` still returns true and
        // `create_and_connect` won't race to create a duplicate.
        let mut client = {
            let mut connections = self.connections.write().await;
            let conn = connections
                .get_mut(id)
                .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;
            conn.take_client().ok_or_else(|| {
                IrcError::other(format!("Connection '{}' is already connecting", id))
            })?
        };
        // Write lock is released here; the entry is still in the map (with client = None).

        let result = client.connect().await;

        // Put the client back regardless of success or failure.
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.get_mut(id) {
                conn.restore_client(client);
            }
            // If the entry was removed while we were connecting (e.g. shutdown),
            // we just drop the client.
        }

        result?;
        info!("Connected IRC connection: {}", id);
        Ok(())
    }

    /// Create and immediately connect a new connection.
    ///
    /// This method is idempotent - if a connection with the given ID already exists
    /// and is active (connecting, negotiating, or ready), it will reuse that connection.
    /// If the connection is disconnected or in error state, it will reconnect.
    ///
    /// Automatic reconnection will be enabled with exponential backoff.
    #[instrument(skip(self, config), fields(connection_id = %id))]
    pub async fn create_and_connect(&self, id: ConnectionId, config: IrcConfig) -> Result<()> {
        // Log a backtrace-like breadcrumb so we can trace who's calling us.
        info!(
            "create_and_connect called for '{}' (nick: {})",
            id, config.nick
        );

        // If the connection already exists in any state, leave it alone.
        // The reconnection watcher handles transient disconnects; destroying
        // and recreating the connection causes nick collisions on the server.
        if self.exists(&id).await {
            match self.state(&id).await {
                Ok(state) => {
                    info!(
                        "Connection '{}' already exists (state: {}), skipping create",
                        id, state
                    );
                    return Ok(());
                }
                Err(_) => {
                    // Error getting state (e.g. client is None during reconnect).
                    // Leave it alone — the reconnection watcher will handle it.
                    info!(
                        "Connection '{}' exists but state unavailable (likely reconnecting), skipping create",
                        id
                    );
                    return Ok(());
                }
            }
        }

        self.create(id.clone(), config).await?;

        // Always spawn the reconnection watcher, even if the initial connect fails.
        // This ensures recovery from transient network errors at startup.
        self.ensure_reconnection_watcher(&id).await;

        self.connect(&id).await?;

        Ok(())
    }

    /// Ensure a reconnection watcher is running for the connection.
    /// Only spawns if one hasn't been spawned already.
    async fn ensure_reconnection_watcher(&self, id: &str) {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.get_mut(id) {
            if conn.watcher_spawned {
                debug!("Reconnection watcher for '{}' already running", id);
                return;
            }
            conn.watcher_spawned = true;
        } else {
            return;
        }

        drop(connections); // Release lock before spawning
        self.spawn_reconnection_watcher(id.to_string());
    }

    /// Spawn a background task that monitors the connection and reconnects if it drops.
    fn spawn_reconnection_watcher(&self, id: ConnectionId) {
        let manager = self.clone();
        let mut shutdown = self.shutdown_rx.clone();

        tokio::spawn(async move {
            let mut backoff_secs = 1u64;
            const MAX_BACKOFF: u64 = 60;
            /// If a connection stays in Connecting/Negotiating for this long, consider it stuck.
            const STUCK_TIMEOUT_SECS: u64 = 30;
            let mut connecting_since: Option<tokio::time::Instant> = None;

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    _ = shutdown.changed() => {
                        debug!("Reconnection watcher for '{}' stopping: shutdown", id);
                        break;
                    }
                }

                // Check if connection still exists
                if !manager.exists(&id).await {
                    debug!(
                        "Reconnection watcher for '{}' stopping: connection removed",
                        id
                    );
                    break;
                }

                // Check connection state
                match manager.state(&id).await {
                    Ok(state) => {
                        let needs_reconnect = match state {
                            ConnectionState::Disconnected | ConnectionState::Error => {
                                connecting_since = None;
                                true
                            }
                            ConnectionState::Connecting | ConnectionState::Negotiating => {
                                // Track how long we've been in this state
                                let since =
                                    connecting_since.get_or_insert_with(tokio::time::Instant::now);
                                if since.elapsed().as_secs() >= STUCK_TIMEOUT_SECS {
                                    warn!(
                                        connection_id = %id,
                                        state = %state,
                                        elapsed_secs = since.elapsed().as_secs(),
                                        "Connection stuck in {} state, forcing reconnect",
                                        state,
                                    );
                                    connecting_since = None;
                                    // Force state to Error so connect() won't bail with "already active"
                                    manager.set_state(&id, ConnectionState::Error).await;
                                    true
                                } else {
                                    false
                                }
                            }
                            ConnectionState::Ready => {
                                connecting_since = None;
                                backoff_secs = 1;
                                false
                            }
                            _ => false,
                        };

                        if needs_reconnect {
                            info!(
                                connection_id = %id,
                                state = %state,
                                backoff_secs,
                                "Reconnection watcher: connection needs reconnect",
                            );

                            // Set state to Reconnecting so the frontend can show a spinner
                            manager.set_state(&id, ConnectionState::Reconnecting).await;

                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                                _ = shutdown.changed() => {
                                    debug!("Reconnection watcher for '{}' stopping: shutdown", id);
                                    break;
                                }
                            }

                            // Try to reconnect
                            if let Err(e) = manager.connect(&id).await {
                                error!(connection_id = %id, error = %e, backoff_secs, "Reconnect failed");
                                // Increase backoff exponentially
                                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF);
                            } else {
                                info!(connection_id = %id, "Reconnected successfully");

                                // If the event channel was recreated (because the
                                // old receiver was dropped), spawn a full event
                                // forwarder so messages, reactions, and history
                                // are all processed — not just auto-join.
                                if let Some(spawner) =
                                    manager.forwarder_spawner.read().await.as_ref()
                                {
                                    info!(connection_id = %id, "Spawning full event forwarder after reconnect");
                                    spawner(manager.clone(), id.clone());
                                } else {
                                    warn!(connection_id = %id, "No event forwarder spawner registered; events after reconnect will be lost");
                                }

                                // Reset backoff on successful connection
                                backoff_secs = 1;
                            }
                        }
                    }
                    Err(_) => {
                        // Connection was removed, stop watcher
                        debug!(
                            "Reconnection watcher for '{}' stopping: connection not found",
                            id
                        );
                        break;
                    }
                }
            }
        });
    }

    /// Disconnect a connection by ID.
    #[instrument(skip(self), fields(connection_id = %id))]
    pub async fn disconnect(&self, id: &str, reason: &str) -> Result<()> {
        let mut connections = self.connections.write().await;

        let conn = connections
            .get_mut(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client_mut()?.disconnect(reason)?;
        info!(reason = reason, "Disconnected IRC connection: {}", id);
        Ok(())
    }

    /// Remove a connection entirely.
    #[instrument(skip(self), fields(connection_id = %id))]
    pub async fn remove(&self, id: &str, reason: &str) -> Result<()> {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.remove(id) {
            if let Some(mut client) = conn.client {
                let _ = client.disconnect(reason);
            }
            // Clean up stored messages/channels for this connection
            self.store.write().await.remove_connection(id);
            info!(reason = reason, "Removed IRC connection: {}", id);
            Ok(())
        } else {
            Err(IrcError::other(format!("Connection '{}' not found", id)))
        }
    }

    /// Get the state of a connection.
    pub async fn state(&self, id: &str) -> Result<ConnectionState> {
        let client_state = {
            let connections = self.connections.read().await;
            let conn = connections
                .get(id)
                .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;
            conn.client()?.state_handle()
        };
        // Lock on `connections` is dropped before awaiting the client state lock
        Ok(*client_state.read().await)
    }

    /// Set the state of a connection (used by the reconnection watcher).
    async fn set_state(&self, id: &str, new_state: ConnectionState) {
        let connections = self.connections.read().await;
        let Some(conn) = connections.get(id) else {
            return;
        };
        let Ok(client) = conn.client() else { return };
        client.set_state(new_state).await;
    }

    /// Check if a connection is ready.
    pub async fn is_ready(&self, id: &str) -> Result<bool> {
        let client_state = {
            let connections = self.connections.read().await;
            let conn = connections
                .get(id)
                .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;
            conn.client()?.state_handle()
        };
        // Lock on `connections` is dropped before awaiting the client state lock
        Ok(*client_state.read().await == ConnectionState::Ready)
    }

    /// Wait for a connection to be ready.
    ///
    /// This polls the connection state until it becomes Ready, or times out.
    /// Default timeout is 30 seconds.
    pub async fn wait_for_ready(&self, id: &str) -> Result<()> {
        self.wait_for_ready_with_timeout(id, Duration::from_secs(30))
            .await
    }

    /// Wait for a connection to be ready with a custom timeout.
    pub async fn wait_for_ready_with_timeout(&self, id: &str, max_wait: Duration) -> Result<()> {
        let poll_interval = Duration::from_millis(100);

        timeout(max_wait, async {
            loop {
                let state = self.state(id).await?;
                match state {
                    ConnectionState::Ready => return Ok(()),
                    ConnectionState::Error | ConnectionState::Disconnected => {
                        return Err(IrcError::connection(format!(
                            "Connection '{}' is in {} state",
                            id, state
                        )));
                    }
                    _ => {
                        tokio::time::sleep(poll_interval).await;
                    }
                }
            }
        })
        .await
        .map_err(|_| {
            IrcError::connection(format!(
                "Timeout waiting for connection '{}' to be ready",
                id
            ))
        })?
    }

    /// Join a channel on a specific connection.
    ///
    /// History replay is handled via the IRCv3 CHATHISTORY extension (supported
    /// by Ergo and other modern IRCds). After joining, the event forwarder
    /// requests recent messages with `CHATHISTORY LATEST`.
    pub async fn join(&self, id: &str, channel: &str) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?.join(channel)?;
        debug!("Connection '{}' joined channel: {}", id, channel);

        Ok(())
    }

    /// Part (leave) a channel on a specific connection.
    pub async fn part(&self, id: &str, channel: &str) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?.part(channel)?;
        debug!("Connection '{}' parted channel: {}", id, channel);
        Ok(())
    }

    /// Add a channel to the auto-join set for a connection.
    ///
    /// The channel is joined immediately if the connection is ready, and will be
    /// automatically re-joined whenever the connection becomes ready (e.g. after
    /// a reconnect).
    pub async fn add_auto_join(&self, id: &str, channel: &str) -> Result<()> {
        self.auto_join_channels
            .write()
            .await
            .entry(id.to_string())
            .or_default()
            .insert(channel.to_string());

        // Best-effort join if already connected.
        if let Ok(true) = self.is_ready(id).await {
            let _ = self.join(id, channel).await;
        }

        debug!("Connection '{}' added auto-join channel: {}", id, channel);
        Ok(())
    }

    /// Remove a channel from the auto-join set and part it.
    pub async fn remove_auto_join(&self, id: &str, channel: &str) -> Result<()> {
        if let Some(set) = self.auto_join_channels.write().await.get_mut(id) {
            set.remove(channel);
        }

        // Best-effort part if connected.
        if let Ok(true) = self.is_ready(id).await {
            let _ = self.part(id, channel).await;
        }

        debug!("Connection '{}' removed auto-join channel: {}", id, channel);
        Ok(())
    }

    /// Get the current auto-join channels for a connection.
    pub async fn get_auto_join_channels(&self, id: &str) -> HashSet<String> {
        self.auto_join_channels
            .read()
            .await
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// Send a message on a specific connection, optionally as a reply.
    pub async fn send_message(
        &self,
        id: &str,
        target: &str,
        message: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?.send_message(target, message, reply_to)?;
        debug!("Connection '{}' sent message to {}", id, target);
        Ok(())
    }

    /// Send a message with data payload on a specific connection.
    pub async fn send_message_with_data(
        &self,
        id: &str,
        target: &str,
        message: &str,
        data: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?
            .send_message_with_data(target, message, data, reply_to)?;
        debug!("Connection '{}' sent message with data to {}", id, target);
        Ok(())
    }

    /// Request chat history for a channel on a specific connection.
    pub async fn request_history(&self, id: &str, channel: &str, limit: u32) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?.request_history(channel, limit)?;
        debug!("Connection '{}' requested history for {}", id, channel);
        Ok(())
    }

    /// Request older messages before a given timestamp.
    pub async fn request_history_before(
        &self,
        id: &str,
        channel: &str,
        before: &str,
        limit: u32,
    ) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?
            .request_history_before(channel, before, limit)?;
        debug!(
            "Connection '{}' requested history before {} for {}",
            id, before, channel
        );
        Ok(())
    }

    /// Take the event receiver for a connection.
    ///
    /// This can only be called once per connection. Subsequent calls return None.
    pub async fn take_event_receiver(
        &self,
        id: &str,
    ) -> Result<Option<mpsc::UnboundedReceiver<IrcEvent>>> {
        let mut connections = self.connections.write().await;

        let conn = connections
            .get_mut(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        Ok(conn
            .client
            .as_mut()
            .ok_or_else(|| IrcError::other("Connection is currently reconnecting".to_string()))?
            .take_event_receiver())
    }

    /// Get the list of successfully negotiated IRCv3 capabilities.
    pub async fn negotiated_caps(&self, id: &str) -> Result<Vec<String>> {
        let connections = self.connections.read().await;
        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;
        Ok(conn.client()?.negotiated_caps())
    }

    /// Check if a specific IRCv3 capability was negotiated for a connection.
    pub async fn has_capability(&self, id: &str, cap: &str) -> bool {
        let connections = self.connections.read().await;
        connections
            .get(id)
            .and_then(|conn| conn.client().ok())
            .map(|client| client.has_capability(cap))
            .unwrap_or(false)
    }

    /// List all connection IDs.
    pub async fn list_connections(&self) -> Vec<ConnectionId> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    /// Check if a connection exists.
    pub async fn exists(&self, id: &str) -> bool {
        let connections = self.connections.read().await;
        connections.contains_key(id)
    }

    /// Gracefully shut down all connections by sending QUIT to each server.
    ///
    /// This iterates over all managed connections, sends a QUIT message, and
    /// removes them from the pool. Errors on individual connections are logged
    /// but do not prevent other connections from being shut down.
    #[instrument(skip(self))]
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        let mut connections = self.connections.write().await;
        for (id, conn) in connections.drain() {
            if let Some(client) = conn.client {
                match client.quit("GitButler shutting down") {
                    Ok(()) => info!("Sent QUIT for connection '{}'", id),
                    Err(e) => debug!("Could not send QUIT for connection '{}': {}", id, e),
                }
            }
        }
    }

    // -- Message store methods --

    /// Store an incoming message. Returns `None` if deduplicated by `msgid`.
    #[allow(clippy::too_many_arguments)]
    pub async fn store_incoming(
        &self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        msgid: Option<&str>,
        reply_to: Option<&str>,
        tag: Option<&str>,
    ) -> Option<StoredMessage> {
        self.store.write().await.store_incoming(
            connection_id,
            target,
            sender,
            content,
            data,
            msgid,
            reply_to,
            tag,
        )
    }

    /// Store a history message (from chathistory batch) with server timestamp.
    /// Returns `None` if the message was a duplicate (by msgid).
    #[allow(clippy::too_many_arguments)]
    pub async fn store_history(
        &self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        server_time: Option<&str>,
        msgid: Option<&str>,
    ) -> Option<StoredMessage> {
        self.store.write().await.store_history(
            connection_id,
            target,
            sender,
            content,
            data,
            server_time,
            msgid,
        )
    }

    /// Store a batch of history messages. Returns only the messages that were
    /// actually inserted (not duplicates).
    pub async fn store_history_batch(
        &self,
        connection_id: &str,
        target: &str,
        batch: Vec<StoredMessage>,
    ) -> Vec<StoredMessage> {
        self.store
            .write()
            .await
            .store_history_batch(connection_id, target, batch)
    }

    /// Store an outgoing message and return the stored copy.
    pub async fn store_outgoing(
        &self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        reply_to: Option<&str>,
    ) -> StoredMessage {
        self.store.write().await.store_outgoing(
            connection_id,
            target,
            sender,
            content,
            data,
            reply_to,
        )
    }

    /// Store an outgoing message echoed back by the server (echo-message capability).
    #[allow(clippy::too_many_arguments)]
    pub async fn store_outgoing_echo(
        &self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        msgid: Option<&str>,
        reply_to: Option<&str>,
    ) -> StoredMessage {
        self.store.write().await.store_outgoing_echo(
            connection_id,
            target,
            sender,
            content,
            data,
            msgid,
            reply_to,
        )
    }

    /// Get stored messages for a channel/target.
    pub async fn get_messages(&self, connection_id: &str, target: &str) -> Vec<StoredMessage> {
        self.store.read().await.get_messages(connection_id, target)
    }

    /// Get channel list for a connection.
    pub async fn get_channels(&self, connection_id: &str) -> Vec<ChannelInfo> {
        self.store.read().await.get_channels(connection_id)
    }

    /// Get users for a specific channel.
    pub async fn get_users(
        &self,
        connection_id: &str,
        channel: &str,
    ) -> Vec<crate::message_store::UserEntry> {
        self.store.read().await.get_users(connection_id, channel)
    }

    /// Track a user joining a channel.
    pub async fn store_user_joined(&self, connection_id: &str, channel: &str, nick: &str) {
        self.store
            .write()
            .await
            .user_joined(connection_id, channel, nick);
    }

    /// Track a user leaving a channel.
    pub async fn store_user_parted(&self, connection_id: &str, channel: &str, nick: &str) {
        self.store
            .write()
            .await
            .user_parted(connection_id, channel, nick);
    }

    /// Track a user quitting. Returns the channels they were in.
    pub async fn store_user_quit(&self, connection_id: &str, nick: &str) -> Vec<String> {
        self.store.write().await.user_quit(connection_id, nick)
    }

    /// Track a nick change. Returns the channels the user was in.
    pub async fn store_nick_changed(
        &self,
        connection_id: &str,
        old_nick: &str,
        new_nick: &str,
    ) -> Vec<String> {
        self.store
            .write()
            .await
            .nick_changed(connection_id, old_nick, new_nick)
    }

    /// Set the full user list for a channel (from NAMES reply).
    pub async fn store_set_users(&self, connection_id: &str, channel: &str, users: Vec<String>) {
        self.store
            .write()
            .await
            .set_users(connection_id, channel, users);
    }

    /// Update away status for a user. Returns the channels they were found in.
    pub async fn store_set_user_away(
        &self,
        connection_id: &str,
        nick: &str,
        away: bool,
    ) -> Vec<String> {
        self.store
            .write()
            .await
            .set_user_away(connection_id, nick, away)
    }

    /// Set the topic for a channel.
    pub async fn store_set_topic(&self, connection_id: &str, channel: &str, topic: &str) {
        self.store
            .write()
            .await
            .set_topic(connection_id, channel, topic);
    }

    /// Add a channel to tracking (e.g. on self-JOIN).
    pub async fn store_add_channel(&self, connection_id: &str, channel: &str) {
        self.store.write().await.add_channel(connection_id, channel);
    }

    /// Remove a channel from tracking (e.g. on self-PART).
    pub async fn store_remove_channel(&self, connection_id: &str, channel: &str) {
        self.store
            .write()
            .await
            .remove_channel(connection_id, channel);
    }

    /// Remove a single message by msgid (for redaction).
    pub async fn remove_message_by_msgid(
        &self,
        connection_id: &str,
        target: &str,
        msgid: &str,
    ) -> bool {
        self.store
            .write()
            .await
            .remove_message_by_msgid(connection_id, target, msgid)
    }

    /// Clear stored messages for a channel.
    pub async fn store_clear_messages(&self, connection_id: &str, channel: &str) {
        self.store
            .write()
            .await
            .clear_messages(connection_id, channel);
    }

    /// Mark a channel as read (local store only).
    pub async fn store_mark_read(&self, connection_id: &str, channel: &str) {
        self.store.write().await.mark_read(connection_id, channel);
    }

    /// Update the local read timestamp from a server-provided ISO 8601 value.
    pub async fn store_mark_read_at(&self, connection_id: &str, channel: &str, timestamp_ms: u64) {
        self.store
            .write()
            .await
            .mark_read_at(connection_id, channel, timestamp_ms);
    }

    /// Send a MARKREAD command to the IRC server for a channel.
    ///
    /// If `timestamp` is provided, tells the server we've read up to that time.
    /// If `None`, queries the server for the current read marker.
    pub async fn send_markread(
        &self,
        id: &str,
        channel: &str,
        timestamp: Option<&str>,
    ) -> Result<()> {
        let cmd = match timestamp {
            Some(ts) => format!("MARKREAD {} timestamp={}", channel, ts),
            None => format!("MARKREAD {}", channel),
        };
        self.send_raw(id, &cmd).await
    }

    /// Record a commit reaction in the store.
    pub async fn store_reaction(
        &self,
        connection_id: &str,
        commit_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        self.store
            .write()
            .await
            .store_reaction(connection_id, commit_id, sender, reaction);
    }

    /// Remove a commit reaction from the store.
    pub async fn remove_reaction(
        &self,
        connection_id: &str,
        commit_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        self.store
            .write()
            .await
            .remove_reaction(connection_id, commit_id, sender, reaction);
    }

    /// Get all commit reactions for a connection, keyed by commit ID.
    pub async fn get_all_reactions(&self, connection_id: &str) -> HashMap<String, Vec<Reaction>> {
        self.store.read().await.get_all_reactions(connection_id)
    }

    /// Record a message reaction in the store (keyed by msgid).
    pub async fn store_message_reaction(
        &self,
        connection_id: &str,
        msg_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        self.store
            .write()
            .await
            .store_message_reaction(connection_id, msg_id, sender, reaction);
    }

    /// Remove a message reaction from the store.
    pub async fn remove_message_reaction(
        &self,
        connection_id: &str,
        msg_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        self.store
            .write()
            .await
            .remove_message_reaction(connection_id, msg_id, sender, reaction);
    }

    /// Find a message's data payload by its IRC msgid.
    pub async fn find_message_data_by_msgid(
        &self,
        connection_id: &str,
        msgid: &str,
    ) -> Option<String> {
        self.store
            .read()
            .await
            .find_message_data_by_msgid(connection_id, msgid)
    }

    /// Get all message reactions for a connection, keyed by message ID.
    pub async fn get_all_message_reactions(
        &self,
        connection_id: &str,
    ) -> HashMap<String, Vec<Reaction>> {
        self.store
            .read()
            .await
            .get_all_message_reactions(connection_id)
    }

    /// Index a hunk share message for reverse lookup by file path.
    pub async fn index_hunk_share(
        &self,
        connection_id: &str,
        file_path: &str,
        hunk_key: &str,
        share_msg_id: &str,
    ) {
        self.store
            .write()
            .await
            .index_hunk_share(connection_id, file_path, hunk_key, share_msg_id);
    }

    /// Get message reactions for a specific file, keyed by hunk key.
    pub async fn get_file_message_reactions(
        &self,
        connection_id: &str,
        file_path: &str,
    ) -> HashMap<String, Vec<Reaction>> {
        self.store
            .read()
            .await
            .get_file_message_reactions(connection_id, file_path)
    }

    /// Apply a full working-files sync for a user on a channel.
    pub async fn apply_working_files_sync(
        &self,
        connection_id: &str,
        channel: &str,
        nick: &str,
        files: Vec<String>,
    ) {
        self.store
            .write()
            .await
            .apply_working_files_sync(connection_id, channel, nick, files);
    }

    /// Apply a working-files delta (added/removed files) for a user on a channel.
    pub async fn apply_working_files_delta(
        &self,
        connection_id: &str,
        channel: &str,
        nick: &str,
        added: Vec<String>,
        removed: Vec<String>,
    ) {
        self.store.write().await.apply_working_files_delta(
            connection_id,
            channel,
            nick,
            added,
            removed,
        );
    }

    /// Get the current working files for all users on a channel.
    pub async fn get_working_files(
        &self,
        connection_id: &str,
        channel: &str,
    ) -> HashMap<String, Vec<String>> {
        self.store
            .read()
            .await
            .get_working_files(connection_id, channel)
    }

    /// Remove a user's working files from all channels (on PART or QUIT).
    pub async fn remove_working_files_user(&self, connection_id: &str, nick: &str) {
        self.store
            .write()
            .await
            .remove_working_files_user(connection_id, nick);
    }

    /// Send a raw IRC command on a specific connection.
    pub async fn send_raw(&self, id: &str, command: &str) -> Result<()> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        conn.client()?.send_raw(command)?;
        debug!("Connection '{}' sent raw command", id);
        Ok(())
    }

    /// Send a TAGMSG with the given tags to a target.
    pub async fn send_tagmsg(
        &self,
        id: &str,
        target: &str,
        tags: Vec<(String, Option<String>)>,
    ) -> Result<()> {
        let connections = self.connections.read().await;
        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;
        conn.client()?.send_tagmsg(target, tags)?;
        Ok(())
    }

    /// Get the nickname for a connection.
    pub async fn nick(&self, id: &str) -> Result<String> {
        let connections = self.connections.read().await;

        let conn = connections
            .get(id)
            .ok_or_else(|| IrcError::other(format!("Connection '{}' not found", id)))?;

        Ok(conn.client()?.nick().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_connection() {
        let manager = IrcManager::new();

        let config = IrcConfig {
            server: "irc.example.com".to_string(),
            server_password: None,
            sasl_password: None,
            port: 6697,
            use_tls: true,
            nick: "testbot".to_string(),
            username: None,
            realname: None,
        };

        manager.create("bot".to_string(), config).await.unwrap();

        assert!(manager.exists("bot").await);
        assert!(!manager.exists("user").await);
    }

    #[tokio::test]
    async fn test_duplicate_connection_fails() {
        let manager = IrcManager::new();

        let config = IrcConfig {
            server: "irc.example.com".to_string(),
            server_password: None,
            sasl_password: None,
            port: 6697,
            use_tls: true,
            nick: "testbot".to_string(),
            username: None,
            realname: None,
        };

        manager
            .create("bot".to_string(), config.clone())
            .await
            .unwrap();

        let result = manager.create("bot".to_string(), config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_connections() {
        let manager = IrcManager::new();

        let config = IrcConfig {
            server: "irc.example.com".to_string(),
            server_password: None,
            sasl_password: None,
            port: 6697,
            use_tls: true,
            nick: "testbot".to_string(),
            username: None,
            realname: None,
        };

        manager
            .create("bot".to_string(), config.clone())
            .await
            .unwrap();
        manager.create("user".to_string(), config).await.unwrap();

        let ids = manager.list_connections().await;
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"bot".to_string()));
        assert!(ids.contains(&"user".to_string()));
    }

    #[tokio::test]
    async fn test_remove_connection() {
        let manager = IrcManager::new();

        let config = IrcConfig {
            server: "irc.example.com".to_string(),
            server_password: None,
            sasl_password: None,
            port: 6697,
            use_tls: true,
            nick: "testbot".to_string(),
            username: None,
            realname: None,
        };

        manager.create("bot".to_string(), config).await.unwrap();
        assert!(manager.exists("bot").await);

        manager.remove("bot", "test").await.unwrap();
        assert!(!manager.exists("bot").await);
    }
}
