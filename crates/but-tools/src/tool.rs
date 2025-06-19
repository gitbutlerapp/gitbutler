use std::{collections::BTreeMap, sync::Arc};


pub struct Toolset {
    tools: BTreeMap<String, Arc<dyn Tool<Output = Box<dyn std::any::Any + Send + Sync>>>>,
}

impl Toolset {
    pub fn new() -> Arc<Self> {
        Arc::new(Toolset {
            tools: BTreeMap::new(),
        })
    }
    pub fn register_tool<T: Tool<Output = Box<dyn std::any::Any + Send + Sync>>>(
        &mut self,
        tool: T,
    ) {
        self.tools.insert(tool.name(), Arc::new(tool));
    }
    pub fn get(
        &self,
        name: &str,
    ) -> Option<Arc<dyn Tool<Output = Box<dyn std::any::Any + Send + Sync>>>> {
        self.tools.get(name).cloned()
    }
    pub fn list(&self) -> Vec<Arc<dyn Tool<Output = Box<dyn std::any::Any + Send + Sync>>>> {
        self.tools.values().cloned().collect()
    }
}

pub trait Tool: 'static + Send + Sync {
    type Output;
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn parameters(&self) -> serde_json::Value;
    fn call(self: Arc<Self>, parameters: serde_json::Value) -> anyhow::Result<Self::Output>;
}
