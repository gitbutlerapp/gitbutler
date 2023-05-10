use anyhow::{Context, Result};
use tauri::Manager;

use crate::app::{deltas, sessions};

#[derive(Clone)]
pub struct Sender {
    app_handle: tauri::AppHandle,
}

impl Sender {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn send(&self, event: Event) -> Result<()> {
        self.app_handle
            .emit_all(&event.name, Some(event.payload))
            .context("emit event")
    }
}

#[derive(Debug)]
pub struct Event {
    name: String,
    payload: serde_json::Value,
}

impl Event {
    pub fn session(project_id: &str, session: &sessions::Session) -> Self {
        let event_name = format!("project://{}/sessions", project_id);
        Event {
            name: event_name,
            payload: serde_json::to_value(session).unwrap(),
        }
    }

    pub fn git_index(project_id: &str) -> Self {
        let event_name = format!("project://{}/git/index", project_id);
        Event {
            name: event_name,
            payload: serde_json::json!({}),
        }
    }

    pub fn git_head(project_id: &str, head: &str) -> Self {
        let event_name = format!("project://{}/git/head", project_id);
        Event {
            name: event_name,
            payload: serde_json::json!({ "head": head }),
        }
    }

    pub fn git_activity(project_id: &str) -> Self {
        let event_name = format!("project://{}/git/activity", project_id);
        Event {
            name: event_name,
            payload: serde_json::json!({}),
        }
    }

    pub fn detlas(
        project_id: &str,
        session: &sessions::Session,
        deltas: &Vec<deltas::Delta>,
        relative_file_path: &std::path::Path,
    ) -> Self {
        let event_name = format!("project://{}/sessions/{}/deltas", project_id, session.id);
        let payload = serde_json::json!({
            "deltas": deltas,
            "filePath": relative_file_path,
        });
        Event {
            name: event_name,
            payload,
        }
    }
}
