//! IRC command handlers for the but-server HTTP API.
//!
//! Many handlers delegate directly to [`but_irc::commands`], while others
//! (e.g. broadcast handlers, `send_message`) contain additional logic such as
//! emitting frontend events or seeding initial state.

use axum::{Json, extract::State};
use but_claude::broadcaster::FrontendEvent;
use but_irc::commands::{self, *};
use serde::Deserialize;
use serde_json::json;

use crate::{AppState, Response};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartWorkingFilesBroadcastParams {
    project_id: String,
    #[serde(default = "default_connection_id")]
    connection_id: String,
    channel: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopWorkingFilesBroadcastParams {
    project_id: String,
}

fn default_connection_id() -> String {
    "personal-irc".to_string()
}

// ============================================================================
// Helper: convert but_irc::Result → JSON Response
// ============================================================================

fn ok_json(value: serde_json::Value) -> Json<serde_json::Value> {
    Json(json!(Response::Success(value)))
}

fn err_json(e: impl Into<anyhow::Error>) -> Json<serde_json::Value> {
    let err = but_api::json::Error::from(e.into());
    Json(json!(Response::Error(json!(err))))
}

// ============================================================================
// Handlers
// ============================================================================

/// Create and connect an IRC connection.
pub async fn irc_connect(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // Accept both `{ params: { ... } }` and flat `{ id, server, ... }`.
    let params: ConnectParams =
        match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
            Ok(p) => p,
            Err(e) => return err_json(e),
        };
    let emitter = crate::irc_lifecycle::BroadcasterEmitter::new(&state.app.broadcaster);
    match commands::connect(&state.irc_manager, &emitter, params).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Disconnect an IRC connection.
pub async fn irc_disconnect(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    match commands::disconnect(&state.irc_manager, &p.id).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Get the state of an IRC connection.
pub async fn irc_state(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    let response = commands::state(&state.irc_manager, &p.id).await;
    ok_json(json!(response))
}

/// Wait for an IRC connection to be ready.
pub async fn irc_wait_ready(
    State(state): State<AppState>,
    Json(p): Json<WaitReadyParams>,
) -> Json<serde_json::Value> {
    match commands::wait_ready(&state.irc_manager, &p.id, p.timeout_secs).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Join a channel on an IRC connection.
pub async fn irc_join(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    match commands::join(&state.irc_manager, &p.id, &p.channel).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Part (leave) a channel on an IRC connection.
pub async fn irc_part(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    match commands::part(&state.irc_manager, &p.id, &p.channel).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Add a channel to the auto-join set (joins immediately if connected, re-joins on reconnect).
pub async fn irc_auto_join(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    match commands::auto_join(&state.irc_manager, &p.id, &p.channel).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Remove a channel from the auto-join set and part it.
pub async fn irc_auto_leave(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    match commands::auto_leave(&state.irc_manager, &p.id, &p.channel).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Send a message on an IRC connection.
pub async fn irc_send_message(
    State(state): State<AppState>,
    Json(p): Json<SendMessageParams>,
) -> Json<serde_json::Value> {
    match commands::send_message(
        &state.irc_manager,
        &p.id,
        &p.target,
        &p.message,
        p.reply_to.as_deref(),
    )
    .await
    {
        Ok(Some((stored, is_dm))) => {
            if is_dm {
                let channels_event = FrontendEvent {
                    name: format!("irc:{}:channels", p.id),
                    payload: json!({ "action": "updated" }),
                };
                state.app.broadcaster.lock().await.send(channels_event);
            }
            let msg_event = FrontendEvent {
                name: format!("irc:{}:message", p.id),
                payload: serde_json::to_value(&stored).unwrap_or_default(),
            };
            state.app.broadcaster.lock().await.send(msg_event);
            ok_json(json!({}))
        }
        Ok(None) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Send a message with data payload on an IRC connection.
pub async fn irc_send_message_with_data(
    State(state): State<AppState>,
    Json(p): Json<SendMessageWithDataParams>,
) -> Json<serde_json::Value> {
    match commands::send_message_with_data(
        &state.irc_manager,
        &p.id,
        &p.target,
        &p.message,
        &p.data,
        p.reply_to.as_deref(),
    )
    .await
    {
        Ok(Some(stored)) => {
            let msg_event = FrontendEvent {
                name: format!("irc:{}:message", p.id),
                payload: serde_json::to_value(&stored).unwrap_or_default(),
            };
            state.app.broadcaster.lock().await.send(msg_event);
            // Notify that reactions may have updated
            let commit_evt = FrontendEvent {
                name: format!("irc:{}:commit-reaction", p.id),
                payload: json!({}),
            };
            state.app.broadcaster.lock().await.send(commit_evt);
            let reaction_evt = FrontendEvent {
                name: format!("irc:{}:message-reaction", p.id),
                payload: json!({}),
            };
            state.app.broadcaster.lock().await.send(reaction_evt);
            ok_json(json!({}))
        }
        Ok(None) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Get all commit reactions for an IRC connection, keyed by commit ID.
pub async fn irc_get_all_commit_reactions(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    let reactions = commands::get_all_commit_reactions(&state.irc_manager, &p.id).await;
    ok_json(json!(reactions))
}

/// Get all message reactions for an IRC connection, keyed by message ID.
pub async fn irc_get_all_message_reactions(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    let reactions = commands::get_all_message_reactions(&state.irc_manager, &p.id).await;
    ok_json(json!(reactions))
}

/// Get the current working files for all users in a channel.
pub async fn irc_get_working_files(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    let files = commands::get_working_files(&state.irc_manager, &p.id, &p.channel).await;
    ok_json(json!(files))
}

/// Start broadcasting working files for a project to an IRC channel.
pub async fn irc_start_working_files_broadcast(
    State(state): State<AppState>,
    Json(p): Json<StartWorkingFilesBroadcastParams>,
) -> Json<serde_json::Value> {
    let project_id = match p
        .project_id
        .parse::<but_ctx::ProjectHandleOrLegacyProjectId>()
    {
        Ok(id) => id,
        Err(e) => return err_json(e),
    };

    // Seed initial file list from the current worktree state.
    let initial_files = match but_ctx::Context::try_from(project_id.clone()) {
        Ok(mut ctx) => match but_api::diff::changes_in_worktree(&mut ctx) {
            Ok(changes) => changes
                .worktree_changes
                .changes
                .iter()
                .map(|c| c.path.to_string())
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to get worktree changes for initial working files");
                vec![]
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create context for initial working files");
            vec![]
        }
    };

    state
        .working_files_broadcast
        .start(project_id, p.connection_id, p.channel, initial_files)
        .await;
    ok_json(json!({}))
}

/// Stop broadcasting working files for a project.
pub async fn irc_stop_working_files_broadcast(
    State(state): State<AppState>,
    Json(p): Json<StopWorkingFilesBroadcastParams>,
) -> Json<serde_json::Value> {
    let project_id = match p
        .project_id
        .parse::<but_ctx::ProjectHandleOrLegacyProjectId>()
    {
        Ok(id) => id,
        Err(e) => return err_json(e),
    };

    state.working_files_broadcast.stop(project_id).await;
    ok_json(json!({}))
}

/// List all IRC connections.
pub async fn irc_list_connections(State(state): State<AppState>) -> Json<serde_json::Value> {
    let connections = commands::list_connections(&state.irc_manager).await;
    ok_json(json!(connections))
}

/// Check if an IRC connection exists.
pub async fn irc_exists(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    let exists = commands::exists(&state.irc_manager, &p.id).await;
    ok_json(json!(exists))
}

/// Get the nickname for an IRC connection.
pub async fn irc_nick(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    match commands::nick(&state.irc_manager, &p.id).await {
        Ok(nick) => ok_json(json!(nick)),
        Err(e) => err_json(e),
    }
}

/// Get stored messages for a channel on an IRC connection.
pub async fn irc_messages(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    let messages = commands::messages(&state.irc_manager, &p.id, &p.channel).await;
    ok_json(json!(messages))
}

/// Get the list of channels for an IRC connection.
pub async fn irc_channels(
    State(state): State<AppState>,
    Json(p): Json<IdParams>,
) -> Json<serde_json::Value> {
    let channels = commands::channels(&state.irc_manager, &p.id).await;
    ok_json(json!(channels))
}

/// Get the user list for a channel on an IRC connection.
pub async fn irc_users(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    let users = commands::users(&state.irc_manager, &p.id, &p.channel).await;
    ok_json(json!(users))
}

/// Clear stored messages for a channel on an IRC connection.
pub async fn irc_clear_messages(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    commands::clear_messages(&state.irc_manager, &p.id, &p.channel).await;
    ok_json(json!({}))
}

/// Mark a channel as read on an IRC connection.
pub async fn irc_mark_read(
    State(state): State<AppState>,
    Json(p): Json<ChannelParams>,
) -> Json<serde_json::Value> {
    commands::mark_read(&state.irc_manager, &p.id, &p.channel).await;
    ok_json(json!({}))
}

/// Request chat history for a channel on an IRC connection.
pub async fn irc_request_history(
    State(state): State<AppState>,
    Json(p): Json<HistoryParams>,
) -> Json<serde_json::Value> {
    match commands::request_history(&state.irc_manager, &p.id, &p.channel, p.limit).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Request older chat history before a given timestamp.
pub async fn irc_request_history_before(
    State(state): State<AppState>,
    Json(p): Json<HistoryBeforeParams>,
) -> Json<serde_json::Value> {
    match commands::request_history_before(
        &state.irc_manager,
        &p.id,
        &p.channel,
        &p.before,
        p.limit,
    )
    .await
    {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Get message reactions for a specific file, keyed by hunk key.
pub async fn irc_get_file_message_reactions(
    State(state): State<AppState>,
    Json(p): Json<FileMessageReactionsParams>,
) -> Json<serde_json::Value> {
    let reactions =
        commands::get_file_message_reactions(&state.irc_manager, &p.id, &p.file_path).await;
    ok_json(json!(reactions))
}

/// Send a raw IRC command on a connection.
pub async fn irc_send_raw(
    State(state): State<AppState>,
    Json(p): Json<SendRawParams>,
) -> Json<serde_json::Value> {
    match commands::send_raw(&state.irc_manager, &p.id, &p.command).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Send a typing indicator via TAGMSG.
pub async fn irc_send_typing(
    State(state): State<AppState>,
    Json(p): Json<SendTypingParams>,
) -> Json<serde_json::Value> {
    match commands::send_typing(&state.irc_manager, &p.id, &p.target, &p.state).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Send a reaction to a message via TAGMSG.
pub async fn irc_send_reaction(
    State(state): State<AppState>,
    Json(p): Json<ReactionParams>,
) -> Json<serde_json::Value> {
    match commands::send_reaction(&state.irc_manager, &p.id, &p.target, &p.msgid, &p.emoji).await {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Remove a reaction from a message via TAGMSG.
pub async fn irc_remove_reaction(
    State(state): State<AppState>,
    Json(p): Json<ReactionParams>,
) -> Json<serde_json::Value> {
    match commands::remove_reaction(&state.irc_manager, &p.id, &p.target, &p.msgid, &p.emoji).await
    {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}

/// Redact (delete) a message via draft/message-redaction.
pub async fn irc_redact_message(
    State(state): State<AppState>,
    Json(p): Json<RedactParams>,
) -> Json<serde_json::Value> {
    match commands::redact_message(
        &state.irc_manager,
        &p.id,
        &p.target,
        &p.msgid,
        p.reason.as_deref(),
    )
    .await
    {
        Ok(()) => ok_json(json!({})),
        Err(e) => err_json(e),
    }
}
