use gitbutler_project::ProjectId;
use tauri::Emitter;

pub trait EmitTokenEvent {
    /// Emits an event with the Gen AI token.
    ///
    /// This method should be implemented to emit an event that updates the Gen AI token in the UI.
    /// # Arguments
    ///  * `project_id` - The ID of the project to which the token belongs.
    ///  * `message_id` - The ID of the message that is being responded to.
    /// * `token` - The token to emit.
    fn emit_token_event(&self, token: &str, project_id: ProjectId, message_id: String);
}

impl EmitTokenEvent for tauri::AppHandle {
    fn emit_token_event(&self, token: &str, project_id: ProjectId, message_id: String) {
        let name = format!("project://{}/token-updates", project_id);
        let payload = serde_json::json!({
            "messageId": message_id,
            "token": token,
        });
        self.emit(&name, payload)
            .expect("Failed to emit token event");
    }
}
