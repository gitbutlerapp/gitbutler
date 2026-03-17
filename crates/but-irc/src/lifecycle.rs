//! Shared IRC connection lifecycle management.
//!
//! Contains the core logic for auto-connecting IRC connections, reacting to
//! settings changes, and processing IRC events into the MessageStore.
//!
//! Both `gitbutler-tauri` and `but-server` use this module, providing their
//! own [`EventEmitter`] implementation for their respective frontend transport
//! (Tauri events vs WebSocket broadcasts).

use crate::message_store::{MessageDirection, StoredMessage, now_millis, parse_iso8601_millis};
use crate::{DEFAULT_HISTORY_LIMIT, IrcConfig, IrcEvent, IrcManager};
use but_settings::app_settings::{IrcConnectionSettings, IrcServerSettings, IrcSettings};
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, error, info};

// ============================================================================
// Event emission abstraction
// ============================================================================

/// Trait for emitting IRC events to a frontend.
///
/// Implementors wrap a transport mechanism (Tauri `AppHandle`, WebSocket
/// `Broadcaster`, etc.) and forward named events with JSON payloads.
pub trait EventEmitter: Send + Sync + Clone + 'static {
    /// Emit a named event with a JSON payload to the frontend.
    fn emit(&self, name: &str, payload: serde_json::Value);
}

// ============================================================================
// Settings → config conversion
// ============================================================================

/// Build an [`IrcConfig`] from settings, returning `None` if the server host is empty.
pub fn build_irc_config(
    server: &IrcServerSettings,
    conn: &IrcConnectionSettings,
    default_nick: &str,
) -> Option<IrcConfig> {
    let nick = conn
        .nickname
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default_nick);

    if server.host.is_empty() {
        debug!("Skipping IRC connection: no server host configured");
        return None;
    }

    let server_password = conn
        .server_password
        .clone()
        .filter(|s| !s.trim().is_empty());
    let sasl_password = conn.sasl_password.clone().filter(|s| !s.trim().is_empty());

    if server_password.is_none() {
        debug!("Skipping IRC connection: server password not configured");
        return None;
    }
    if sasl_password.is_none() {
        debug!("Skipping IRC connection: account password not configured");
        return None;
    }

    Some(IrcConfig {
        server: server.host.clone(),
        port: server.port,
        use_tls: true,
        nick: nick.to_string(),
        server_password,
        sasl_password,
        username: Some(nick.to_string()),
        realname: conn
            .realname
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| Some(nick.to_string())),
    })
}

// ============================================================================
// Settings reconciliation
// ============================================================================

/// What action to take for a connection when settings change.
#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionAction {
    /// No change needed.
    NoOp,
    /// Create and connect.
    Connect,
    /// Disconnect and remove.
    Disconnect,
    /// Disconnect, then reconnect with new config.
    Reconnect,
}

/// Determine what action to take for a connection based on old/new settings.
///
/// Pure function — no side effects, fully testable.
pub fn compute_reconciliation(
    old_server: &IrcServerSettings,
    old_conn: &IrcConnectionSettings,
    new_server: &IrcServerSettings,
    new_conn: &IrcConnectionSettings,
) -> ConnectionAction {
    let was_active = old_conn.enabled;
    let should_be_active = new_conn.enabled;

    if !was_active && should_be_active {
        ConnectionAction::Connect
    } else if was_active && !should_be_active {
        ConnectionAction::Disconnect
    } else if was_active && should_be_active {
        let config_changed = old_server != new_server
            || old_conn.nickname != new_conn.nickname
            || old_conn.server_password != new_conn.server_password
            || old_conn.sasl_password != new_conn.sasl_password
            || old_conn.realname != new_conn.realname;

        if config_changed {
            ConnectionAction::Reconnect
        } else {
            ConnectionAction::NoOp
        }
    } else {
        ConnectionAction::NoOp
    }
}

// ============================================================================
// Connection startup & event forwarding
// ============================================================================

/// Auto-connect IRC connections based on current settings.
///
/// Called once at startup. Connects if the connection is enabled.
pub fn auto_connect_on_startup(
    manager: &IrcManager,
    emitter: &impl EventEmitter,
    irc_settings: &IrcSettings,
) {
    if irc_settings.connection.enabled {
        let config = build_irc_config(
            &irc_settings.server,
            &irc_settings.connection,
            "personal-irc",
        );
        if let Some(config) = config {
            let manager = manager.clone();
            let emitter = emitter.clone();
            tokio::spawn(async move {
                start_connection(
                    manager,
                    emitter,
                    "personal-irc".to_string(),
                    config,
                    "auto_connect_on_startup",
                )
                .await;
            });
        }
    }
}

/// React to IRC settings changes by connecting/disconnecting/reconnecting as needed.
pub fn on_settings_changed(
    manager: &IrcManager,
    emitter: &impl EventEmitter,
    old: &IrcSettings,
    new: &IrcSettings,
) {
    info!("on_settings_changed called, diffing IRC settings");
    reconcile_connection(
        manager,
        emitter,
        "personal-irc",
        &old.server,
        &old.connection,
        &new.server,
        &new.connection,
    );
}

/// Reconcile a single connection's settings.
#[allow(clippy::too_many_arguments)]
fn reconcile_connection(
    manager: &IrcManager,
    emitter: &impl EventEmitter,
    connection_id: &str,
    old_server: &IrcServerSettings,
    old_conn: &IrcConnectionSettings,
    new_server: &IrcServerSettings,
    new_conn: &IrcConnectionSettings,
) {
    match compute_reconciliation(old_server, old_conn, new_server, new_conn) {
        ConnectionAction::NoOp => {}
        ConnectionAction::Connect => {
            info!("IRC connection '{}' enabled, connecting", connection_id);
            let config = build_irc_config(new_server, new_conn, connection_id);
            if let Some(config) = config {
                let manager = manager.clone();
                let emitter = emitter.clone();
                let id = connection_id.to_string();
                tokio::spawn(async move {
                    start_connection(manager, emitter, id, config, "settings_connect").await;
                });
            }
        }
        ConnectionAction::Disconnect => {
            info!("IRC connection '{}' disabled, disconnecting", connection_id);
            let manager = manager.clone();
            let id = connection_id.to_string();
            tokio::spawn(async move {
                let _ = manager.remove(&id, "settings_disabled").await;
            });
        }
        ConnectionAction::Reconnect => {
            info!(
                "IRC connection '{}' config changed, reconnecting",
                connection_id
            );
            let config = build_irc_config(new_server, new_conn, connection_id);
            if let Some(config) = config {
                let manager = manager.clone();
                let emitter = emitter.clone();
                let id = connection_id.to_string();
                tokio::spawn(async move {
                    let _ = manager.remove(&id, "settings_reconnect").await;
                    start_connection(manager, emitter, id, config, "settings_reconnect").await;
                });
            }
        }
    }
}

