//! IRC Tauri commands — thin wrappers around [`but_irc::commands`].

use but_api::json::{Error, ToJsonError};
use but_irc::WorkingFilesBroadcast;
use but_irc::commands::{self, ConnectParams, ConnectionStateResponse};
use but_irc::message_store::Reaction;
use but_irc::{ChannelInfo, ConnectionId, IrcManager, StoredMessage};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(manager, app_handle), err(Debug))]
pub async fn irc_connect(
    manager: State<'_, IrcManager>,
    app_handle: AppHandle,
    params: ConnectParams,
) -> Result<(), Error> {
    let emitter = crate::irc_lifecycle::TauriEmitter::new(&app_handle);
    commands::connect(&manager, &emitter, params)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_disconnect(manager: State<'_, IrcManager>, id: String) -> Result<(), Error> {
    commands::disconnect(&manager, &id).await.to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager))]
pub async fn irc_state(
    manager: State<'_, IrcManager>,
    id: String,
) -> Result<ConnectionStateResponse, Error> {
    Ok(commands::state(&manager, &id).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_wait_ready(
    manager: State<'_, IrcManager>,
    id: String,
    timeout_secs: Option<u64>,
) -> Result<(), Error> {
    commands::wait_ready(&manager, &id, timeout_secs)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_join(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<(), Error> {
    commands::join(&manager, &id, &channel)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_part(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<(), Error> {
    commands::part(&manager, &id, &channel)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_auto_join(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<(), Error> {
    commands::auto_join(&manager, &id, &channel)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_auto_leave(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<(), Error> {
    commands::auto_leave(&manager, &id, &channel)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager, app_handle), err(Debug))]
pub async fn irc_send_message(
    manager: State<'_, IrcManager>,
    app_handle: AppHandle,
    id: String,
    target: String,
    message: String,
    reply_to: Option<String>,
) -> Result<(), Error> {
    let result = commands::send_message(&manager, &id, &target, &message, reply_to.as_deref())
        .await
        .to_json_error()?;

    // Emit the stored message to the frontend for immediate display.
    // With echo-message, the server echo also triggers process_event_for_store
    // which deduplicates, so this is safe.
    if let Some((stored, is_dm)) = result {
        if is_dm {
            let _ = app_handle.emit(
                &format!("irc:{id}:channels"),
                &serde_json::json!({ "action": "updated" }),
            );
        }
        let _ = app_handle.emit(&format!("irc:{id}:message"), &stored);
    }
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(manager, app_handle), err(Debug))]
pub async fn irc_send_message_with_data(
    manager: State<'_, IrcManager>,
    app_handle: AppHandle,
    id: String,
    target: String,
    message: String,
    data: String,
    reply_to: Option<String>,
) -> Result<(), Error> {
    let result = commands::send_message_with_data(
        &manager,
        &id,
        &target,
        &message,
        &data,
        reply_to.as_deref(),
    )
    .await
    .to_json_error()?;

    // Emit locally for immediate display; echo-message deduplicates.
    if let Some(stored) = result {
        let _ = app_handle.emit(&format!("irc:{id}:message"), &stored);
        // Also notify that reactions may have changed
        let _ = app_handle.emit(&format!("irc:{id}:commit-reaction"), serde_json::json!({}));
        let _ = app_handle.emit(&format!("irc:{id}:message-reaction"), serde_json::json!({}));
    }
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_get_all_commit_reactions(
    manager: State<'_, IrcManager>,
    id: String,
) -> Result<HashMap<String, Vec<Reaction>>, Error> {
    Ok(commands::get_all_commit_reactions(&manager, &id).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_get_all_message_reactions(
    manager: State<'_, IrcManager>,
    id: String,
) -> Result<HashMap<String, Vec<Reaction>>, Error> {
    Ok(commands::get_all_message_reactions(&manager, &id).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_get_working_files(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<HashMap<String, Vec<String>>, Error> {
    Ok(commands::get_working_files(&manager, &id, &channel).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_list_connections(
    manager: State<'_, IrcManager>,
) -> Result<Vec<ConnectionId>, Error> {
    Ok(commands::list_connections(&manager).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_exists(manager: State<'_, IrcManager>, id: String) -> Result<bool, Error> {
    Ok(commands::exists(&manager, &id).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_nick(manager: State<'_, IrcManager>, id: String) -> Result<String, Error> {
    commands::nick(&manager, &id).await.to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_messages(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<Vec<StoredMessage>, Error> {
    Ok(commands::messages(&manager, &id, &channel).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_channels(
    manager: State<'_, IrcManager>,
    id: String,
) -> Result<Vec<ChannelInfo>, Error> {
    Ok(commands::channels(&manager, &id).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_users(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<Vec<but_irc::message_store::UserEntry>, Error> {
    Ok(commands::users(&manager, &id, &channel).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_clear_messages(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<(), Error> {
    commands::clear_messages(&manager, &id, &channel).await;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_mark_read(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
) -> Result<(), Error> {
    commands::mark_read(&manager, &id, &channel).await;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_request_history(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
    limit: Option<u32>,
) -> Result<(), Error> {
    commands::request_history(&manager, &id, &channel, limit)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_send_raw(
    manager: State<'_, IrcManager>,
    id: String,
    command: String,
) -> Result<(), Error> {
    commands::send_raw(&manager, &id, &command)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_send_typing(
    manager: State<'_, IrcManager>,
    id: String,
    target: String,
    state: String,
) -> Result<(), Error> {
    commands::send_typing(&manager, &id, &target, &state)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_send_reaction(
    manager: State<'_, IrcManager>,
    id: String,
    target: String,
    msgid: String,
    emoji: String,
) -> Result<(), Error> {
    commands::send_reaction(&manager, &id, &target, &msgid, &emoji)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_remove_reaction(
    manager: State<'_, IrcManager>,
    id: String,
    target: String,
    msgid: String,
    emoji: String,
) -> Result<(), Error> {
    commands::remove_reaction(&manager, &id, &target, &msgid, &emoji)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_redact_message(
    manager: State<'_, IrcManager>,
    id: String,
    target: String,
    msgid: String,
    reason: Option<String>,
) -> Result<(), Error> {
    commands::redact_message(&manager, &id, &target, &msgid, reason.as_deref())
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_get_file_message_reactions(
    manager: State<'_, IrcManager>,
    id: String,
    file_path: String,
) -> Result<HashMap<String, Vec<Reaction>>, Error> {
    Ok(commands::get_file_message_reactions(&manager, &id, &file_path).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_request_history_before(
    manager: State<'_, IrcManager>,
    id: String,
    channel: String,
    before: String,
    limit: Option<u32>,
) -> Result<(), Error> {
    commands::request_history_before(&manager, &id, &channel, &before, limit)
        .await
        .to_json_error()
}

#[tauri::command(async)]
#[instrument(skip(broadcast), err(Debug))]
pub async fn irc_start_working_files_broadcast(
    broadcast: State<'_, WorkingFilesBroadcast>,
    project_id: String,
    connection_id: String,
    channel: String,
) -> Result<(), Error> {
    let project_id: but_ctx::ProjectHandleOrLegacyProjectId =
        project_id.parse().map_err(Error::from)?;

    // Seed initial file list from the current worktree state.
    let initial_files = match but_ctx::Context::try_from(project_id.clone()) {
        Ok(mut ctx) => match but_api::legacy::diff::changes_in_worktree(&mut ctx) {
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

    broadcast
        .start(project_id, connection_id, channel, initial_files)
        .await;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(broadcast), err(Debug))]
pub async fn irc_stop_working_files_broadcast(
    broadcast: State<'_, WorkingFilesBroadcast>,
    project_id: String,
) -> Result<(), Error> {
    let project_id: but_ctx::ProjectHandleOrLegacyProjectId =
        project_id.parse().map_err(Error::from)?;

    broadcast.stop(project_id).await;
    Ok(())
}
