use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};

use crate::tool::Tool;

impl TryFrom<&dyn Tool> for ChatCompletionTools {
    type Error = anyhow::Error;
    fn try_from(tool: &dyn Tool) -> Result<ChatCompletionTools, Self::Error> {
        let tool = ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: tool.name(),
                description: Some(tool.description()),
                parameters: Some(tool.parameters()),
                strict: Some(false),
            },
        });

        Ok(tool)
    }
}
