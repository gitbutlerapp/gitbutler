//! IRC Tauri commands — thin wrappers around [`but_irc::commands`].

use but_api::json::{Error, ToJsonError};
use but_irc::commands::{self, ConnectParams, ConnectionStateResponse};
use but_irc::message_store::CommitReaction;
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

    // With echo-message, the server echo handles storage and event emission
    // via process_event_for_store, so we skip local event emission.
    if let Some((stored, is_dm)) = result {
        if is_dm {
            let _ = app_handle.emit(
                &format!("irc:{}:channels", id),
                &serde_json::json!({ "action": "updated" }),
            );
        }
        let _ = app_handle.emit(&format!("irc:{}:message", id), &stored);
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

    // With echo-message, the server echo handles storage and event emission.
    if let Some(stored) = result {
        let _ = app_handle.emit(&format!("irc:{}:message", id), &stored);
        // Also notify that reactions may have changed
        let _ = app_handle.emit(
            &format!("irc:{}:commit-reaction", id),
            serde_json::json!({}),
        );
        let _ = app_handle.emit(
            &format!("irc:{}:message-reaction", id),
            serde_json::json!({}),
        );
    }
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_get_all_commit_reactions(
    manager: State<'_, IrcManager>,
    id: String,
) -> Result<HashMap<String, Vec<CommitReaction>>, Error> {
    Ok(commands::get_all_commit_reactions(&manager, &id).await)
}

#[tauri::command(async)]
#[instrument(skip(manager), err(Debug))]
pub async fn irc_get_all_message_reactions(
    manager: State<'_, IrcManager>,
    id: String,
) -> Result<HashMap<String, Vec<CommitReaction>>, Error> {
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
pub async fn irc_get_file_message_reactions(
    manager: State<'_, IrcManager>,
    id: String,
    file_path: String,
) -> Result<HashMap<String, Vec<CommitReaction>>, Error> {
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
