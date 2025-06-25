use std::{collections::BTreeMap, sync::Arc};

use gitbutler_command_context::CommandContext;

pub struct Toolset<'a> {
    ctx: &'a mut CommandContext,
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl<'a> Toolset<'a> {
    pub fn new(ctx: &'a mut CommandContext) -> Self {
        Toolset {
            ctx,
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
        tool.call(params, self.ctx)
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
    ) -> anyhow::Result<serde_json::Value>;
}