/// Create a connection and spawn an event forwarder.
async fn start_connection(
    manager: IrcManager,
    emitter: impl EventEmitter,
    id: String,
    config: IrcConfig,
    caller: &'static str,
) {
    info!("start_connection called for '{}' by {}", id, caller);
    if let Err(e) = manager.create_and_connect(id.clone(), config).await {
        // Log the error but don't return — the reconnection watcher (spawned inside
        // create_and_connect) will keep retrying. We still need to set up the event forwarder.
        error!(
            "Failed to auto-connect IRC '{}': {} (reconnection watcher will retry)",
            id, e
        );
    } else {
        info!("create_and_connect completed for '{}'", id);
        match manager.negotiated_caps(&id).await {
            Ok(caps) => info!(connection_id = %id, ?caps, "Negotiated capabilities"),
            Err(e) => info!(connection_id = %id, error = %e, "Could not read negotiated caps"),
        }
    }

    // Register the event forwarder spawner so the reconnection watcher
    // can re-establish full event processing after channel recreation.
    let emitter_for_spawner = emitter.clone();
    manager
        .set_event_forwarder_spawner(move |mgr, conn_id| {
            let em = emitter_for_spawner.clone();
            tokio::spawn(async move {
                spawn_event_forwarder(&mgr, &em, &conn_id).await;
            });
        })
        .await;

    spawn_event_forwarder(&manager, &emitter, &id).await;

    info!("IRC connection '{}' started (event forwarder active)", id);
}

/// Spawn a task that forwards IRC events to the frontend.
///
/// Takes ownership of the event receiver for the given connection and
/// processes each event: stores data in the MessageStore, emits
/// granular frontend events, and handles chat history batches.
pub async fn spawn_event_forwarder(manager: &IrcManager, emitter: &impl EventEmitter, id: &str) {
    let Ok(Some(mut events)) = manager.take_event_receiver(id).await else {
        return;
    };
    let connection_id = id.to_string();
    let mgr = manager.clone();
    let emitter = emitter.clone();

    tokio::spawn(async move {
        // Track active chathistory batch IDs → channel name.
        let mut history_batch_channels: HashMap<String, String> = HashMap::new();
        // Buffer history messages per batch ID until BatchEnd.
        let mut history_batch_buffer: HashMap<String, Vec<StoredMessage>> = HashMap::new();
        // Track active draft/multiline batch IDs → buffered IrcEvent lines.
        let mut multiline_batches: HashMap<String, Vec<IrcEvent>> = HashMap::new();

        while let Some(event) = events.recv().await {
            if matches!(&event, IrcEvent::Away { .. }) {
                debug!(
                    connection_id = %connection_id,
                    "AWAY event arrived in event forwarder: {:?}", event,
                );
            }
            debug!(
                connection_id = %connection_id,
                event_type = %event.event_type(),
                "Forwarding IRC event to frontend",
            );

            // Track chathistory batch boundaries
            match &event {
                IrcEvent::BatchStart { id, batch_type, .. }
                    if batch_type.eq_ignore_ascii_case("draft/multiline") =>
                {
                    debug!(batch_id = %id, "Multiline batch started");
                    multiline_batches.insert(id.clone(), Vec::new());
                }
                IrcEvent::BatchStart {
                    id,
                    batch_type,
                    params,
                } if batch_type.eq_ignore_ascii_case("chathistory") => {
                    let channel = params.first().cloned().unwrap_or_default();
                    debug!(batch_id = %id, %channel, "Chat history replay started");
                    history_batch_channels.insert(id.clone(), channel);
                    history_batch_buffer.insert(id.clone(), Vec::new());
                }
                IrcEvent::BatchEnd { id } if multiline_batches.contains_key(id) => {
                    if let Some(lines) = multiline_batches.remove(id) {
                        debug!(batch_id = %id, line_count = lines.len(), "Multiline batch ended, merging");
                        if let Some(merged) = merge_multiline_batch(lines) {
                            // Process the merged event as if it were a single message
                            process_event_for_store(&mgr, &emitter, &connection_id, &merged).await;
                        }
                    }
                    continue;
                }
                IrcEvent::BatchEnd { id } => {
                    if let Some(channel) = history_batch_channels.remove(id) {
                        let batch = history_batch_buffer.remove(id).unwrap_or_default();
                        let count = batch.len();
                        debug!(batch_id = %id, %channel, %count, "Chat history replay ended, flushing batch");

                        // Extract reactions and hunk share index from history batch.
                        for msg in &batch {
                            if let Some(data) = msg.data.as_deref() {
                                // Legacy commit-reaction (direct commitId keying)
                                if let Some((commit_id, reaction)) =
                                    try_extract_legacy_commit_reaction(data)
                                {
                                    mgr.store_reaction(
                                        &connection_id,
                                        &commit_id,
                                        &msg.sender,
                                        &reaction,
                                    )
                                    .await;
                                    emitter.emit(
                                        &format!("irc:{}:commit-reaction", connection_id),
                                        json!({ "commitId": commit_id }),
                                    );
                                } else if let Some((msg_id, reaction, remove)) =
                                    try_extract_message_reaction(data)
                                {
                                    if remove {
                                        mgr.remove_message_reaction(
                                            &connection_id,
                                            &msg_id,
                                            &msg.sender,
                                            &reaction,
                                        )
                                        .await;
                                    } else {
                                        mgr.store_message_reaction(
                                            &connection_id,
                                            &msg_id,
                                            &msg.sender,
                                            &reaction,
                                        )
                                        .await;
                                    }
                                    emitter.emit(
                                        &format!("irc:{}:message-reaction", connection_id),
                                        json!({ "msgId": msg_id }),
                                    );
                                    // Derive commit-reaction index if the target is a shared-commit.
                                    let target_data = batch
                                        .iter()
                                        .find(|m| m.msgid.as_deref() == Some(msg_id.as_str()))
                                        .and_then(|m| m.data.as_deref());
                                    if let Some(commit_id) =
                                        target_data.and_then(try_extract_shared_commit_id)
                                    {
                                        if remove {
                                            mgr.remove_reaction(
                                                &connection_id,
                                                &commit_id,
                                                &msg.sender,
                                                &reaction,
                                            )
                                            .await;
                                        } else {
                                            mgr.store_reaction(
                                                &connection_id,
                                                &commit_id,
                                                &msg.sender,
                                                &reaction,
                                            )
                                            .await;
                                        }
                                        emitter.emit(
                                            &format!("irc:{}:commit-reaction", connection_id),
                                            json!({ "commitId": commit_id }),
                                        );
                                    }
                                }
                                // Index hunk shares so we can look up reactions by file path
                                if let Some(ref mid) = msg.msgid
                                    && let Some((file_path, hunk_key)) =
                                        try_extract_hunk_share(data)
                                {
                                    mgr.index_hunk_share(
                                        &connection_id,
                                        &file_path,
                                        &hunk_key,
                                        mid,
                                    )
                                    .await;
                                }
                            }
                        }

                        // Store the batch — only genuinely new messages are returned.
                        let inserted = mgr
                            .store_history_batch(&connection_id, &channel, batch)
                            .await;

                        // Emit only non-reaction, newly-inserted messages to the frontend.
                        let visible: Vec<_> = inserted
                            .into_iter()
                            .filter(|m| {
                                m.data.as_deref().is_none_or(|d| {
                                    try_extract_legacy_commit_reaction(d).is_none()
                                        && try_extract_message_reaction(d).is_none()
                                })
                            })
                            .collect();
                        emitter.emit(
                            &format!("irc:{}:history-batch", connection_id),
                            json!({ "channel": channel, "messages": visible }),
                        );
                    }
                }
                // Auto-request chat history when we join a channel.
                // Skip if the join arrived inside a chathistory batch — that
                // means it is a historical replay (draft/event-playback), not
                // a live join, so requesting history again would cause a loop.
                IrcEvent::UserJoined {
                    channel,
                    nick,
                    batch,
                    ..
                } => {
                    if batch.is_none()
                        && let Ok(our_nick) = mgr.nick(&connection_id).await
                        && nick == &our_nick
                    {
                        debug!(%channel, "Requesting chat history after join");
                        if let Err(e) = mgr
                            .request_history(&connection_id, channel, DEFAULT_HISTORY_LIMIT)
                            .await
                        {
                            debug!(%channel, error = %e, "Failed to request history after join");
                        }
                        // Request WHO to get initial away status for all users
                        // in the channel. Live updates are handled by away-notify.
                        if let Err(e) = mgr
                            .send_raw(&connection_id, &format!("WHO {channel}"))
                            .await
                        {
                            debug!(%channel, error = %e, "Failed to request WHO after join");
                        }
                        // Query server for the stored read marker so we can
                        // initialize unread counts from persisted state.
                        if let Err(e) = mgr.send_markread(&connection_id, channel, None).await {
                            debug!(%channel, error = %e, "Failed to query MARKREAD after join");
                        }
                    }
                }
                // Server-side read marker update — sync local read position.
                IrcEvent::MarkRead { target, timestamp } => {
                    if let Some(ts) = timestamp.as_deref().and_then(parse_iso8601_millis) {
                        mgr.store_mark_read_at(&connection_id, target, ts).await;
                        emitter.emit(&format!("irc:{}:channels", connection_id), json!(null));
                    }
                }
                // Auto-join channels when connection becomes ready (including after reconnect)
                IrcEvent::StateChanged { state } if state == "ready" => {
                    let channels = mgr.get_auto_join_channels(&connection_id).await;
                    for channel in &channels {
                        if let Err(e) = mgr.join(&connection_id, channel).await {
                            debug!(%channel, error = %e, "Failed to auto-join channel on ready");
                        }
                    }
                }
                _ => {}
            }

            // Buffer messages belonging to a multiline batch
            let multiline_buffered = try_buffer_multiline_message(&event, &mut multiline_batches);

            if multiline_buffered {
                continue;
            }

            // Check if this event is a history message that should be buffered
            let buffered = try_buffer_history_message(
                &event,
                &connection_id,
                &history_batch_channels,
                &mut history_batch_buffer,
            );

            if !buffered {
                // Not a buffered history message — process normally
                process_event_for_store(&mgr, &emitter, &connection_id, &event).await;
            }

            // Emit state changes for connection-level events.
            let state_change = match &event {
                IrcEvent::StateChanged { state } => Some(state.as_str()),
                IrcEvent::Welcome { .. } => Some("ready"),
                IrcEvent::Error { code, .. } if code == "ERROR" => Some("error"),
                _ => None,
            };
            if let Some(state) = state_change {
                let state_event = format!("irc:{}:state", connection_id);
                emitter.emit(&state_event, json!(state));
            }
        }
        tracing::warn!(
            "IRC event forwarder for '{}' ended (all senders dropped — channel will be recreated on next reconnect)",
            connection_id
        );
    });
}

