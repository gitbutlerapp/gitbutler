use but_workspace::StackId;
use gitbutler_project::ProjectId;
use tauri::Emitter;

pub trait EmitStackUpdate {
    /// Emits a stack update event with the given stack ID.
    ///
    /// This method should be implemented to emit an event that updates the stack details in the UI.
    ///
    /// # Arguments
    ///
    /// * `project_id` - The ID of the project to which the stack belongs.
    /// * `stack_id` - The ID of the stack to update.
    fn emit_stack_update(&self, project_id: ProjectId, stack_id: StackId);
}

impl EmitStackUpdate for tauri::AppHandle {
    fn emit_stack_update(&self, project_id: ProjectId, stack_id: StackId) {
        let name = format!("project://{}/stack_details_update", project_id);
        let payload = serde_json::json!({ "stackId": stack_id });
        self.emit(&name, payload)
            .expect("Failed to emit stack details update");
    }
}

pub struct ToolCall {
    pub name: String,
    pub parameters: String,
    pub result: String,
}

pub trait EmitToolCall {
    /// Emits a tool call event with the given project ID and tool call content.
    ///
    /// This method should be implemented to emit an event that notifies the UI about a tool call.
    ///
    /// # Arguments
    ///
    /// * `project_id` - The ID of the project where the tool call is made.
    /// * `message_id` - The ID of the message associated with the tool call.
    /// * `tool_call` - The content of the tool call to emit.
    fn emit_tool_call(&self, project_id: ProjectId, message_id: String, tool_call: ToolCall);
}

impl EmitToolCall for tauri::AppHandle {
    fn emit_tool_call(&self, project_id: ProjectId, message_id: String, tool_call: ToolCall) {
        let name = format!("project://{}/tool-call", project_id);
        let payload = serde_json::json!({
            "messageId": message_id,
            "name": tool_call.name,
            "parameters": tool_call.parameters,
            "result": tool_call.result,
        });
        self.emit(&name, payload)
            .expect("Failed to emit tool call event");
    }
}
