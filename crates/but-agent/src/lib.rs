//! A butler agent

pub mod agent;
pub mod llm;
pub mod openai_like_llm;
pub mod store;
pub mod types;

#[cfg(test)]
mod test {
    use crate::agent::*;
    use crate::llm::{LLMResponse, *};
    use crate::store::{ConversationStore as _, test::*};
    use crate::types::*;

    fn system_prompt() -> String {
        "You are a great agent :flower:".into()
    }

    #[test]
    fn tool_call_and_response() {
        let available_tools = [Tool {
            tool_type: ToolType::Function,
            function: ToolFunction {
                name: "foo".into(),
                description: "it does foo".into(),
                parameters: ToolFunctionParameters {
                    parameters_type: ToolFunctionParametersType::Object,
                    properties: std::collections::BTreeMap::new(),
                    additional_properties: true,
                    required: vec![],
                },
                strict: false,
            },
        }];

        let available_tools_with_handler = available_tools
            .iter()
            .map(|t| ToolWithHandler {
                tool: t.clone(),
                handler: ToolHandler::RawHandler(Box::new(|input| format!("handled: {}", input))),
            })
            .collect();

        let llm = MockLLM {
            callback: move |params| {
                if params.messages.last().unwrap().content == "call tool" {
                    LLMResponse::ToolCalls {
                        message: "tools have been called".into(),
                        tool_calls: vec![ToolCall {
                            id: "69".into(),
                            tool_call_type: ToolCallType::Function,
                            function: ToolCallFunction {
                                name: "foo".into(),
                                arguments: "args".into(),
                            },
                        }],
                    }
                } else {
                    LLMResponse::Message {
                        message: "finished".into(),
                    }
                }
            },
        };

        let mut conversation_store = InMemoryConversationStore::new();

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let mut config = AgentConfig {
            llm: &llm,
            conversation_store: &mut conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: available_tools_with_handler,
        };

        agent_perform(&mut config, Action::StartNewThread);

        let id = responses.lock().unwrap().last().unwrap().id();

        agent_perform(
            &mut config,
            Action::SendMessage {
                id,
                message: "call tool".into(),
            },
        );

        assert_eq!(
            conversation_store.read(id).unwrap(),
            vec![
                Message {
                    role: MessageRole::System,
                    content: system_prompt(),
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::User,
                    content: "call tool".into(),
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::Assistant,
                    content: "tools have been called".into(),
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::Tool,
                    content: "handled: args".into(),
                    tool_call_id: Some("69".into()),
                },
                Message {
                    role: MessageRole::Assistant,
                    content: "finished".into(),
                    tool_call_id: None,
                },
            ],
        );

        assert_eq!(
            *responses.lock().unwrap(),
            vec![
                Response::ThreadCreated { id },
                Response::MessageRecieved { id },
                Response::ToolCallReplyRecieved { id },
                Response::ToolCallResponseCreated { id },
                Response::ReplyReceived { id },
            ]
        );
    }

    #[test]
    fn llm_recieved_tools() {
        let available_tools = vec![Tool {
            tool_type: ToolType::Function,
            function: ToolFunction {
                name: "foo".into(),
                description: "it does foo".into(),
                parameters: ToolFunctionParameters {
                    parameters_type: ToolFunctionParametersType::Object,
                    properties: std::collections::BTreeMap::new(),
                    additional_properties: false,
                    required: vec![],
                },
                strict: true,
            },
        }];

        let available_tools_with_handler = available_tools
            .iter()
            .map(|t| ToolWithHandler {
                tool: t.clone(),
                handler: ToolHandler::RawHandler(Box::new(|_| "".into())),
            })
            .collect();

        let moved_available_tools = available_tools.clone();
        let llm = MockLLM {
            callback: move |params| {
                assert_eq!(params.tools, moved_available_tools);
                LLMResponse::Message { message: "".into() }
            },
        };

        let mut conversation_store = InMemoryConversationStore::new();

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let mut config = AgentConfig {
            llm: &llm,
            conversation_store: &mut conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: available_tools_with_handler,
        };

        agent_perform(&mut config, Action::StartNewThread);

        let id = responses.lock().unwrap().last().unwrap().id();

        agent_perform(
            &mut config,
            Action::SendMessage {
                id,
                message: "Hello!".into(),
            },
        );
        assert_eq!(responses.lock().unwrap().len(), 3);
    }

    #[test]
    fn send_message() {
        let llm = MockLLM {
            callback: |params| LLMResponse::Message {
                message: format!(
                    "Mocked response: {}",
                    params.messages.last().unwrap().content
                ),
            },
        };
        let mut conversation_store = InMemoryConversationStore::new();

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let mut config = AgentConfig {
            llm: &llm,
            conversation_store: &mut conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: vec![],
        };

        agent_perform(&mut config, Action::StartNewThread);

        let id = responses.lock().unwrap().last().unwrap().id();

        agent_perform(
            &mut config,
            Action::SendMessage {
                id,
                message: "Hello!".into(),
            },
        );

        assert_eq!(responses.lock().unwrap().len(), 3);
        assert!(matches!(
            responses.lock().unwrap().last().unwrap(),
            Response::ReplyReceived { .. }
        ));

        assert_eq!(
            conversation_store.read(id).unwrap(),
            vec![
                Message {
                    role: MessageRole::System,
                    content: system_prompt(),
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::User,
                    content: "Hello!".into(),
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::Assistant,
                    content: "Mocked response: Hello!".into(),
                    tool_call_id: None,
                },
            ],
        );
    }

    #[test]
    fn create_thread() {
        let llm = MockLLM {
            callback: |params| LLMResponse::Message {
                message: format!(
                    "Mocked response: {}",
                    params.messages.last().unwrap().content
                ),
            },
        };
        let mut conversation_store = InMemoryConversationStore::new();

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let mut config = AgentConfig {
            llm: &llm,
            conversation_store: &mut conversation_store,
            callback,
            system_prompt: system_prompt(),
            tools: vec![],
        };

        agent_perform(&mut config, Action::StartNewThread);

        assert_eq!(responses.lock().unwrap().len(), 1);
        assert!(matches!(
            responses.lock().unwrap().last().unwrap(),
            Response::ThreadCreated { .. }
        ));

        let id = responses.lock().unwrap().last().unwrap().id();

        assert_eq!(
            conversation_store.read(id).unwrap(),
            vec![Message {
                role: MessageRole::System,
                content: system_prompt(),
                tool_call_id: None,
            }],
        );
    }
}