// ============================================================================
// Data payload decoding
// ============================================================================

/// Parse a JSON data payload, returning the parsed JSON value.
fn decode_data_payload(data: &str) -> Option<serde_json::Value> {
    serde_json::from_str(data).ok()
}

/// Try to extract a commit ID from a shared-commit data payload.
///
/// Shared commits have `{ "type": "shared-commit", "payload": { "commit": { "commitId": "..." } } }`.
pub(crate) fn try_extract_shared_commit_id(data: &str) -> Option<String> {
    let json = decode_data_payload(data)?;
    if json["type"].as_str()? != "shared-commit" {
        return None;
    }
    json["payload"]["commit"]["commitId"]
        .as_str()
        .map(|s| s.to_string())
}

/// Try to decode a message's base64+JSON data payload as a message reaction.
///
/// Accepts both `message-reaction` (current) and `hunk-reaction` (legacy).
/// Returns `(msg_id, reaction, remove)`.
pub(crate) fn try_extract_message_reaction(data: &str) -> Option<(String, String, bool)> {
    let json = decode_data_payload(data)?;
    let rtype = json["type"].as_str()?;
    if rtype != "message-reaction" && rtype != "hunk-reaction" {
        return None;
    }
    // Accept both `msgId` (current) and `shareMsgId` (legacy)
    let msg_id = json["payload"]["msgId"]
        .as_str()
        .or_else(|| json["payload"]["shareMsgId"].as_str())?
        .to_string();
    let reaction = json["payload"]["reaction"].as_str()?.to_string();
    let remove = json["payload"]["remove"].as_bool().unwrap_or(false);
    Some((msg_id, reaction, remove))
}

/// Try to decode a legacy `commit-reaction` payload.
///
/// Returns `(commit_id, reaction)`. Only needed for backwards compat with old history.
fn try_extract_legacy_commit_reaction(data: &str) -> Option<(String, String)> {
    let json = decode_data_payload(data)?;
    if json["type"].as_str()? != "commit-reaction" {
        return None;
    }
    let commit_id = json["payload"]["commitId"].as_str()?.to_string();
    let reaction = json["payload"]["reaction"].as_str()?.to_string();
    Some((commit_id, reaction))
}

/// Try to decode a message's base64+JSON data payload as a hunk share.
///
/// Hunk shares have no `type` field — they contain `{ change: { path }, diff: { oldStart, oldLines, newStart, newLines } }`.
/// Returns `(file_path, hunk_key)` where `hunk_key` is `"oldStart:oldLines:newStart:newLines"`.
pub(crate) fn try_extract_hunk_share(data: &str) -> Option<(String, String)> {
    let json = decode_data_payload(data)?;
    if json.get("type").is_some() {
        return None;
    }
    let file_path = json["change"]["path"].as_str()?;
    let old_start = json["diff"]["oldStart"].as_u64()?;
    let old_lines = json["diff"]["oldLines"].as_u64()?;
    let new_start = json["diff"]["newStart"].as_u64()?;
    let new_lines = json["diff"]["newLines"].as_u64()?;
    let hunk_key = format!("{old_start}:{old_lines}:{new_start}:{new_lines}");
    Some((file_path.to_string(), hunk_key))
}

/// Try to decode a working-files-sync payload.
/// Returns the file list if `{ type: "working-files-sync", files: [...] }`.
pub(crate) fn try_extract_working_files_sync(data: &str) -> Option<Vec<String>> {
    let json = decode_data_payload(data)?;
    if json["type"].as_str()? != "working-files-sync" {
        return None;
    }
    let files = json["files"]
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_owned()))
        .collect();
    Some(files)
}

/// Try to decode a working-files-delta payload.
/// Returns `(added, removed)` if `{ type: "working-files-delta", added: [...], removed: [...] }`.
pub(crate) fn try_extract_working_files_delta(data: &str) -> Option<(Vec<String>, Vec<String>)> {
    let json = decode_data_payload(data)?;
    if json["type"].as_str()? != "working-files-delta" {
        return None;
    }
    let to_string_vec = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let added = to_string_vec(&json["added"]);
    let removed = to_string_vec(&json["removed"]);
    Some((added, removed))
}

