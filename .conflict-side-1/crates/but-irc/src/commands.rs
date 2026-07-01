//! Shared IRC command handlers.
//!
//! These are the core implementations called by both `gitbutler-tauri` (Tauri
//! commands) and `but-server` (Axum handlers).  Each function takes an
//! [`IrcManager`] plus an [`EventEmitter`] where needed.

use crate::lifecycle::EventEmitter;
use crate::message_store::Reaction;
use crate::{
    ChannelInfo, ConnectionId, ConnectionState, DEFAULT_HISTORY_LIMIT, IrcConfig, IrcError,
    IrcManager, Result, StoredMessage,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Param / response types
// ============================================================================

/// Parameters for creating an IRC connection.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectParams {
    pub id: String,
    pub server: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_tls")]
    pub use_tls: bool,
    pub nick: String,
    pub server_password: Option<String>,
    pub sasl_password: Option<String>,
    pub username: Option<String>,
    pub realname: Option<String>,
}

impl std::fmt::Debug for ConnectParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectParams")
            .field("id", &self.id)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("use_tls", &self.use_tls)
            .field("nick", &self.nick)
            .field(
                "server_password",
                &self.server_password.as_ref().map(|_| "***"),
            )
            .field("sasl_password", &self.sasl_password.as_ref().map(|_| "***"))
            .field("username", &self.username)
            .field("realname", &self.realname)
            .finish()
    }
}

fn default_port() -> u16 {
    6697
}
fn default_tls() -> bool {
    true
}

/// Connection state response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStateResponse {
    pub id: String,
    pub state: String,
    pub ready: bool,
}

/// Params: just a connection id.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdParams {
    pub id: String,
}

/// Params: connection id + channel.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelParams {
    pub id: String,
    pub channel: String,
}

/// Params: wait-ready with optional timeout.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitReadyParams {
    pub id: String,
    pub timeout_secs: Option<u64>,
}

/// Params: send a PRIVMSG.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    pub id: String,
    pub target: String,
    pub message: String,
    #[serde(default)]
    pub reply_to: Option<String>,
}

/// Params: send a PRIVMSG with an attached data payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageWithDataParams {
    pub id: String,
    pub target: String,
    pub message: String,
    pub data: String,
    #[serde(default)]
    pub reply_to: Option<String>,
}

/// Params: request chat history.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryParams {
    pub id: String,
    pub channel: String,
    pub limit: Option<u32>,
}

/// Params: request chat history before a timestamp.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryBeforeParams {
    pub id: String,
    pub channel: String,
    pub before: String,
    pub limit: Option<u32>,
}

/// Params: send a raw IRC command.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRawParams {
    pub id: String,
    pub command: String,
}

/// Params: send a typing indicator.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTypingParams {
    pub id: String,
    pub target: String,
    pub state: String,
}

/// Params: send or remove a reaction.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionParams {
    pub id: String,
    pub target: String,
    pub msgid: String,
    pub emoji: String,
}

/// Params: redact (delete) a message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactParams {
    pub id: String,
    pub target: String,
    pub msgid: String,
    pub reason: Option<String>,
}

/// Params: get message reactions for a specific file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMessageReactionsParams {
    pub id: String,
    pub file_path: String,
}

// ============================================================================
// Command implementations
// ============================================================================

/// Create and connect, then spawn the event forwarder.
pub async fn connect(
    manager: &IrcManager,
    emitter: &impl EventEmitter,
    params: ConnectParams,
) -> Result<()> {
    let config = IrcConfig {
        server: params.server,
        port: params.port,
        use_tls: params.use_tls,
        nick: params.nick,
        server_password: params.server_password,
        sasl_password: params.sasl_password,
        username: params.username,
        realname: params.realname,
    };
    let id = params.id;

    manager.create_and_connect(id.clone(), config).await?;
    crate::lifecycle::spawn_event_forwarder(manager, emitter, &id).await;

    tracing::info!("IRC connection '{}' established", id);
    Ok(())
}

/// Disconnect and remove a connection.
pub async fn disconnect(manager: &IrcManager, id: &str) -> Result<()> {
    manager.remove(id, "api_disconnect").await?;
    tracing::info!("IRC connection '{}' disconnected", id);
    Ok(())
}

/// Get the connection state (returns disconnected for missing connections).
pub async fn state(manager: &IrcManager, id: &str) -> ConnectionStateResponse {
    let (state, ready) = match manager.state(id).await {
        Ok(s) => (s.to_string(), s == ConnectionState::Ready),
        Err(_) => ("disconnected".to_string(), false),
    };
    ConnectionStateResponse {
        id: id.to_string(),
        state,
        ready,
    }
}

/// Wait for a connection to become ready.
pub async fn wait_ready(manager: &IrcManager, id: &str, timeout_secs: Option<u64>) -> Result<()> {
    if let Some(secs) = timeout_secs {
        manager
            .wait_for_ready_with_timeout(id, std::time::Duration::from_secs(secs))
            .await?;
    } else {
        manager.wait_for_ready(id).await?;
    }
    tracing::info!("IRC connection '{}' is ready", id);
    Ok(())
}

