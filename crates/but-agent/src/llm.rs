use crate::types::{Message, Tool, ToolCall};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct LLMParams {
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LLMResponse {
    Message {
        message: String,
    },
    ToolCalls {
        message: String,
        tool_calls: Vec<ToolCall>,
    },
}

#[allow(clippy::upper_case_acronyms)]
pub trait LLM {
    fn perform(&self, params: LLMParams) -> Result<LLMResponse>;
}

// #[cfg(test)]
// pub(crate) mod test {
//     use super::*;

pub struct MockLLM<CB: Fn(LLMParams) -> LLMResponse> {
    pub callback: CB,
}

impl<CB: Fn(LLMParams) -> LLMResponse> LLM for MockLLM<CB> {
    fn perform(&self, params: LLMParams) -> Result<LLMResponse> {
        Ok((self.callback)(params))
    }
}
// }
