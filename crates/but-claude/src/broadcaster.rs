use but_db::poll::ItemKind;
use gitbutler_project::ProjectId;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

impl FrontendEvent {
    /// Converts a database ItemKind to a FrontendEvent that can be sent over WebSocket.
    /// This matches the implementation used in both gitbutler-tauri and but-server for consistency.
    pub fn from_db_item(project_id: ProjectId, item: ItemKind) -> Self {
        match item {
            ItemKind::Actions => FrontendEvent {
                name: format!("project://{project_id}/db-updates"),
                payload: json!({
                    "kind": "actions"
                }),
            },
            ItemKind::Workflows => FrontendEvent {
                name: format!("project://{project_id}/db-updates"),
                payload: json!({
                    "kind": "workflows"
                }),
            },
            ItemKind::Assignments => FrontendEvent {
                name: format!("project://{project_id}/hunk-assignment-update"),
                payload: json!({
                    "kind": "hunk-assignments"
                }),
            },
            ItemKind::Rules => FrontendEvent {
                name: format!("project://{project_id}/rule-updates"),
                payload: json!({
                    "kind": "rules"
                }),
            },
            ItemKind::ClaudePermissionRequests => FrontendEvent {
                name: format!("project://{project_id}/claude-permission-requests"),
                payload: json!({
                    "kind": "claude-permission-requests"
                }),
            },
            _ => {
                tracing::warn!("Unhandled ItemKind in from_db_item: {item:?}");
                FrontendEvent {
                    name: format!("project://{project_id}/db-updates"),
                    payload: json!({
                        "kind": "unknown",
                        "item": format!("{:?}", item)
                    }),
                }
            }
        }
    }
}

pub(super) mod types {
    use super::FrontendEvent;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct Broadcaster {
        pub(super) senders: HashMap<uuid::Uuid, tokio::sync::mpsc::UnboundedSender<FrontendEvent>>,
    }
}
use types::Broadcaster;

impl Broadcaster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send(&self, event: FrontendEvent) {
        for sender in self.senders.values() {
            let _ = sender.send(event.clone());
        }
    }

    pub fn register_sender(
        &mut self,
        id: &uuid::Uuid,
        sender: tokio::sync::mpsc::UnboundedSender<FrontendEvent>,
    ) {
        self.senders.insert(*id, sender);
    }

    pub fn deregister_sender(&mut self, id: &uuid::Uuid) {
        self.senders.remove(id);
    }
}
