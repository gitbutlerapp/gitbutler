use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{bookmarks, deltas, reader, sessions};

#[derive(Clone)]
pub struct Sender {
    app_handle: tauri::AppHandle,
}

impl From<&AppHandle> for Sender {
    fn from(value: &AppHandle) -> Self {
        Self {
            app_handle: value.clone(),
        }
    }
}

impl Sender {
    pub fn send(&self, event: &Event) -> Result<()> {
        self.app_handle
            .emit_all(&event.name, Some(&event.payload))
            .context("emit event")?;
        tracing::debug!(event_name = event.name, "sent event");
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    name: String,
    payload: serde_json::Value,
    project_id: String,
}

impl Event {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn git_index(project_id: &str) -> Self {
        Event {
            name: format!("project://{}/git/index", project_id),
            payload: serde_json::json!({}),
            project_id: project_id.to_string(),
        }
    }

    pub fn git_fetch(project_id: &str) -> Self {
        Event {
            name: format!("project://{}/git/fetch", project_id),
            payload: serde_json::json!({}),
            project_id: project_id.to_string(),
        }
    }

    pub fn git_head(project_id: &str, head: &str) -> Self {
        Event {
            name: format!("project://{}/git/head", project_id),
            payload: serde_json::json!({ "head": head }),
            project_id: project_id.to_string(),
        }
    }

    pub fn git_activity(project_id: &str) -> Self {
        Event {
            name: format!("project://{}/git/activity", project_id),
            payload: serde_json::json!({}),
            project_id: project_id.to_string(),
        }
    }

    pub fn file(
        project_id: &str,
        session_id: &str,
        file_path: &str,
        contents: Option<&reader::Content>,
    ) -> Self {
        Event {
            name: format!("project://{}/sessions/{}/files", project_id, session_id),
            payload: serde_json::json!({
                "filePath": file_path,
                "contents": contents,
            }),
            project_id: project_id.to_string(),
        }
    }

    pub fn session(project_id: &str, session: &sessions::Session) -> Self {
        Event {
            name: format!("project://{}/sessions", project_id),
            payload: serde_json::to_value(session).unwrap(),
            project_id: project_id.to_string(),
        }
    }

    pub fn bookmark(project_id: &str, bookmark: &bookmarks::Bookmark) -> Self {
        Event {
            name: format!("project://{}/bookmarks", project_id),
            payload: serde_json::to_value(bookmark).unwrap(),
            project_id: project_id.to_string(),
        }
    }

    pub fn deltas(
        project_id: &str,
        session_id: &str,
        deltas: &Vec<deltas::Delta>,
        relative_file_path: &std::path::Path,
    ) -> Self {
        Event {
            name: format!("project://{}/sessions/{}/deltas", project_id, session_id),
            payload: serde_json::json!({
                "deltas": deltas,
                "filePath": relative_file_path,
            }),
            project_id: project_id.to_string(),
        }
    }
}