/// Join a channel.
pub async fn join(manager: &IrcManager, id: &str, channel: &str) -> Result<()> {
    manager.join(id, channel).await?;
    tracing::debug!("IRC connection '{}' joined '{}'", id, channel);
    Ok(())
}

/// Part (leave) a channel.
pub async fn part(manager: &IrcManager, id: &str, channel: &str) -> Result<()> {
    manager.part(id, channel).await?;
    tracing::debug!("IRC connection '{}' parted '{}'", id, channel);
    Ok(())
}

/// Add a channel to the auto-join set and join it if connected.
pub async fn auto_join(manager: &IrcManager, id: &str, channel: &str) -> Result<()> {
    manager.add_auto_join(id, channel).await?;
    tracing::debug!("IRC connection '{}' auto-join added '{}'", id, channel);
    Ok(())
}

/// Remove a channel from the auto-join set and part it.
pub async fn auto_leave(manager: &IrcManager, id: &str, channel: &str) -> Result<()> {
    manager.remove_auto_join(id, channel).await?;
    tracing::debug!("IRC connection '{}' auto-join removed '{}'", id, channel);
    Ok(())
}

/// Send a message and store it. Returns the stored message for event emission,
/// or `None` if `echo-message` is negotiated (the server echo handles storage).
pub async fn send_message(
    manager: &IrcManager,
    id: &str,
    target: &str,
    message: &str,
    reply_to: Option<&str>,
) -> Result<Option<(StoredMessage, bool)>> {
    manager.send_message(id, target, message, reply_to).await?;

    // For DMs, ensure the target nick appears as a channel in the sidebar
    let is_dm = !target.starts_with('#') && target != "*";
    if is_dm {
        manager.store_add_channel(id, target).await;
    }

    tracing::debug!("IRC connection '{}' sent message to '{}'", id, target);

    // With echo-message, the server echoes our message back with a proper msgid.
    // The echo will be processed by process_event_for_store, so skip local storage.
    if manager.has_capability(id, "echo-message").await {
        return Ok(None);
    }

    let nick = manager.nick(id).await.unwrap_or_default();
    let stored = manager
        .store_outgoing(id, target, &nick, message, None, reply_to)
        .await;
    Ok(Some((stored, is_dm)))
}

/// Send a message with a data payload and store it. Returns the stored message
/// for event emission, or `None` if `echo-message` is negotiated.
pub async fn send_message_with_data(
    manager: &IrcManager,
    id: &str,
    target: &str,
    message: &str,
    data: &str,
    reply_to: Option<&str>,
) -> Result<Option<StoredMessage>> {
    manager
        .send_message_with_data(id, target, message, data, reply_to)
        .await?;

    tracing::debug!(
        "IRC connection '{}' sent message with data to '{}'",
        id,
        target
    );

    // With echo-message, the server echoes our message back with a proper msgid.
    // The echo will be processed by process_event_for_store, so skip local storage.
    if manager.has_capability(id, "echo-message").await {
        return Ok(None);
    }

    let nick = manager.nick(id).await.unwrap_or_default();
    let stored = manager
        .store_outgoing(id, target, &nick, message, Some(data), reply_to)
        .await;

    // Index reactions we send ourselves (no echo-message path).
    if let Some((msg_id, reaction, remove)) = crate::lifecycle::try_extract_message_reaction(data) {
        if remove {
            manager
                .remove_message_reaction(id, &msg_id, &nick, &reaction)
                .await;
        } else {
            manager
                .store_message_reaction(id, &msg_id, &nick, &reaction)
                .await;
        }
        // Derive commit-reaction index if the target is a shared-commit.
        if let Some(target_data) = manager.find_message_data_by_msgid(id, &msg_id).await
            && let Some(commit_id) = crate::lifecycle::try_extract_shared_commit_id(&target_data)
        {
            if remove {
                manager
                    .remove_reaction(id, &commit_id, &nick, &reaction)
                    .await;
            } else {
                manager
                    .store_reaction(id, &commit_id, &nick, &reaction)
                    .await;
            }
        }
    }

    Ok(Some(stored))
}

/// Get all commit reactions for a connection, keyed by commit ID.
pub async fn get_all_commit_reactions(
    manager: &IrcManager,
    id: &str,
) -> HashMap<String, Vec<Reaction>> {
    manager.get_all_reactions(id).await
}

/// Get all message reactions for a connection, keyed by message ID.
pub async fn get_all_message_reactions(
    manager: &IrcManager,
    id: &str,
) -> HashMap<String, Vec<Reaction>> {
    manager.get_all_message_reactions(id).await
}

/// Get the current working files for all users in a channel.
/// Returns a map of nick → list of file paths.
pub async fn get_working_files(
    manager: &IrcManager,
    id: &str,
    channel: &str,
) -> HashMap<String, Vec<String>> {
    manager.get_working_files(id, channel).await
}

/// List all connection IDs.
pub async fn list_connections(manager: &IrcManager) -> Vec<ConnectionId> {
    manager.list_connections().await
}

/// Check whether a connection exists.
pub async fn exists(manager: &IrcManager, id: &str) -> bool {
    manager.exists(id).await
}

