use async_openai::types::{FunctionObject, FunctionObjectArgs};

use crate::tool::Tool;

impl TryFrom<&dyn Tool<Output = Box<dyn std::any::Any + Send + Sync>>> for FunctionObject {
    type Error = anyhow::Error;
    fn try_from(
        tool: &dyn Tool<Output = Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<FunctionObject, Self::Error> {
        let func = FunctionObjectArgs::default()
            .name(tool.name())
            .description(tool.description())
            .parameters(tool.parameters())
            .build()?;
        Ok(func)
    }
}
