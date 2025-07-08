use std::{collections::BTreeMap, sync::Arc};

use but_workspace::ui::StackEntry;
use gitbutler_command_context::CommandContext;
use serde_json::json;

pub struct Toolset<'a> {
    ctx: &'a mut CommandContext,
    app_handle: Option<&'a tauri::AppHandle>,
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl<'a> Toolset<'a> {
    pub fn new(ctx: &'a mut CommandContext, app_handle: Option<&'a tauri::AppHandle>) -> Self {
        Toolset {
            ctx,
            app_handle,
            tools: BTreeMap::new(),
        }
    }

    pub fn register_tool<T: Tool>(&mut self, tool: T) {
        self.tools.insert(tool.name(), Arc::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    pub fn call_tool(&mut self, name: &str, parameters: &str) -> anyhow::Result<serde_json::Value> {
        let tool = self
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", name))?;
        let params: serde_json::Value = serde_json::from_str(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse parameters: {}", e))?;
        tool.call(params, self.ctx, self.app_handle)
    }
}

pub trait Tool: 'static + Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn parameters(&self) -> serde_json::Value;
    fn call(
        self: Arc<Self>,
        parameters: serde_json::Value,
        ctx: &mut CommandContext,
        app_handle: Option<&tauri::AppHandle>,
    ) -> anyhow::Result<serde_json::Value>;
}

pub fn error_to_json(error: &anyhow::Error, action_identifier: &str) -> serde_json::Value {
    serde_json::json!({
        "error": format!("Failed to {}: {}", action_identifier, error.to_string())
    })
}

pub fn result_to_json<T: serde::Serialize>(
    result: &Result<T, anyhow::Error>,
    action_identifier: &str,
    data_identifier: &str,
) -> serde_json::Value {
    match result {
        Ok(entry) => json!({ "result": serde_json::to_value(entry).unwrap_or_else(
            |e| json!({ "error": format!("Failed to serialize {}: {}", data_identifier, e.to_string())}),
        )}),
        Err(e) => error_to_json(e, action_identifier),
    }
}

pub trait ToolResult: 'static + Send + Sync {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value;
}

impl ToolResult for Result<StackEntry, anyhow::Error> {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value {
        result_to_json(self, action_identifier, "StackEntry")
    }
}

impl ToolResult for Result<but_workspace::commit_engine::ui::CreateCommitOutcome, anyhow::Error> {
    fn to_json(&self, action_identifier: &str) -> serde_json::Value {
        result_to_json(self, action_identifier, "CreateCommitOutcome")
    }
}