/// Get the nickname for a connection.
pub async fn nick(manager: &IrcManager, id: &str) -> Result<String> {
    manager.nick(id).await
}

/// Get stored messages for a channel.
pub async fn messages(manager: &IrcManager, id: &str, channel: &str) -> Vec<StoredMessage> {
    manager.get_messages(id, channel).await
}

/// Get the list of channels.
pub async fn channels(manager: &IrcManager, id: &str) -> Vec<ChannelInfo> {
    manager.get_channels(id).await
}

/// Get the user list for a channel.
pub async fn users(
    manager: &IrcManager,
    id: &str,
    channel: &str,
) -> Vec<crate::message_store::UserEntry> {
    manager.get_users(id, channel).await
}

/// Clear stored messages for a channel.
pub async fn clear_messages(manager: &IrcManager, id: &str, channel: &str) {
    manager.store_clear_messages(id, channel).await;
}

/// Mark a channel as read.
///
/// Updates the local store and sends a MARKREAD command to the IRC server
/// so the read position is persisted server-side (if `draft/read-marker`
/// was negotiated).
pub async fn mark_read(manager: &IrcManager, id: &str, channel: &str) {
    manager.store_mark_read(id, channel).await;

    // Send MARKREAD to the server with the current timestamp.
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    if let Err(e) = manager.send_markread(id, channel, Some(&ts)).await {
        tracing::debug!(
            %channel,
            error = %e,
            "Failed to send MARKREAD to server (cap may not be negotiated)",
        );
    }
}

/// Request chat history for a channel.
pub async fn request_history(
    manager: &IrcManager,
    id: &str,
    channel: &str,
    limit: Option<u32>,
) -> Result<()> {
    manager
        .request_history(id, channel, limit.unwrap_or(DEFAULT_HISTORY_LIMIT))
        .await?;
    tracing::debug!(
        "IRC connection '{}' requested history for '{}'",
        id,
        channel
    );
    Ok(())
}

/// Request older chat history before a given timestamp.
pub async fn request_history_before(
    manager: &IrcManager,
    id: &str,
    channel: &str,
    before: &str,
    limit: Option<u32>,
) -> Result<()> {
    manager
        .request_history_before(id, channel, before, limit.unwrap_or(DEFAULT_HISTORY_LIMIT))
        .await?;
    tracing::debug!(
        "IRC connection '{}' requested history before '{}' for '{}'",
        id,
        before,
        channel
    );
    Ok(())
}

/// Send a raw IRC command.
///
/// Rejects commands containing CR or LF to prevent IRC protocol injection.
pub async fn send_raw(manager: &IrcManager, id: &str, command: &str) -> Result<()> {
    if command.contains('\r') || command.contains('\n') {
        return Err(IrcError::other(
            "Raw command must not contain CR or LF characters".to_string(),
        ));
    }
    manager.send_raw(id, command).await?;
    tracing::debug!("IRC connection '{}' sent raw command", id);
    Ok(())
}

/// Send a typing indicator via TAGMSG.
pub async fn send_typing(manager: &IrcManager, id: &str, target: &str, state: &str) -> Result<()> {
    manager
        .send_tagmsg(
            id,
            target,
            vec![("+typing".to_string(), Some(state.to_string()))],
        )
        .await?;
    Ok(())
}

/// Send a reaction to a message via TAGMSG.
pub async fn send_reaction(
    manager: &IrcManager,
    id: &str,
    target: &str,
    msgid: &str,
    emoji: &str,
) -> Result<()> {
    manager
        .send_tagmsg(
            id,
            target,
            vec![
                ("+draft/react".to_string(), Some(emoji.to_string())),
                ("+draft/reply".to_string(), Some(msgid.to_string())),
            ],
        )
        .await?;
    Ok(())
}

/// Remove a reaction from a message via TAGMSG.
pub async fn remove_reaction(
    manager: &IrcManager,
    id: &str,
    target: &str,
    msgid: &str,
    emoji: &str,
) -> Result<()> {
    manager
        .send_tagmsg(
            id,
            target,
            vec![
                ("+draft/react".to_string(), Some(format!("-{}", emoji))),
                ("+draft/reply".to_string(), Some(msgid.to_string())),
            ],
        )
        .await?;
    Ok(())
}

/// Redact (delete) a message via draft/message-redaction.
pub async fn redact_message(
    manager: &IrcManager,
    id: &str,
    target: &str,
    msgid: &str,
    reason: Option<&str>,
) -> Result<()> {
    let cmd = if let Some(reason) = reason {
        format!("REDACT {} {} :{}", target, msgid, reason)
    } else {
        format!("REDACT {} {}", target, msgid)
    };
    manager.send_raw(id, &cmd).await?;
    Ok(())
}

/// Get message reactions for a specific file, keyed by hunk key
/// (formatted as `"oldStart:oldLines:newStart:newLines"`).
pub async fn get_file_message_reactions(
    manager: &IrcManager,
    id: &str,
    file_path: &str,
) -> HashMap<String, Vec<Reaction>> {
    manager.get_file_message_reactions(id, file_path).await
}
