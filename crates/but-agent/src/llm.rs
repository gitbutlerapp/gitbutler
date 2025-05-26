use crate::types::{Message, Tool, ToolCall};

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
    fn perform(&self, params: LLMParams) -> LLMResponse;
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    pub struct MockLLM<CB: Fn(LLMParams) -> LLMResponse> {
        pub callback: CB,
    }

    impl<CB: Fn(LLMParams) -> LLMResponse> LLM for MockLLM<CB> {
        fn perform(&self, params: LLMParams) -> LLMResponse {
            (self.callback)(params)
        }
    }
}