// ============================================================================
// History batch buffering
// ============================================================================

/// If the event is a channel/private message inside an active history batch,
/// build a `StoredMessage` and buffer it. Returns `true` if the message was
/// buffered (so the caller should skip normal processing).
fn try_buffer_history_message(
    event: &IrcEvent,
    connection_id: &str,
    history_batch_channels: &HashMap<String, String>,
    history_batch_buffer: &mut HashMap<String, Vec<StoredMessage>>,
) -> bool {
    match event {
        IrcEvent::ChannelMessage {
            from,
            channel,
            text,
            data,
            server_time,
            batch,
            msgid,
            reply_to,
        } => {
            if let Some(batch_id) = batch
                .as_ref()
                .filter(|b| history_batch_channels.contains_key(b.as_str()))
            {
                let parsed = server_time.as_deref().and_then(parse_iso8601_millis);
                let timestamp = parsed.unwrap_or_else(now_millis);
                let msg = StoredMessage {
                    sender: from.clone(),
                    content: text.clone(),
                    data: data.clone(),
                    timestamp,
                    direction: MessageDirection::Incoming,
                    target: channel.clone(),
                    is_history: true,
                    msgid: msgid.clone(),
                    reply_to: reply_to.clone(),
                    tag: None,
                };
                if let Some(buf) = history_batch_buffer.get_mut(batch_id.as_str()) {
                    debug!(
                        %connection_id, channel, from,
                        server_time = server_time.as_deref().unwrap_or("none"),
                        "Buffering history message",
                    );
                    buf.push(msg);
                }
                return true;
            }
        }
        IrcEvent::PrivateMessage {
            from,
            target: pm_target,
            text,
            data,
            server_time,
            batch,
            msgid,
            reply_to,
        } => {
            if let Some(batch_id) = batch
                .as_ref()
                .filter(|b| history_batch_channels.contains_key(b.as_str()))
            {
                let parsed = server_time.as_deref().and_then(parse_iso8601_millis);
                let timestamp = parsed.unwrap_or_else(now_millis);
                let msg = StoredMessage {
                    sender: from.clone(),
                    content: text.clone(),
                    data: data.clone(),
                    timestamp,
                    direction: MessageDirection::Incoming,
                    target: pm_target.clone(),
                    is_history: true,
                    msgid: msgid.clone(),
                    reply_to: reply_to.clone(),
                    tag: None,
                };
                if let Some(buf) = history_batch_buffer.get_mut(batch_id.as_str()) {
                    debug!(
                        %connection_id, from,
                        server_time = server_time.as_deref().unwrap_or("none"),
                        "Buffering history private message",
                    );
                    buf.push(msg);
                }
                return true;
            }
        }
        // With `draft/event-playback`, JOIN/PART/QUIT/NICK arrive as real
        // protocol events inside chathistory batches.  Buffer them as
        // display-only system messages but do NOT mutate user lists.
        // Skip join/part/quit from history replay — they're noise in the chat timeline.
        IrcEvent::UserJoined { batch, .. }
        | IrcEvent::UserParted { batch, .. }
        | IrcEvent::UserQuit { batch, .. } => {
            if batch
                .as_ref()
                .is_some_and(|b| history_batch_channels.contains_key(b.as_str()))
            {
                return true;
            }
        }
        IrcEvent::NickChanged {
            old_nick,
            new_nick,
            batch,
            server_time,
            msgid,
        } => {
            if let Some(batch_id) = batch
                .as_ref()
                .filter(|b| history_batch_channels.contains_key(b.as_str()))
            {
                let parsed = server_time.as_deref().and_then(parse_iso8601_millis);
                let timestamp = parsed.unwrap_or_else(now_millis);
                let target = history_batch_channels
                    .get(batch_id.as_str())
                    .cloned()
                    .unwrap_or_default();
                let msg = StoredMessage {
                    sender: "*".to_string(),
                    content: format!("{old_nick} is now known as {new_nick}"),
                    data: None,
                    timestamp,
                    direction: MessageDirection::Incoming,
                    target,
                    is_history: true,
                    msgid: msgid.clone(),
                    reply_to: None,
                    tag: Some("nick".to_string()),
                };
                if let Some(buf) = history_batch_buffer.get_mut(batch_id.as_str()) {
                    buf.push(msg);
                }
                return true;
            }
        }
        // Typing indicators from history are stale — silently discard them.
        IrcEvent::Typing { batch, .. } => {
            if batch
                .as_ref()
                .is_some_and(|b| history_batch_channels.contains_key(b.as_str()))
            {
                return true;
            }
        }
        // TAGMSG reactions from history: let them fall through to
        // process_event_for_store so reactions get indexed.
        IrcEvent::TagReaction { batch, .. } => {
            if batch
                .as_ref()
                .is_some_and(|b| history_batch_channels.contains_key(b.as_str()))
            {
                return false;
            }
        }
        _ => {}
    }
    false
}

// ============================================================================
// draft/multiline batch helpers
// ============================================================================

/// If the event is a message inside an active multiline batch, buffer it.
/// Returns `true` if the message was buffered.
fn try_buffer_multiline_message(
    event: &IrcEvent,
    multiline_batches: &mut HashMap<String, Vec<IrcEvent>>,
) -> bool {
    let batch_id = match event {
        IrcEvent::ChannelMessage { batch: Some(b), .. } => b,
        IrcEvent::PrivateMessage { batch: Some(b), .. } => b,
        _ => return false,
    };

    if let Some(buf) = multiline_batches.get_mut(batch_id.as_str()) {
        buf.push(event.clone());
        true
    } else {
        false
    }
}

