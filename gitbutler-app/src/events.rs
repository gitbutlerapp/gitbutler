use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    deltas,
    projects::ProjectId,
    reader,
    sessions::{self, SessionId},
    virtual_branches,
};

#[derive(Clone)]
pub struct Sender {
    app_handle: tauri::AppHandle,
}

impl TryFrom<&AppHandle> for Sender {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(sender) = value.try_state::<Sender>() {
            Ok(sender.inner().clone())
        } else {
            let sender = Sender::new(value.clone());
            value.manage(sender.clone());
            Ok(sender)
        }
    }
}

impl Sender {
    fn new(app_handle: AppHandle) -> Sender {
        Sender { app_handle }
    }

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
    project_id: ProjectId,
}

impl Event {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn git_index(project_id: &ProjectId) -> Self {
        Event {
            name: format!("project://{}/git/index", project_id),
            payload: serde_json::json!({}),
            project_id: *project_id,
        }
    }

    pub fn git_fetch(project_id: &ProjectId) -> Self {
        Event {
            name: format!("project://{}/git/fetch", project_id),
            payload: serde_json::json!({}),
            project_id: *project_id,
        }
    }

    pub fn git_head(project_id: &ProjectId, head: &str) -> Self {
        Event {
            name: format!("project://{}/git/head", project_id),
            payload: serde_json::json!({ "head": head }),
            project_id: *project_id,
        }
    }

    pub fn git_activity(project_id: &ProjectId) -> Self {
        Event {
            name: format!("project://{}/git/activity", project_id),
            payload: serde_json::json!({}),
            project_id: *project_id,
        }
    }

    pub fn file(
        project_id: &ProjectId,
        session_id: &SessionId,
        file_path: &str,
        contents: Option<&reader::Content>,
    ) -> Self {
        Event {
            name: format!("project://{}/sessions/{}/files", project_id, session_id),
            payload: serde_json::json!({
                "filePath": file_path,
                "contents": contents,
            }),
            project_id: *project_id,
        }
    }

    pub fn session(project_id: &ProjectId, session: &sessions::Session) -> Self {
        Event {
            name: format!("project://{}/sessions", project_id),
            payload: serde_json::to_value(session).unwrap(),
            project_id: *project_id,
        }
    }

    pub fn deltas(
        project_id: &ProjectId,
        session_id: &SessionId,
        deltas: &Vec<deltas::Delta>,
        relative_file_path: &std::path::Path,
    ) -> Self {
        Event {
            name: format!("project://{}/sessions/{}/deltas", project_id, session_id),
            payload: serde_json::json!({
                "deltas": deltas,
                "filePath": relative_file_path,
            }),
            project_id: *project_id,
        }
    }

    pub fn virtual_branches(
        project_id: &ProjectId,
        virtual_branches: &virtual_branches::VirtualBranches,
    ) -> Self {
        Event {
            name: format!("project://{}/virtual-branches", project_id),
            payload: serde_json::json!(virtual_branches),
            project_id: *project_id,
        }
    }
}
