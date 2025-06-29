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