/// Merge buffered multiline batch messages into a single IrcEvent.
///
/// Takes the metadata (from, channel, data, server_time, msgid, reply_to)
/// from the first message and concatenates all text lines with newlines.
fn merge_multiline_batch(lines: Vec<IrcEvent>) -> Option<IrcEvent> {
    if lines.is_empty() {
        return None;
    }

    // Collect all text lines
    let texts: Vec<&str> = lines
        .iter()
        .filter_map(|e| match e {
            IrcEvent::ChannelMessage { text, .. } => Some(text.as_str()),
            IrcEvent::PrivateMessage { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    let merged_text = texts.join("\n");

    // Use the first message's metadata
    match &lines[0] {
        IrcEvent::ChannelMessage {
            from,
            channel,
            data,
            server_time,
            msgid,
            reply_to,
            ..
        } => Some(IrcEvent::ChannelMessage {
            from: from.clone(),
            channel: channel.clone(),
            text: merged_text,
            data: data.clone(),
            server_time: server_time.clone(),
            batch: None, // merged — no longer in a batch
            msgid: msgid.clone(),
            reply_to: reply_to.clone(),
        }),
        IrcEvent::PrivateMessage {
            from,
            target: pm_target,
            data,
            server_time,
            msgid,
            reply_to,
            ..
        } => Some(IrcEvent::PrivateMessage {
            from: from.clone(),
            target: pm_target.clone(),
            text: merged_text,
            data: data.clone(),
            server_time: server_time.clone(),
            batch: None,
            msgid: msgid.clone(),
            reply_to: reply_to.clone(),
        }),
        _ => None,
    }
}

// ============================================================================
// Event → MessageStore processing
// ============================================================================

/// Process the `+data` payload of a stored message: extract reactions (legacy
/// commit reactions, message reactions with optional commit-reaction derivation),
/// index hunk shares, and emit the corresponding events.
///
/// Returns `true` if the message was a reaction (and should NOT be emitted as a
/// regular message event).
async fn process_message_data(
    manager: &IrcManager,
    emitter: &impl EventEmitter,
    connection_id: &str,
    from: &str,
    stored: &StoredMessage,
    data: &str,
) -> bool {
    let mut is_reaction = false;

    if let Some((commit_id, reaction)) = try_extract_legacy_commit_reaction(data) {
        manager
            .store_reaction(connection_id, &commit_id, from, &reaction)
            .await;
        emitter.emit(
            &format!("irc:{}:commit-reaction", connection_id),
            json!({ "commitId": commit_id }),
        );
        is_reaction = true;
    } else if let Some((msg_id, reaction, remove)) = try_extract_message_reaction(data) {
        if remove {
            manager
                .remove_message_reaction(connection_id, &msg_id, from, &reaction)
                .await;
        } else {
            manager
                .store_message_reaction(connection_id, &msg_id, from, &reaction)
                .await;
        }
        emitter.emit(
            &format!("irc:{}:message-reaction", connection_id),
            json!({ "msgId": msg_id }),
        );
        // Derive commit-reaction index if the target is a shared-commit.
        if let Some(target_data) = manager
            .find_message_data_by_msgid(connection_id, &msg_id)
            .await
            && let Some(commit_id) = try_extract_shared_commit_id(&target_data)
        {
            if remove {
                manager
                    .remove_reaction(connection_id, &commit_id, from, &reaction)
                    .await;
            } else {
                manager
                    .store_reaction(connection_id, &commit_id, from, &reaction)
                    .await;
            }
            emitter.emit(
                &format!("irc:{}:commit-reaction", connection_id),
                json!({ "commitId": commit_id }),
            );
        }
        is_reaction = true;
    }

    // Index hunk shares so we can look up reactions by file path
    if let Some(ref mid) = stored.msgid
        && let Some((file_path, hunk_key)) = try_extract_hunk_share(data)
    {
        manager
            .index_hunk_share(connection_id, &file_path, &hunk_key, mid)
            .await;
    }

    is_reaction
}

/// Process an IRC event: store data in the MessageStore and emit granular
/// events for the frontend RTKQ cache.
///
/// History messages inside chathistory batches are handled by
/// `try_buffer_history_message` and should NOT reach this function.
pub async fn process_event_for_store(
    manager: &IrcManager,
    emitter: &impl EventEmitter,
    connection_id: &str,
    event: &IrcEvent,
) {
    match event {
        IrcEvent::ChannelMessage {
            from,
            channel,
            text,
            data,
            msgid,
            reply_to,
            ..
        } => {
            // With echo-message, our own sent messages come back as ChannelMessage events.
            // Detect this and set the direction to Outgoing.
            let is_echo = if let Ok(our_nick) = manager.nick(connection_id).await {
                from == &our_nick
            } else {
                false
            };

            let stored = if is_echo {
                Some(
                    manager
                        .store_outgoing_echo(
                            connection_id,
                            channel,
                            from,
                            text,
                            data.as_deref(),
                            msgid.as_deref(),
                            reply_to.as_deref(),
                        )
                        .await,
                )
            } else {
                manager
                    .store_incoming(
                        connection_id,
                        channel,
                        from,
                        text,
                        data.as_deref(),
                        msgid.as_deref(),
                        reply_to.as_deref(),
                        None,
                    )
                    .await
            };

            if let Some(stored) = stored {
                // A message from this user implicitly ends their typing state.
                emitter.emit(
                    &format!("irc:{}:typing", connection_id),
                    json!({ "from": from, "target": channel, "state": "done" }),
                );

                let is_reaction = if let Some(d) = data.as_deref() {
                    let reacted =
                        process_message_data(manager, emitter, connection_id, from, &stored, d)
                            .await;
                    // Working-files sync/delta only applies to channels, not DMs.
                    if !reacted && !is_echo {
                        if let Some(files) = try_extract_working_files_sync(d) {
                            manager
                                .apply_working_files_sync(connection_id, channel, from, files)
                                .await;
                            emitter.emit(
                                &format!("irc:{}:working-files", connection_id),
                                json!({ "channel": channel }),
                            );
                        } else if let Some((added, removed)) = try_extract_working_files_delta(d) {
                            manager
                                .apply_working_files_delta(
                                    connection_id,
                                    channel,
                                    from,
                                    added,
                                    removed,
                                )
                                .await;
                            emitter.emit(
                                &format!("irc:{}:working-files", connection_id),
                                json!({ "channel": channel }),
                            );
                        }
                    }
                    reacted
                } else {
                    false
                };
                if !is_reaction {
                    emitter.emit(
                        &format!("irc:{}:message", connection_id),
                        serde_json::to_value(&stored).unwrap_or_default(),
                    );
                    // Notify on mention in channel messages (not our own echo).
                    if !is_echo {
                        match manager.nick(connection_id).await {
                            Ok(our_nick) => {
                                let mentioned = is_mention(text, &our_nick);
                                debug!(
                                    %from, %channel, %our_nick, %mentioned,
                                    text_preview = &text[..floor_char_boundary(text, text.len().min(80))],
                                    "Mention check for channel message",
                                );
                                if mentioned {
                                    notify_mention(from, channel, text);
                                    emitter.emit(
                                        &format!("irc:{}:mention", connection_id),
                                        json!({ "from": from, "target": channel, "text": text }),
                                    );
                                }
                            }
                            Err(e) => {
                                debug!(error = %e, "Failed to get nick for mention check");
                            }
                        }
                    } else {
                        debug!(%from, %channel, "Skipping mention check (echo)");
                    }
                }
            }
        }
        IrcEvent::PrivateMessage {
            from,
            target: pm_target,
            text,
            data,
            msgid,
            reply_to,
            ..
        } => {
            // With echo-message, our own sent DMs come back as PrivateMessage events.
            let is_echo = if let Ok(our_nick) = manager.nick(connection_id).await {
                from == &our_nick
            } else {
                false
            };

            // For DMs: if it's an echo, `from` is us and `pm_target` is the recipient.
            // For incoming DMs, `from` is the sender and `pm_target` is us.
            // We store under the *other* party's nick (the conversation partner).
            let conversation_target = if is_echo { pm_target } else { from };
            manager
                .store_add_channel(connection_id, conversation_target)
                .await;

            let stored = if is_echo {
                Some(
                    manager
                        .store_outgoing_echo(
                            connection_id,
                            conversation_target,
                            from,
                            text,
                            data.as_deref(),
                            msgid.as_deref(),
                            reply_to.as_deref(),
                        )
                        .await,
                )
            } else {
                manager
                    .store_incoming(
                        connection_id,
                        conversation_target,
                        from,
                        text,
                        data.as_deref(),
                        msgid.as_deref(),
                        reply_to.as_deref(),
                        None,
                    )
                    .await
            };

            if let Some(stored) = stored {
                // A message from this user implicitly ends their typing state.
                emitter.emit(
                    &format!("irc:{}:typing", connection_id),
                    json!({ "from": from, "target": conversation_target, "state": "done" }),
                );

                let is_reaction = if let Some(d) = data.as_deref() {
                    process_message_data(manager, emitter, connection_id, from, &stored, d).await
                } else {
                    false
                };
                if !is_reaction {
                    emitter.emit(
                        &format!("irc:{}:message", connection_id),
                        serde_json::to_value(&stored).unwrap_or_default(),
                    );
                    // Notify on all incoming DMs.
                    if !is_echo {
                        notify_mention(from, from, text);
                        emitter.emit(
                            &format!("irc:{}:mention", connection_id),
                            json!({ "from": from, "target": conversation_target, "text": text }),
                        );
                    }
                }
                emitter.emit(
                    &format!("irc:{}:channels", connection_id),
                    json!({ "action": "updated" }),
                );
            }
        }
        IrcEvent::UserJoined { channel, nick, .. } => {
            manager
                .store_user_joined(connection_id, channel, nick)
                .await;
            manager.store_add_channel(connection_id, channel).await;
            emitter.emit(
                &format!("irc:{}:users", connection_id),
                json!({ "channel": channel, "action": "joined", "nick": nick }),
            );
            emitter.emit(
                &format!("irc:{}:channels", connection_id),
                json!({ "action": "updated" }),
            );
        }
        IrcEvent::UserParted { channel, nick, .. } => {
            // If it's us parting, remove the channel and its messages entirely
            if let Ok(our_nick) = manager.nick(connection_id).await
                && nick == &our_nick
            {
                manager.store_remove_channel(connection_id, channel).await;
                emitter.emit(
                    &format!("irc:{}:channels", connection_id),
                    json!({ "action": "updated" }),
                );
                emitter.emit(
                    &format!("irc:{}:users", connection_id),
                    json!({ "channel": channel, "action": "parted", "nick": nick }),
                );
                return;
            }
            manager
                .store_user_parted(connection_id, channel, nick)
                .await;
            manager.remove_working_files_user(connection_id, nick).await;
            emitter.emit(
                &format!("irc:{}:working-files", connection_id),
                json!({ "channel": channel }),
            );
            emitter.emit(
                &format!("irc:{}:users", connection_id),
                json!({ "channel": channel, "action": "parted", "nick": nick }),
            );
        }
        IrcEvent::UserQuit { nick, .. } => {
            let channels = manager.store_user_quit(connection_id, nick).await;
            let _ = channels; // user list updates are emitted below
            manager.remove_working_files_user(connection_id, nick).await;
            emitter.emit(&format!("irc:{}:working-files", connection_id), json!({}));
            emitter.emit(
                &format!("irc:{}:users", connection_id),
                json!({ "action": "quit", "nick": nick }),
            );
        }
        IrcEvent::NamesList { channel, names } => {
            manager
                .store_set_users(connection_id, channel, names.clone())
                .await;
            emitter.emit(
                &format!("irc:{}:users", connection_id),
                json!({ "channel": channel, "action": "names", "users": names }),
            );
        }
        IrcEvent::WhoReply {
            channel,
            nick,
            away,
        } => {
            manager
                .store_set_user_away(connection_id, nick, *away)
                .await;
            emitter.emit(
                &format!("irc:{}:users", connection_id),
                json!({ "channel": channel, "action": "away", "nick": nick, "away": away }),
            );
        }
        IrcEvent::ChannelTopic { channel, topic } => {
            manager.store_set_topic(connection_id, channel, topic).await;
            emitter.emit(
                &format!("irc:{}:channels", connection_id),
                json!({ "action": "topic", "channel": channel, "topic": topic }),
            );
        }
        IrcEvent::NickChanged {
            old_nick,
            new_nick,
            msgid,
            ..
        } => {
            let channels = manager
                .store_nick_changed(connection_id, old_nick, new_nick)
                .await;
            // Store a system message in each channel the user was in
            let text = format!("{old_nick} is now known as {new_nick}");
            for channel in &channels {
                if let Some(stored) = manager
                    .store_incoming(
                        connection_id,
                        channel,
                        "*",
                        &text,
                        None,
                        msgid.as_deref(),
                        None,
                        Some("nick"),
                    )
                    .await
                {
                    emitter.emit(
                        &format!("irc:{}:message", connection_id),
                        serde_json::to_value(&stored).unwrap_or_default(),
                    );
                }
            }
            emitter.emit(
                &format!("irc:{}:users", connection_id),
                json!({ "action": "nick_changed", "oldNick": old_nick, "newNick": new_nick }),
            );
        }
        IrcEvent::Away { nick, message } => {
            let away = message.is_some();
            debug!(
                %connection_id, %nick, %away,
                away_message = message.as_deref().unwrap_or("(back)"),
                "AWAY event received",
            );
            let channels = manager.store_set_user_away(connection_id, nick, away).await;
            debug!(
                %connection_id, %nick, %away,
                affected_channels = ?channels,
                "AWAY status updated in store",
            );
            for channel in &channels {
                emitter.emit(
                    &format!("irc:{}:users", connection_id),
                    json!({ "channel": channel, "action": "away", "nick": nick, "away": away }),
                );
            }
        }
        // Server messages → store under the "*" system channel
        IrcEvent::Welcome { nick, message } => {
            manager.store_add_channel(connection_id, "*").await;
            let text = format!("Welcome {nick}: {message}");
            if let Some(stored) = manager
                .store_incoming(
                    connection_id,
                    "*",
                    "*",
                    &text,
                    None,
                    None,
                    None,
                    Some("001"),
                )
                .await
            {
                emitter.emit(
                    &format!("irc:{}:message", connection_id),
                    serde_json::to_value(&stored).unwrap_or_default(),
                );
            }
            // Emit negotiated IRCv3 capabilities to the server log channel.
            if let Ok(caps) = manager.negotiated_caps(connection_id).await {
                let caps_text = if caps.is_empty() {
                    "CAP: no capabilities negotiated".to_string()
                } else {
                    format!("CAP: negotiated {}", caps.join(", "))
                };
                if let Some(stored) = manager
                    .store_incoming(
                        connection_id,
                        "*",
                        "*",
                        &caps_text,
                        None,
                        None,
                        None,
                        Some("cap"),
                    )
                    .await
                {
                    emitter.emit(
                        &format!("irc:{}:message", connection_id),
                        serde_json::to_value(&stored).unwrap_or_default(),
                    );
                }
            }
            emitter.emit(
                &format!("irc:{}:channels", connection_id),
                serde_json::json!({ "action": "updated" }),
            );
        }
        IrcEvent::Motd { message } => {
            if let Some(stored) = manager
                .store_incoming(
                    connection_id,
                    "*",
                    "*",
                    message,
                    None,
                    None,
                    None,
                    Some("motd"),
                )
                .await
            {
                emitter.emit(
                    &format!("irc:{}:message", connection_id),
                    serde_json::to_value(&stored).unwrap_or_default(),
                );
            }
        }
        IrcEvent::Invited {
            from,
            nick: invited_nick,
            channel,
        } => {
            // Check if we are the one being invited
            let is_us = manager
                .nick(connection_id)
                .await
                .map(|our_nick| our_nick == *invited_nick)
                .unwrap_or(false);
            if is_us {
                info!(%channel, %from, "Received channel invite, auto-joining");

                // Auto-join the channel we were invited to.
                if let Err(e) = manager.join(connection_id, channel).await {
                    tracing::warn!(%channel, error = %e, "Failed to auto-join after invite");
                }

                // Show a system message in the server channel
                let text = format!("{from} invited you to {channel}");
                if let Some(stored) = manager
                    .store_incoming(
                        connection_id,
                        "*",
                        "*",
                        &text,
                        None,
                        None,
                        None,
                        Some("invite"),
                    )
                    .await
                {
                    emitter.emit(
                        &format!("irc:{}:message", connection_id),
                        serde_json::to_value(&stored).unwrap_or_default(),
                    );
                }

                // Emit a dedicated invite event so the frontend can prompt the user.
                emitter.emit(
                    &format!("irc:{}:invited", connection_id),
                    json!({ "from": from, "channel": channel }),
                );
            }
        }
        IrcEvent::InviteSent { nick, channel } => {
            // Confirmation that our INVITE was accepted by the server
            let text = format!("Invited {nick} to {channel}");
            if let Some(stored) = manager
                .store_incoming(
                    connection_id,
                    "*",
                    "*",
                    &text,
                    None,
                    None,
                    None,
                    Some("invite"),
                )
                .await
            {
                emitter.emit(
                    &format!("irc:{}:message", connection_id),
                    serde_json::to_value(&stored).unwrap_or_default(),
                );
            }
        }
        IrcEvent::Notice { from, message, .. } => {
            let sender = from.as_deref().unwrap_or("*");
            if let Some(stored) = manager
                .store_incoming(
                    connection_id,
                    "*",
                    sender,
                    message,
                    None,
                    None,
                    None,
                    Some("notice"),
                )
                .await
            {
                emitter.emit(
                    &format!("irc:{}:message", connection_id),
                    serde_json::to_value(&stored).unwrap_or_default(),
                );
            }
        }
        IrcEvent::Error { code, message } => {
            let text = format!("[{code}] {message}");
            if let Some(stored) = manager
                .store_incoming(connection_id, "*", "*", &text, None, None, None, Some(code))
                .await
            {
                emitter.emit(
                    &format!("irc:{}:message", connection_id),
                    serde_json::to_value(&stored).unwrap_or_default(),
                );
            }
        }
        IrcEvent::Redact {
            from,
            target,
            msgid,
            reason,
        } => {
            let removed = manager
                .remove_message_by_msgid(connection_id, target, msgid)
                .await;
            debug!(
                %from, %target, %msgid, ?reason, %removed,
                "Message redacted",
            );
            emitter.emit(
                &format!("irc:{}:message-redacted", connection_id),
                json!({ "target": target, "msgid": msgid, "from": from }),
            );
        }
        IrcEvent::Typing {
            from,
            target,
            state,
            ..
        } => {
            // Typing indicators are ephemeral — emit a WebSocket event but don't store.
            emitter.emit(
                &format!("irc:{}:typing", connection_id),
                json!({ "from": from, "target": target, "state": state }),
            );
        }
        IrcEvent::TagReaction {
            from,
            target: _,
            msgid,
            emoji,
            remove,
            ..
        } => {
            if !msgid.is_empty() {
                if *remove {
                    manager
                        .remove_message_reaction(connection_id, msgid, from, emoji)
                        .await;
                } else {
                    manager
                        .store_message_reaction(connection_id, msgid, from, emoji)
                        .await;
                }
                emitter.emit(
                    &format!("irc:{}:message-reaction", connection_id),
                    json!({ "msgId": msgid }),
                );
                // Derive commit-reaction index if the reacted-to message is a shared-commit.
                if let Some(target_data) = manager
                    .find_message_data_by_msgid(connection_id, msgid)
                    .await
                    && let Some(commit_id) = try_extract_shared_commit_id(&target_data)
                {
                    if *remove {
                        manager
                            .remove_reaction(connection_id, &commit_id, from, emoji)
                            .await;
                    } else {
                        manager
                            .store_reaction(connection_id, &commit_id, from, emoji)
                            .await;
                    }
                    emitter.emit(
                        &format!("irc:{}:commit-reaction", connection_id),
                        json!({ "commitId": commit_id }),
                    );
                }
            }
        }
        IrcEvent::Raw { command, params } => {
            // Numeric server responses (002, 003, 005, 251-255, etc.) go to the
            // server log channel, just like a regular IRC client would show them.
            if command.parse::<u16>().is_ok() {
                // First param is usually our nick — skip it for display.
                let text = if params.len() > 1 {
                    params[1..].join(" ")
                } else {
                    params.join(" ")
                };
                if !text.is_empty()
                    && let Some(stored) = manager
                        .store_incoming(
                            connection_id,
                            "*",
                            "*",
                            &text,
                            None,
                            None,
                            None,
                            Some(command),
                        )
                        .await
                {
                    emitter.emit(
                        &format!("irc:{}:message", connection_id),
                        serde_json::to_value(&stored).unwrap_or_default(),
                    );
                }
            } else {
                debug!(
                    "Unhandled IRC event on '{}': command={}, params={:?}",
                    connection_id, command, params
                );
            }
        }
        // Ping, Batch, etc. don't need store handling
        _ => {}
    }
}

// ============================================================================
// Mention detection & desktop notifications
// ============================================================================

/// Check whether `text` contains a word-boundary mention of `nick`.
fn is_mention(text: &str, nick: &str) -> bool {
    let text_lower = text.to_lowercase();
    let nick_lower = nick.to_lowercase();
    for (idx, _) in text_lower.match_indices(&nick_lower) {
        let before = if idx == 0 {
            true
        } else {
            !text_lower.as_bytes()[idx - 1].is_ascii_alphanumeric()
        };
        let after_idx = idx + nick_lower.len();
        let after = if after_idx >= text_lower.len() {
            true
        } else {
            !text_lower.as_bytes()[after_idx].is_ascii_alphanumeric()
        };
        if before && after {
            return true;
        }
    }
    false
}

/// Fire a desktop notification for a mention or DM.
fn notify_mention(from: &str, source: &str, text: &str) {
    use notify_rust::Notification;

    tracing::debug!(%from, %source, "Attempting desktop notification");

    let title = if source.starts_with('#') {
        format!("{from} in {source}")
    } else {
        from.to_string()
    };

    // Truncate at a char boundary to avoid panicking on multi-byte UTF-8.
    let body = if text.len() > 140 {
        let end = floor_char_boundary(text, 140);
        format!("{}…", &text[..end])
    } else {
        text.to_string()
    };

    match Notification::new().summary(&title).body(&body).show() {
        Ok(_) => tracing::debug!(%title, "Desktop notification shown"),
        Err(e) => tracing::warn!(%title, error = %e, "Failed to show desktop notification"),
    }
}

/// MSRV-compatible replacement for `str::floor_char_boundary` (stable since 1.91).
///
/// Returns the largest byte index `<= index` that is a valid UTF-8 char boundary.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        s.len()
    } else {
        let mut i = index;
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn server(host: &str, port: u16) -> IrcServerSettings {
        IrcServerSettings {
            host: host.to_string(),
            port,
        }
    }

    fn conn(enabled: bool) -> IrcConnectionSettings {
        IrcConnectionSettings {
            enabled,
            nickname: None,
            server_password: None,
            sasl_password: None,
            realname: None,
        }
    }

    fn conn_with_nick(enabled: bool, nick: &str) -> IrcConnectionSettings {
        IrcConnectionSettings {
            nickname: Some(nick.to_string()),
            ..conn(enabled)
        }
    }

    /// Connection settings with both passwords set — required for `build_irc_config` to return `Some`.
    fn conn_with_passwords(enabled: bool) -> IrcConnectionSettings {
        IrcConnectionSettings {
            server_password: Some("gate".to_string()),
            sasl_password: Some("secret".to_string()),
            ..conn(enabled)
        }
    }

    fn conn_with_nick_and_passwords(enabled: bool, nick: &str) -> IrcConnectionSettings {
        IrcConnectionSettings {
            nickname: Some(nick.to_string()),
            ..conn_with_passwords(enabled)
        }
    }

    // =========================================================================
    // build_irc_config tests
    // =========================================================================

    #[test]
    fn build_config_with_valid_server_and_nick() {
        let s = server("irc.example.com", 6697);
        let c = conn_with_nick_and_passwords(true, "testuser");
        let config = build_irc_config(&s, &c, "fallback").unwrap();

        assert_eq!(config.server, "irc.example.com");
        assert_eq!(config.port, 6697);
        assert!(config.use_tls);
        assert_eq!(config.nick, "testuser");
        assert_eq!(config.username, Some("testuser".to_string()));
        assert_eq!(config.realname, Some("testuser".to_string()));
    }

    #[test]
    fn build_config_empty_host_returns_none() {
        let s = server("", 6697);
        let c = conn_with_nick(true, "testuser");
        assert!(build_irc_config(&s, &c, "fallback").is_none());
    }

    #[test]
    fn build_config_empty_nick_uses_default() {
        let s = server("irc.example.com", 6697);
        let c = conn_with_passwords(true); // nickname is None
        let config = build_irc_config(&s, &c, "my-default").unwrap();

        assert_eq!(config.nick, "my-default");
    }

    #[test]
    fn build_config_whitespace_nick_uses_default() {
        let s = server("irc.example.com", 6697);
        let c = IrcConnectionSettings {
            nickname: Some("   ".to_string()),
            ..conn_with_passwords(true)
        };
        let config = build_irc_config(&s, &c, "my-default").unwrap();

        assert_eq!(config.nick, "my-default");
    }

    #[test]
    fn build_config_missing_server_password_returns_none() {
        let s = server("irc.example.com", 6697);
        let c = IrcConnectionSettings {
            sasl_password: Some("secret".to_string()),
            ..conn(true)
        };
        assert!(build_irc_config(&s, &c, "fallback").is_none());
    }

    #[test]
    fn build_config_missing_sasl_password_returns_none() {
        let s = server("irc.example.com", 6697);
        let c = IrcConnectionSettings {
            server_password: Some("gate".to_string()),
            ..conn(true)
        };
        assert!(build_irc_config(&s, &c, "fallback").is_none());
    }

    #[test]
    fn build_config_empty_sasl_password_returns_none() {
        let s = server("irc.example.com", 6697);
        let c = IrcConnectionSettings {
            server_password: Some("gate".to_string()),
            sasl_password: Some("".to_string()),
            ..conn(true)
        };
        assert!(build_irc_config(&s, &c, "fallback").is_none());
    }

    #[test]
    fn build_config_both_passwords_preserved() {
        let s = server("irc.example.com", 6697);
        let c = IrcConnectionSettings {
            server_password: Some("gate".to_string()),
            sasl_password: Some("secret".to_string()),
            ..conn(true)
        };
        let config = build_irc_config(&s, &c, "fallback").unwrap();

        assert_eq!(config.server_password, Some("gate".to_string()));
        assert_eq!(config.sasl_password, Some("secret".to_string()));
    }

    #[test]
    fn build_config_realname_falls_back_to_nick() {
        let s = server("irc.example.com", 6697);
        let c = conn_with_nick_and_passwords(true, "myuser");
        let config = build_irc_config(&s, &c, "fallback").unwrap();

        assert_eq!(config.realname, Some("myuser".to_string()));
    }

    #[test]
    fn build_config_explicit_realname_used() {
        let s = server("irc.example.com", 6697);
        let c = IrcConnectionSettings {
            realname: Some("My Real Name".to_string()),
            ..conn_with_nick_and_passwords(true, "myuser")
        };
        let config = build_irc_config(&s, &c, "fallback").unwrap();

        assert_eq!(config.realname, Some("My Real Name".to_string()));
    }

    #[test]
    fn build_config_port_from_server_settings() {
        let s = server("irc.example.com", 7000);
        let c = conn_with_nick_and_passwords(true, "testuser");
        let config = build_irc_config(&s, &c, "fallback").unwrap();

        assert_eq!(config.port, 7000);
    }

    // =========================================================================
    // compute_reconciliation tests
    // =========================================================================

    #[test]
    fn reconcile_disabled_to_enabled_triggers_connect() {
        let s = server("irc.example.com", 6697);
        assert_eq!(
            compute_reconciliation(&s, &conn(false), &s, &conn(true)),
            ConnectionAction::Connect
        );
    }

    #[test]
    fn reconcile_enabled_to_disabled_triggers_disconnect() {
        let s = server("irc.example.com", 6697);
        assert_eq!(
            compute_reconciliation(&s, &conn(true), &s, &conn(false)),
            ConnectionAction::Disconnect
        );
    }

    #[test]
    fn reconcile_same_config_is_noop() {
        let s = server("irc.example.com", 6697);
        let c = conn_with_nick(true, "testuser");
        assert_eq!(
            compute_reconciliation(&s, &c, &s, &c),
            ConnectionAction::NoOp
        );
    }

    #[test]
    fn reconcile_nick_change_triggers_reconnect() {
        let s = server("irc.example.com", 6697);
        assert_eq!(
            compute_reconciliation(
                &s,
                &conn_with_nick(true, "old"),
                &s,
                &conn_with_nick(true, "new"),
            ),
            ConnectionAction::Reconnect
        );
    }

    #[test]
    fn reconcile_server_change_triggers_reconnect() {
        let old_s = server("old.example.com", 6697);
        let new_s = server("new.example.com", 6697);
        let c = conn_with_nick(true, "user");
        assert_eq!(
            compute_reconciliation(&old_s, &c, &new_s, &c),
            ConnectionAction::Reconnect
        );
    }

    #[test]
    fn reconcile_port_change_triggers_reconnect() {
        let old_s = server("irc.example.com", 6697);
        let new_s = server("irc.example.com", 7000);
        let c = conn_with_nick(true, "user");
        assert_eq!(
            compute_reconciliation(&old_s, &c, &new_s, &c),
            ConnectionAction::Reconnect
        );
    }

    #[test]
    fn reconcile_sasl_password_change_triggers_reconnect() {
        let s = server("irc.example.com", 6697);
        let old_c = conn_with_nick(true, "user");
        let new_c = IrcConnectionSettings {
            sasl_password: Some("secret".to_string()),
            ..conn_with_nick(true, "user")
        };
        assert_eq!(
            compute_reconciliation(&s, &old_c, &s, &new_c),
            ConnectionAction::Reconnect
        );
    }

    #[test]
    fn reconcile_both_disabled_is_noop() {
        let s = server("irc.example.com", 6697);
        assert_eq!(
            compute_reconciliation(&s, &conn(false), &s, &conn(false)),
            ConnectionAction::NoOp
        );
    }
}
