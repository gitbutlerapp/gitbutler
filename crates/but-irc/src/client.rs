//! IRC client wrapper using the `irc` crate.

use crate::error::{IrcError, Result};
use crate::message::IrcEvent;
use crate::state::ConnectionState;
use futures::StreamExt;
use irc::client::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{RwLock, mpsc, watch};
use tracing::{debug, error, info, info_span, warn};

/// Configuration for IRC client.
#[derive(Clone)]
pub struct IrcConfig {
    /// IRC server hostname (e.g., "irc.example.com")
    pub server: String,
    /// IRC server port (default: 6697 for TLS)
    pub port: u16,
    /// Use TLS (default: true)
    pub use_tls: bool,
    /// IRC nickname
    pub nick: String,
    /// IRC server password (optional)
    pub password: Option<String>,
    /// IRC username (defaults to nick if not provided)
    pub username: Option<String>,
    /// IRC realname (defaults to nick if not provided)
    pub realname: Option<String>,
}

impl std::fmt::Debug for IrcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IrcConfig")
            .field("server", &self.server)
            .field("port", &self.port)
            .field("use_tls", &self.use_tls)
            .field("nick", &self.nick)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("username", &self.username)
            .field("realname", &self.realname)
            .finish()
    }
}

impl Default for IrcConfig {
    fn default() -> Self {
        Self {
            server: String::new(),
            port: 6697,
            use_tls: true,
            nick: String::new(),
            password: None,
            username: None,
            realname: None,
        }
    }
}

/// IRC client wrapper with event handling.
pub struct IrcClient {
    config: IrcConfig,
    state: Arc<RwLock<ConnectionState>>,
    /// Incremented on each `connect()` call so that stale message handlers
    /// (from a previous connection) do not overwrite state set by a newer one.
    generation: Arc<AtomicU64>,
    client: Option<Client>,
    event_tx: mpsc::UnboundedSender<IrcEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<IrcEvent>>,
    shutdown_rx: watch::Receiver<bool>,
    /// IRCv3 capabilities that were successfully negotiated with the server.
    negotiated_caps: Vec<String>,
    /// Counter for generating unique BATCH reference IDs.
    batch_counter: AtomicU64,
}

