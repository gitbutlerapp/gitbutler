use crate::tool::Tool;

impl TryFrom<&dyn Tool> for ollama_rs::generation::tools::ToolInfo {
    type Error = anyhow::Error;
    fn try_from(tool: &dyn Tool) -> Result<ollama_rs::generation::tools::ToolInfo, Self::Error> {
        Ok(ollama_rs::generation::tools::ToolInfo {
            tool_type: ollama_rs::generation::tools::ToolType::Function,
            function: ollama_rs::generation::tools::ToolFunctionInfo {
                name: tool.name(),
                description: tool.description(),
                parameters: tool.parameters().try_into()?,
            },
        })
    }
}
