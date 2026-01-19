use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use but_tools::tool::Toolset;

use crate::ChatMessage;

pub trait LLMClient: Debug + Clone {
    fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: String,
        on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
    ) -> Result<(String, Vec<ChatMessage>)>;
}
