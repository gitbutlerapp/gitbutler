//! Backend-driven working-files broadcast.
//!
//! Sends the user's modified file list to the project IRC channel
//! so other users can see what files each person is working on.
//! Uses full sync on start and debounced deltas on worktree changes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use but_ctx::ProjectHandleOrLegacyProjectId as ProjectId;
use but_irc::IrcManager;
use serde_json::json;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::debug;

/// Per-project broadcast state.
struct ActiveBroadcast {
    connection_id: String,
    channel: String,
    previous_files: HashSet<String>,
    /// Handle to the pending debounced send, if any.
    debounce_handle: Option<JoinHandle<()>>,
}

/// Manages broadcasting worktree file lists to IRC channels.
#[derive(Clone)]
pub struct WorkingFilesBroadcast {
    state: Arc<RwLock<HashMap<ProjectId, ActiveBroadcast>>>,
    irc_manager: IrcManager,
}

const DEBOUNCE_MS: u64 = 2000;

impl WorkingFilesBroadcast {
    pub fn new(irc_manager: IrcManager) -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            irc_manager,
        }
    }

    /// Start broadcasting working files for a project.
    /// Sends an initial full sync with the given file list.
    pub async fn start(
        &self,
        project_id: ProjectId,
        connection_id: String,
        channel: String,
        initial_files: Vec<String>,
    ) {
        let file_set: HashSet<String> = initial_files.into_iter().collect();
        let file_count = file_set.len();

        let mut state = self.state.write().await;
        // Stop any existing broadcast for this project.
        if let Some(old) = state.remove(&project_id) {
            if let Some(h) = old.debounce_handle {
                h.abort();
            }
        }

        state.insert(
            project_id,
            ActiveBroadcast {
                connection_id: connection_id.clone(),
                channel: channel.clone(),
                previous_files: file_set.clone(),
                debounce_handle: None,
            },
        );
        drop(state);

        // Send initial sync.
        let files_vec: Vec<String> = file_set.into_iter().collect();
        let text = format!(
            "\u{1f4c2} working on {} file{}",
            file_count,
            if file_count != 1 { "s" } else { "" }
        );
        let payload = json!({
            "type": "working-files-sync",
            "files": files_vec,
        });
        self.send_data(&connection_id, &channel, &text, &payload)
            .await;
    }

    /// Stop broadcasting for a project.
    pub async fn stop(&self, project_id: ProjectId) {
        let mut state = self.state.write().await;
        if let Some(old) = state.remove(&project_id) {
            if let Some(h) = old.debounce_handle {
                h.abort();
            }
        }
    }

    /// Called synchronously from the file watcher handler when worktree changes
    /// are detected. Spawns a debounced async task to compute and send the delta.
    pub fn on_worktree_change(&self, project_id: ProjectId, file_paths: Vec<String>) {
        let this = self.clone();
        tokio::spawn(async move {
            this.handle_worktree_change(project_id, file_paths).await;
        });
    }

    async fn handle_worktree_change(&self, project_id: ProjectId, file_paths: Vec<String>) {
        let mut state = self.state.write().await;
        let Some(broadcast) = state.get_mut(&project_id) else {
            return;
        };

        // Cancel any pending debounce.
        if let Some(h) = broadcast.debounce_handle.take() {
            h.abort();
        }

        let new_files: HashSet<String> = file_paths.into_iter().collect();
        let connection_id = broadcast.connection_id.clone();
        let channel = broadcast.channel.clone();

        // Don't update previous_files here — only the debounce task that
        // actually fires should update it. Otherwise, if a second watcher
        // event cancels and replaces the first debounce task, the diff
        // baseline is lost and the replacement task sees no change.
        let broadcast_state = self.state.clone();
        let irc_manager = self.irc_manager.clone();
        let spawn_project_id = project_id.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(DEBOUNCE_MS)).await;

            // Read previous_files and update it atomically at send time.
            let mut state = broadcast_state.write().await;
            let Some(broadcast) = state.get_mut(&spawn_project_id) else {
                return;
            };

            let added: Vec<String> = new_files
                .difference(&broadcast.previous_files)
                .cloned()
                .collect();
            let removed: Vec<String> = broadcast
                .previous_files
                .difference(&new_files)
                .cloned()
                .collect();

            broadcast.previous_files = new_files;
            broadcast.debounce_handle = None;
            drop(state);

            if added.is_empty() && removed.is_empty() {
                return;
            }

            let text = format!("\u{1f4c2} +{} -{} files", added.len(), removed.len());
            let payload = json!({
                "type": "working-files-delta",
                "added": added,
                "removed": removed,
            });
            let data = encode_payload(&payload);
            let _ = irc_manager
                .send_message_with_data(&connection_id, &channel, &text, &data, None)
                .await;
            debug!(
                "Working files delta sent to {} (added: {}, removed: {})",
                channel,
                added.len(),
                removed.len()
            );
        });

        // Store the debounce handle so we can cancel it if another change comes in.
        if let Some(broadcast) = state.get_mut(&project_id) {
            broadcast.debounce_handle = Some(handle);
        }
    }

    async fn send_data(
        &self,
        connection_id: &str,
        channel: &str,
        text: &str,
        payload: &serde_json::Value,
    ) {
        let data = encode_payload(payload);
        let _ = self
            .irc_manager
            .send_message_with_data(connection_id, channel, text, &data, None)
            .await;
    }
}

/// Serialize a JSON value to a string for use as an IRC data payload.
/// Base64 encoding is handled at the wire boundary by `IrcClient::send_message_with_data`.
fn encode_payload(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
