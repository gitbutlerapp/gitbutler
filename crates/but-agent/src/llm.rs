use crate::types::{Message, Tool, ToolCall};

pub enum LLMParams {
    Message {
        messages: Vec<Message>,
        tools: Vec<Tool>,
    },
}

pub enum LLMResponse {
    Message {
        message: String,
    },
    ToolCalls {
        message: String,
        tool_calls: Vec<ToolCall>,
    },
}

pub trait LLM {
    fn perform(&self, params: LLMParams) -> LLMResponse;
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    pub struct MockLLM<CB: Fn(String) -> String, TC: Fn(Vec<Tool>)> {
        pub callback: CB,
        pub tools_callback: TC,
    }

    impl<CB: Fn(String) -> String, TC: Fn(Vec<Tool>)> LLM for MockLLM<CB, TC> {
        fn perform(&self, params: LLMParams) -> LLMResponse {
            match params {
                LLMParams::Message { messages, tools } => {
                    let last = messages.last().unwrap();
                    (self.tools_callback)(tools);
                    LLMResponse::Message {
                        message: (self.callback)(last.content.clone()),
                    }
                }
            }
        }
    }
}
