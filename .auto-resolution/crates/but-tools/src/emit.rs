use but_core::ref_metadata::StackId;
use gitbutler_project::ProjectId;

pub type Emitter = dyn Fn(&str, serde_json::Value) + Send + Sync + 'static;
pub trait Emittable {
    fn emittable(&self) -> (String, serde_json::Value);
}

pub struct StackUpdate {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

impl Emittable for StackUpdate {
    fn emittable(&self) -> (String, serde_json::Value) {
        let name = format!("project://{}/stack_details_update", self.project_id);
        let payload = serde_json::json!({ "stackId": self.stack_id });
        (name, payload)
    }
}

pub struct ToolCall {
    pub project_id: ProjectId,
    pub message_id: String,
    pub name: String,
    pub parameters: String,
    pub result: String,
}

impl Emittable for ToolCall {
    fn emittable(&self) -> (String, serde_json::Value) {
        let name = format!("project://{}/tool-call", self.project_id);
        let payload = serde_json::json!({
            "messageId": self.message_id,
            "name": self.name,
            "parameters": self.parameters,
            "result": self.result,
        });
        (name, payload)
    }
}

pub struct TokenUpdate {
    pub token: String,
    pub project_id: ProjectId,
    pub message_id: String,
}

impl Emittable for TokenUpdate {
    fn emittable(&self) -> (String, serde_json::Value) {
        let name = format!("project://{}/token-updates", self.project_id);
        let payload = serde_json::json!({
            "messageId": self.message_id,
            "token": self.token,
        });
        (name, payload)
    }
}

pub struct TokenEnd {
    pub project_id: ProjectId,
    pub message_id: String,
}

impl Emittable for TokenEnd {
    fn emittable(&self) -> (String, serde_json::Value) {
        let name = format!("project://{}/token-updates", self.project_id);
        let payload = serde_json::json!({
            "messageId": self.message_id,
            "token": "\n\n---\n\n"
        });
        (name, payload)
    }
}

pub struct TodoState {
    pub id: String,
    pub title: String,
    pub status: String,
}

pub struct TodoUpdate {
    pub project_id: ProjectId,
    pub message_id: String,
    pub list: Vec<TodoState>,
}

impl Emittable for TodoUpdate {
    fn emittable(&self) -> (String, serde_json::Value) {
        let name = format!("project://{}/todo-updates", self.project_id);
        let payload = serde_json::json!({
            "messageId": self.message_id,
            "list": self.list.iter().map(|todo| {
                serde_json::json!({
                    "id": todo.id,
                    "title": todo.title,
                    "status": todo.status,
                })
            }).collect::<Vec<_>>(),
        });
        (name, payload)
    }
}
