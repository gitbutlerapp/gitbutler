use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};

use crate::tool::Tool;

impl TryFrom<&dyn Tool> for ChatCompletionTool {
    type Error = anyhow::Error;
    fn try_from(tool: &dyn Tool) -> Result<ChatCompletionTool, Self::Error> {
        let tool = ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: tool.name(),
                description: Some(tool.description()),
                parameters: Some(tool.parameters()),
                strict: Some(false),
            },
        };

        Ok(tool)
    }
}