impl IrcClient {
    /// Create a new IRC client with the given configuration.
    pub fn new(config: IrcConfig, shutdown_rx: watch::Receiver<bool>) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            generation: Arc::new(AtomicU64::new(0)),
            client: None,
            event_tx,
            event_rx: Some(event_rx),
            shutdown_rx,
            negotiated_caps: Vec::new(),
            batch_counter: AtomicU64::new(0),
        }
    }

    /// Get the current connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Get a clone of the state handle for use outside of the connection lock.
    pub fn state_handle(&self) -> Arc<RwLock<ConnectionState>> {
        self.state.clone()
    }

    /// Check if the client is ready to send messages.
    pub async fn is_ready(&self) -> bool {
        *self.state.read().await == ConnectionState::Ready
    }

    /// Set connection state and emit a StateChanged event.
    async fn set_state(&self, new_state: ConnectionState) {
        *self.state.write().await = new_state;
        let state_str = new_state.to_string();
        let _ = self
            .event_tx
            .send(IrcEvent::StateChanged { state: state_str });
    }

    /// Set connection state and emit a StateChanged event (public for use by manager).
    pub async fn set_state_public(&self, new_state: ConnectionState) {
        self.set_state(new_state).await;
    }

    /// Negotiate IRCv3 capabilities and send registration commands.
    ///
    /// Performs proper CAP negotiation:
    /// 1. `CAP LS 302` — ask server what caps it supports
    /// 2. Wait for `CAP * LS` response
    /// 3. `CAP REQ :batch server-time` — request the caps the server supports
    /// 4. Wait for `CAP * ACK`
    /// 5. `CAP END` — finish negotiation
    /// 6. `PASS` / `NICK` / `USER` — register
    ///
    /// Returns the client stream for reuse by `spawn_message_handler` (the `irc`
    /// crate only allows `stream()` to be called once) and the list of
    /// capabilities that were successfully negotiated.
    async fn negotiate_caps_and_register(
        &self,
        client: &mut Client,
    ) -> Result<(irc::client::ClientStream, Vec<String>)> {
        use tokio::time::timeout;

        // Start capability negotiation
        client
            .send_cap_ls(NegotiationVersion::V302)
            .map_err(|e| IrcError::other(format!("Failed to send CAP LS: {}", e)))?;

        // Get the stream — can only be called once per Client.
        let mut stream = client.stream().map_err(|e| {
            IrcError::other(format!("Failed to get stream for CAP negotiation: {}", e))
        })?;

        // Single list of desired IRCv3 capabilities.
        // Each entry is the capability name as it appears in `CAP LS` / `CAP REQ`.
        // We use `Capability::Custom` for all of them because `send_cap_req`
        // only calls `.as_ref()` to get the string back — the dedicated enum
        // variants (`Batch`, `ServerTime`, …) produce identical wire output.
        let desired_caps: &[&str] = &[
            "batch",
            "server-time",
            "draft/chathistory",
            "draft/event-playback",
            "draft/multiline",
            "message-tags",
            "sasl",
            "away-notify",
            "echo-message",
        ];
        let mut caps_to_request = Vec::new();
        let mut negotiated: Vec<String> = Vec::new();

        // Wait for the CAP LS response (with timeout).
        //
        // With CAP LS 302, the server may split capabilities across multiple
        // lines.  Continuation lines use `*` as the middle param and put the
        // caps in the trailing param.  The final line puts caps directly in
        // the middle param (no `*`).
        let cap_result = timeout(std::time::Duration::from_secs(10), async {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(message) => {
                        debug!("CAP negotiation: < {:?}", message);
                        if let Command::CAP(_, ref sub, ref middle, ref trailing) = message.command
                        {
                            if matches!(sub, irc::proto::command::CapSubCommand::LS) {
                                // "*" as middle param means continuation — caps are in trailing.
                                // Otherwise the caps are in middle (final line).
                                let is_continuation = middle.as_deref() == Some("*");
                                let caps_str = if is_continuation {
                                    trailing.as_deref()
                                } else {
                                    middle.as_deref().or(trailing.as_deref())
                                };

                                if let Some(caps) = caps_str {
                                    let advertised: Vec<&str> =
                                        caps.split_whitespace().collect();
                                    for desired in desired_caps {
                                        if advertised.iter().any(|cap| {
                                            cap.split('=').next() == Some(desired)
                                        }) && !caps_to_request.contains(desired)
                                        {
                                            caps_to_request.push(*desired);
                                        }
                                    }
                                }

                                info!(
                                    server_caps = ?caps_str,
                                    is_continuation,
                                    requesting = ?caps_to_request,
                                    "CAP LS received",
                                );

                                // Only break on the final line (no continuation marker)
                                if !is_continuation {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return Err(IrcError::other(format!(
                            "Error during CAP negotiation: {}",
                            e
                        )));
                    }
                }
            }
            Ok(())
        })
        .await;

        match cap_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                warn!("CAP LS timed out, proceeding without capability negotiation");
            }
        }

        // Request the caps the server supports
        if !caps_to_request.is_empty() {
            let caps: Vec<Capability> = caps_to_request
                .iter()
                .map(|name| Capability::Custom(name))
                .collect();

            if !caps.is_empty() {
                client
                    .send_cap_req(&caps)
                    .map_err(|e| IrcError::other(format!("Failed to send CAP REQ: {}", e)))?;

                // Wait for ACK/NAK (with timeout)
                let mut acked_caps = Vec::new();
                let ack_result = timeout(std::time::Duration::from_secs(5), async {
                    while let Some(msg) = stream.next().await {
                        match msg {
                            Ok(message) => {
                                debug!("CAP negotiation: < {:?}", message);
                                if let Command::CAP(_, ref sub, ref middle, ref trailing) =
                                    message.command
                                {
                                    match sub {
                                        irc::proto::command::CapSubCommand::ACK => {
                                            // Parse the ACK'd caps from the response
                                            let caps_str =
                                                middle.as_deref().or(trailing.as_deref());
                                            if let Some(caps) = caps_str {
                                                acked_caps = caps
                                                    .split_whitespace()
                                                    .map(|s| s.to_string())
                                                    .collect();
                                            }
                                            info!(caps = ?acked_caps, "CAP ACK received");
                                            break;
                                        }
                                        irc::proto::command::CapSubCommand::NAK => {
                                            warn!("CAP NAK received");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Err(e) => {
                                return Err(IrcError::other(format!(
                                    "Error during CAP negotiation: {}",
                                    e
                                )));
                            }
                        }
                    }
                    Ok(())
                })
                .await;

                match ack_result {
                    Ok(Ok(())) => {
                        negotiated = acked_caps;
                    }
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        warn!("CAP ACK timed out, proceeding anyway");
                    }
                }
            }
        }

        // Perform SASL PLAIN authentication if the server supports it and we have a password
        let sasl_succeeded = if negotiated.iter().any(|c| c == "sasl") {
            if let Some(ref password) = self.config.password {
                match self.perform_sasl_auth(client, &mut stream, password).await {
                    Ok(()) => {
                        info!("SASL PLAIN authentication succeeded");
                        true
                    }
                    Err(e) => {
                        warn!(error = %e, "SASL PLAIN authentication failed, proceeding without auth");
                        false
                    }
                }
            } else {
                false
            }
        } else {
            false
        };

        // End capability negotiation and register.
        // Send CAP END, then PASS/NICK/USER manually instead of calling identify(),
        // so we can skip PASS when SASL already handled authentication.
        use irc::proto::command::CapSubCommand::END;
        client
            .send(Command::CAP(None, END, None, None))
            .map_err(|e| IrcError::other(format!("Failed to send CAP END: {}", e)))?;

        if !sasl_succeeded {
            if let Some(ref password) = self.config.password {
                if !password.is_empty() {
                    client
                        .send(Command::PASS(password.clone()))
                        .map_err(|e| IrcError::other(format!("Failed to send PASS: {}", e)))?;
                }
            }
        }

        let nick = self.config.nick.clone();
        let username = self.config.username.clone().unwrap_or_else(|| nick.clone());
        let realname = self.config.realname.clone().unwrap_or_else(|| nick.clone());

        client
            .send(Command::NICK(nick))
            .map_err(|e| IrcError::other(format!("Failed to send NICK: {}", e)))?;
        client
            .send(Command::USER(username, "0".to_string(), realname))
            .map_err(|e| IrcError::other(format!("Failed to send USER: {}", e)))?;

        Ok((stream, negotiated))
    }

    /// Perform SASL PLAIN authentication.
    ///
    /// SASL PLAIN sends credentials as base64(`authzid\0authcid\0password`),
    /// where authzid and authcid are both the nick.
    async fn perform_sasl_auth(
        &self,
        client: &Client,
        stream: &mut irc::client::ClientStream,
        password: &str,
    ) -> Result<()> {
        use base64::Engine;
        use irc::proto::response::Response;
        use tokio::time::timeout;

        // Step 1: Tell the server we want PLAIN mechanism
        client
            .send(Command::AUTHENTICATE("PLAIN".to_string()))
            .map_err(|e| IrcError::other(format!("Failed to send AUTHENTICATE PLAIN: {}", e)))?;

        // Step 2: Wait for the server's AUTHENTICATE + response
        let auth_result = timeout(std::time::Duration::from_secs(10), async {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(message) => {
                        debug!("SASL: < {:?}", message);
                        match &message.command {
                            // Server sends AUTHENTICATE + to indicate it's ready for credentials
                            Command::AUTHENTICATE(data) if data == "+" => {
                                // Build SASL PLAIN payload: base64(nick\0nick\0password)
                                let nick = &self.config.nick;
                                let plain = format!("{nick}\0{nick}\0{password}");
                                let encoded = base64::engine::general_purpose::STANDARD
                                    .encode(plain.as_bytes());

                                client.send(Command::AUTHENTICATE(encoded)).map_err(|e| {
                                    IrcError::other(format!(
                                        "Failed to send AUTHENTICATE credentials: {}",
                                        e
                                    ))
                                })?;
                            }
                            // 903: SASL authentication successful
                            Command::Response(Response::RPL_SASLSUCCESS, _) => {
                                return Ok(());
                            }
                            // 904: SASL authentication failed
                            Command::Response(Response::ERR_SASLFAIL, params) => {
                                let msg = params
                                    .last()
                                    .map(|s| s.as_str())
                                    .unwrap_or("Authentication failed");
                                return Err(IrcError::other(format!("SASL failed: {}", msg)));
                            }
                            // 905: SASL message too long
                            Command::Response(Response::ERR_SASLTOOLONG, _) => {
                                return Err(IrcError::other("SASL credentials too long"));
                            }
                            // 906: SASL aborted
                            Command::Response(Response::ERR_SASLABORT, _) => {
                                return Err(IrcError::other("SASL authentication aborted"));
                            }
                            // 907: Already authenticated
                            Command::Response(Response::ERR_SASLALREADY, _) => {
                                return Ok(());
                            }
                            _ => {
                                // Ignore other messages during SASL exchange (e.g., NOTICE)
                            }
                        }
                    }
                    Err(e) => {
                        return Err(IrcError::other(format!("Error during SASL: {}", e)));
                    }
                }
            }
            Err(IrcError::other("Stream ended during SASL authentication"))
        })
        .await;

        match auth_result {
            Ok(result) => result,
            Err(_) => Err(IrcError::other("SASL authentication timed out")),
        }
    }

    /// Connect to the IRC server.
    ///
    /// If already connected or connecting, this is a no-op and returns Ok.
    pub async fn connect(&mut self) -> Result<()> {
        // Check current state
        let current_state = *self.state.read().await;

        // If already connected, ready, or connecting, reuse the existing connection
        if matches!(
            current_state,
            ConnectionState::Ready | ConnectionState::Connecting | ConnectionState::Negotiating
        ) {
            debug!(
                state = %current_state,
                "Connection already active, reusing existing connection",
            );
            return Ok(());
        }

        let span = info_span!("irc_connection",
            server = %self.config.server,
            port = self.config.port,
            nick = %self.config.nick,
            tls = self.config.use_tls,
        );
        let _enter = span.enter();

        // Bump generation so any still-running handler from a previous
        // connection will no longer touch the shared state.
        let new_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        self.set_state(ConnectionState::Connecting).await;
        info!(
            generation = new_gen,
            previous_state = %current_state,
            "Connecting",
        );

        let irc_config = Config {
            nickname: Some(self.config.nick.clone()),
            // Password is sent manually in negotiate_caps_and_register
            // to avoid double-sending when the irc crate also tries PASS.
            password: None,
            username: Some(
                self.config
                    .username
                    .clone()
                    .unwrap_or_else(|| self.config.nick.clone()),
            ),
            realname: Some(
                self.config
                    .realname
                    .clone()
                    .unwrap_or_else(|| self.config.nick.clone()),
            ),
            server: Some(self.config.server.clone()),
            port: Some(self.config.port),
            use_tls: Some(self.config.use_tls),
            ..Config::default()
        };

        let mut client = match Client::from_config(irc_config).await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "Failed to create IRC client");
                self.set_state(ConnectionState::Error).await;
                return Err(IrcError::other(format!("Failed to create client: {}", e)));
            }
        };

        // Negotiate IRCv3 capabilities before registering.
        // The `irc` crate only allows `client.stream()` to be called once,
        // so we capture the stream here and pass it to `spawn_message_handler`.
        let (stream, negotiated) = match self.negotiate_caps_and_register(&mut client).await {
            Ok(result) => result,
            Err(e) => {
                error!(error = %e, "Failed to negotiate/register");
                self.set_state(ConnectionState::Error).await;
                return Err(e);
            }
        };
        self.negotiated_caps = negotiated;

        self.set_state(ConnectionState::Negotiating).await;
        info!("Negotiating");

        self.client = Some(client);

        // Spawn the message handling loop
        self.spawn_message_handler(stream);

        Ok(())
    }

    /// Spawn a background task to handle incoming messages.
    ///
    /// The stream must be obtained from `negotiate_caps_and_register` — the
    /// `irc` crate only allows `client.stream()` to be called once.
    fn spawn_message_handler(&mut self, mut stream: irc::client::ClientStream) {
        let state = Arc::clone(&self.state);
        let generation = Arc::clone(&self.generation);
        let my_gen = generation.load(Ordering::SeqCst);
        let event_tx = self.event_tx.clone();
        let mut shutdown = self.shutdown_rx.clone();
        let server = self.config.server.clone();
        let port = self.config.port;
        let nick = self.config.nick.clone();

        // NOTE: Do NOT use `.instrument(span)` on this long-lived task.
        // `tracing_forest` buffers all output within a span until the span
        // closes, so an instrument span that lives for the entire connection
        // would swallow every log message until disconnect.
        tokio::spawn(async move {
            let mut disconnect_reason = "stream_ended";
            let mut last_command = String::new();
            let mut message_count: u64 = 0;

            // Helper: only write state if we are still the current generation.
            // A newer `connect()` call bumps the generation, making this handler stale.
            let is_current = || generation.load(Ordering::SeqCst) == my_gen;

            info!(%server, port, %nick, generation = my_gen, "Message handler started");

            loop {
                tokio::select! {
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(message)) => {
                                message_count += 1;
                                last_command = format!("{:?}", message.command);

                                debug!(%server, "< {:?}", message);

                                match &message.command {
                                    // Welcome — connection is ready
                                    Command::Response(Response::RPL_WELCOME, _) => {
                                        if is_current() {
                                            *state.write().await = ConnectionState::Ready;
                                            let _ = event_tx.send(IrcEvent::StateChanged { state: "ready".to_string() });
                                            info!(%server, generation = my_gen, "Connection ready (state set to Ready)");
                                        } else {
                                            info!(%server, generation = my_gen, "Connection ready (stale handler, state NOT updated)");
                                        }
                                    }
                                    // Server ERROR — clean server-initiated close
                                    Command::ERROR(msg) => {
                                        warn!(%server, message = %msg, "Server sent ERROR");
                                        disconnect_reason = "server_error";
                                    }
                                    // KILL — server operator killed us
                                    Command::KILL(_, msg) => {
                                        warn!(%server, message = %msg, "Killed by server");
                                        disconnect_reason = "killed";
                                    }
                                    _ => {}
                                }

                                // Convert to our event type and send
                                if let Some(event) = IrcEvent::from_irc_message(&message) {
                                    if event_tx.send(event).is_err() {
                                        debug!(%server, "Event receiver dropped, stopping message handler");
                                        break;
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                let err_str = e.to_string();
                                if err_str.contains("tls") || err_str.contains("ssl") || err_str.contains("certificate") {
                                    error!(%server, error = %e, reason = "tls_error", "TLS error");
                                    disconnect_reason = "tls_error";
                                } else {
                                    error!(%server, error = %e, reason = "network_failure", "Connection error");
                                    disconnect_reason = "network_failure";
                                }
                                if is_current() {
                                    *state.write().await = ConnectionState::Error;
                                    let _ = event_tx.send(IrcEvent::StateChanged { state: "error".to_string() });
                                }
                                break;
                            }
                            None => {
                                // Stream ended — keep whatever reason was set
                                // (server_error/killed set it above, otherwise stream_ended)
                                break;
                            }
                        }
                    }
                    _ = shutdown.changed() => {
                        disconnect_reason = "client_quit";
                        break;
                    }
                }
            }

            if is_current() {
                *state.write().await = ConnectionState::Disconnected;
                let _ = event_tx.send(IrcEvent::StateChanged {
                    state: "disconnected".to_string(),
                });
                info!(
                    %server,
                    reason = disconnect_reason,
                    last_command,
                    message_count,
                    generation = my_gen,
                    "Connection closed",
                );
            } else {
                debug!(
                    %server,
                    reason = disconnect_reason,
                    last_command,
                    message_count,
                    generation = my_gen,
                    current_generation = generation.load(Ordering::SeqCst),
                    "Stale handler exiting (superseded by reconnect)",
                );
            }
        });
    }

    /// Take the event receiver. Can only be called once.
    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<IrcEvent>> {
        self.event_rx.take()
    }

    /// Send a raw IRC command.
    pub fn send_raw(&self, command: impl Into<String>) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        let cmd_str = command.into();
        debug!("> {}", cmd_str);

        // Parse and send the raw command
        let message: Message = cmd_str
            .parse()
            .map_err(|e| IrcError::protocol(format!("Invalid command: {}", e)))?;

        client
            .send(message)
            .map_err(|e| IrcError::other(format!("Send failed: {}", e)))
    }

    /// Send a message to a channel or user, optionally as a reply.
    ///
    /// If the message contains newlines and the server supports `draft/multiline`,
    /// the message is sent as a BATCH of PRIVMSGs. Otherwise each line is sent
    /// as a separate PRIVMSG (fallback).
    pub fn send_message(&self, target: &str, message: &str, reply_to: Option<&str>) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        let lines: Vec<&str> = message.split('\n').collect();
        let is_multiline = lines.len() > 1;

        // Use draft/multiline BATCH when available and message has newlines
        if is_multiline && self.has_capability("draft/multiline") {
            return self.send_multiline_batch(client, target, &lines, reply_to, None);
        }

        // Single line or fallback: send each line as a separate PRIVMSG
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() && is_multiline {
                continue; // skip empty lines in fallback mode
            }
            // Only attach reply-to on the first line
            if i == 0 {
                if let Some(msgid) = reply_to {
                    self.send_privmsg_with_tags(client, target, line, Some(msgid), None)?;
                } else {
                    debug!("> PRIVMSG {} :{}", target, line);
                    client
                        .send_privmsg(target, line)
                        .map_err(|e| IrcError::other(format!("Send failed: {}", e)))?;
                }
            } else {
                debug!("> PRIVMSG {} :{}", target, line);
                client
                    .send_privmsg(target, line)
                    .map_err(|e| IrcError::other(format!("Send failed: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Send a message with an embedded data payload.
    ///
    /// The data is base64 encoded and embedded in the PRIVMSG body as a CTCP-style
    /// envelope: `\x01ACTION GBDATA <base64> :<human text>\x01`
    ///
    /// Regular IRC clients treat unknown CTCP commands silently (no visible output).
    /// GitButler clients parse the envelope to extract the data, and display the
    /// human-readable text portion to the user.
    ///
    /// This avoids the IRCv3 +data tag 4KB size limit — the PRIVMSG body is only
    /// limited by the server's max-line-len setting.
    ///
    /// If the message contains newlines and the server supports `draft/multiline`,
    /// the first line carries the GBDATA envelope and subsequent lines are plain
    /// PRIVMSGs, all wrapped in a BATCH.
    pub fn send_message_with_data(
        &self,
        target: &str,
        message: &str,
        data: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        use base64::{Engine as _, engine::general_purpose};

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        let encoded = general_purpose::STANDARD.encode(data);
        let lines: Vec<&str> = message.split('\n').collect();
        let is_multiline = lines.len() > 1;

        if is_multiline && self.has_capability("draft/multiline") {
            return self.send_multiline_batch(client, target, &lines, reply_to, Some(&encoded));
        }

        // Single line (or fallback): pack everything into one GBDATA PRIVMSG.
        // Use ACTION framing so Ergo persists the message in chathistory.
        let body = format!("\x01ACTION GBDATA {encoded} :{message}\x01");
        debug!("> PRIVMSG {} :<GBDATA {} bytes>", target, body.len());

        if reply_to.is_some() {
            self.send_privmsg_with_tags(client, target, &body, reply_to, None)
        } else {
            client
                .send_privmsg(target, &body)
                .map_err(|e| IrcError::other(format!("Send failed: {}", e)))
        }
    }

    /// Join a channel.
    pub fn join(&self, channel: &str) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        debug!("> JOIN {}", channel);

        client
            .send_join(channel)
            .map_err(|e| IrcError::other(format!("Join failed: {}", e)))
    }

    /// Part (leave) a channel.
    pub fn part(&self, channel: &str) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        debug!("> PART {}", channel);

        client
            .send(Command::PART(channel.to_string(), None))
            .map_err(|e| IrcError::other(format!("Part failed: {}", e)))
    }

    /// Request chat history for a channel using IRCv3 CHATHISTORY.
    ///
    /// Sends `CHATHISTORY LATEST <channel> * <limit>` which asks the server
    /// for the most recent messages. The server responds with a BATCH
    /// containing PRIVMSG messages.
    pub fn request_history(&self, channel: &str, limit: u32) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        let raw = format!("CHATHISTORY LATEST {} * {}", channel, limit);
        info!("> {}", raw);

        let message: Message = raw
            .parse()
            .map_err(|e| IrcError::protocol(format!("Invalid CHATHISTORY command: {}", e)))?;

        client
            .send(message)
            .map_err(|e| IrcError::other(format!("Send failed: {}", e)))?;

        info!("request_history: sent for {} (limit {})", channel, limit);
        Ok(())
    }

    /// Request older messages before a given timestamp.
    ///
    /// Sends `CHATHISTORY BEFORE <channel> timestamp=<iso8601> <limit>` which asks
    /// the server for messages older than the given timestamp. Used for pagination
    /// ("load more" on scroll up).
    pub fn request_history_before(&self, channel: &str, before: &str, limit: u32) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        let raw = format!(
            "CHATHISTORY BEFORE {} timestamp={} {}",
            channel, before, limit
        );
        info!("> {}", raw);

        let message: Message = raw
            .parse()
            .map_err(|e| IrcError::protocol(format!("Invalid CHATHISTORY command: {}", e)))?;

        client
            .send(message)
            .map_err(|e| IrcError::other(format!("Send failed: {}", e)))?;

        debug!(
            "request_history_before: sent for {} (before {}, limit {})",
            channel, before, limit
        );
        Ok(())
    }

    /// Check if a capability was successfully negotiated with the server.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.negotiated_caps.iter().any(|c| c == cap)
    }

    /// Get all negotiated capabilities.
    pub fn negotiated_caps(&self) -> Vec<String> {
        self.negotiated_caps.clone()
    }

    /// Generate a unique batch reference ID.
    fn next_batch_id(&self) -> String {
        let n = self.batch_counter.fetch_add(1, Ordering::Relaxed);
        format!("gb{n}")
    }

    /// Send a PRIVMSG with optional IRCv3 tags (reply-to, batch reference).
    fn send_privmsg_with_tags(
        &self,
        client: &Client,
        target: &str,
        text: &str,
        reply_to: Option<&str>,
        batch: Option<&str>,
    ) -> Result<()> {
        use irc::proto::Message;
        use irc::proto::message::Tag;

        let mut tags = Vec::new();
        if let Some(msgid) = reply_to {
            tags.push(Tag("+draft/reply".to_string(), Some(msgid.to_string())));
        }
        if let Some(bid) = batch {
            tags.push(Tag("batch".to_string(), Some(bid.to_string())));
        }

        let msg = Message::with_tags(
            if tags.is_empty() { None } else { Some(tags) },
            None,
            "PRIVMSG",
            vec![target, text],
        )
        .map_err(|e| IrcError::other(format!("Failed to build message: {}", e)))?;

        client
            .send(msg)
            .map_err(|e| IrcError::other(format!("Send failed: {}", e)))
    }

    /// Send a multiline message as an IRCv3 `draft/multiline` BATCH.
    ///
    /// When `encoded_data` is provided, the first PRIVMSG carries a CTCP GBDATA
    /// envelope; subsequent PRIVMSGs are plain text. The receiver concatenates
    /// all lines and associates the data with the full message.
    fn send_multiline_batch(
        &self,
        client: &Client,
        target: &str,
        lines: &[&str],
        reply_to: Option<&str>,
        encoded_data: Option<&str>,
    ) -> Result<()> {
        use irc::proto::Message;
        use irc::proto::command::BatchSubCommand;

        let batch_id = self.next_batch_id();
        debug!(
            "> BATCH +{} draft/multiline {} ({} lines)",
            batch_id,
            target,
            lines.len()
        );

        // Construct BATCH commands directly to preserve "draft/multiline" casing.
        // The irc crate uppercases batch subcommands when parsing from strings,
        // but servers expect the exact lowercase form.
        let start = Message {
            tags: None,
            prefix: None,
            command: Command::BATCH(
                format!("+{batch_id}"),
                Some(BatchSubCommand::CUSTOM("draft/multiline".to_string())),
                Some(vec![target.to_string()]),
            ),
        };
        client
            .send(start)
            .map_err(|e| IrcError::other(format!("Send failed: {}", e)))?;

        // Send each line as a PRIVMSG tagged with the batch reference.
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                if let Some(data) = encoded_data {
                    // First line with data: GBDATA envelope
                    let gbdata_body = format!("\x01ACTION GBDATA {data} :{line}\x01");
                    self.send_privmsg_with_tags(
                        client,
                        target,
                        &gbdata_body,
                        reply_to,
                        Some(&batch_id),
                    )?;
                } else {
                    self.send_privmsg_with_tags(client, target, line, reply_to, Some(&batch_id))?;
                }
            } else {
                self.send_privmsg_with_tags(client, target, line, None, Some(&batch_id))?;
            }
        }

        // BATCH -<id>
        let end = Message {
            tags: None,
            prefix: None,
            command: Command::BATCH(format!("-{batch_id}"), None, None),
        };
        client
            .send(end)
            .map_err(|e| IrcError::other(format!("Send failed: {}", e)))
    }

    /// Send a QUIT message to the IRC server with a custom message.
    ///
    /// This sends the QUIT command but does not drop the connection — the server
    /// will close it after acknowledging the quit.
    pub fn quit(&self, message: &str) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| IrcError::not_ready("Not connected"))?;

        debug!("> QUIT :{}", message);
        client
            .send_quit(message)
            .map_err(|e| IrcError::other(format!("Quit failed: {}", e)))
    }

    /// Disconnect from the IRC server.
    pub fn disconnect(&mut self, reason: &str) -> Result<()> {
        if let Some(client) = &self.client {
            info!(reason = reason, nick = %self.config.nick, "Disconnecting");
            let _ = client.send_quit("Disconnecting");
        } else {
            debug!(reason = reason, nick = %self.config.nick, "Disconnect called but no client");
        }
        self.client = None;
        // State will be updated by the message handler when it detects disconnect
        Ok(())
    }

    /// Get the current nickname.
    pub fn nick(&self) -> &str {
        &self.config.nick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = IrcConfig {
            server: "irc.example.com".to_string(),
            password: Some("".to_string()),
            port: 6697,
            use_tls: true,
            nick: "testbot".to_string(),
            username: None,
            realname: None,
        };
        assert_eq!(config.nick, "testbot");
        assert_eq!(config.port, 6697);
        assert!(config.use_tls);
    }

    #[test]
    fn test_config_default() {
        let config = IrcConfig::default();
        assert_eq!(config.port, 6697);
        assert!(config.use_tls);
    }
}
