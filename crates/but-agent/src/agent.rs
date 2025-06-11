use crate::llm::{LLM, LLMParams, LLMResponse};
use crate::store::ConversationStore;
use crate::types::{Action, ConversationId, Message, MessageRole, Response, ToolWithHandler};
use anyhow::Result;

pub struct AgentConfig<'a, CB: Fn(Response) + Send + 'static> {
    pub llm: &'a dyn LLM,
    pub conversation_store: &'a mut dyn ConversationStore,
    pub callback: CB,
    pub system_prompt: String,
    pub tools: Vec<ToolWithHandler>,
}

pub fn agent_perform<CB: Fn(Response) + Send + 'static>(
    AgentConfig {
        llm,
        conversation_store,
        callback,
        system_prompt,
        tools,
    }: &mut AgentConfig<CB>,
    action: Action,
) -> Result<()> {
    let mut responding_to_tools = false;
    loop {
        match &action {
            Action::StartNewThread => {
                let id = ConversationId::generate();
                conversation_store.write(
                    id,
                    &[Message {
                        role: MessageRole::System,
                        content: system_prompt.clone(),
                        tool_call_id: None,
                    }],
                );
                callback(Response::ThreadCreated { id });
                break;
            }
            Action::SendMessage { id, message } => {
                // Persist and acknowledge the message
                let messages = {
                    let mut messages: Vec<Message> = conversation_store.read(*id)?;
                    if !responding_to_tools {
                        messages.push(Message {
                            role: MessageRole::User,
                            content: message.to_string(),
                            tool_call_id: None,
                        });
                        conversation_store.write(*id, &messages);
                        callback(Response::MessageRecieved { id: *id });
                    }
                    messages
                };

                let response = llm.perform(LLMParams {
                    messages,
                    tools: tools.iter().map(|t| t.tool.clone()).collect(),
                })?;

                match response {
                    LLMResponse::Message { message } => {
                        let mut messages = conversation_store.read(*id)?;
                        messages.push(Message {
                            role: MessageRole::Assistant,
                            content: message,
                            tool_call_id: None,
                        });
                        conversation_store.write(*id, &messages);
                        callback(Response::ReplyReceived { id: *id });
                        break;
                    }
                    LLMResponse::ToolCalls {
                        message,
                        tool_calls,
                    } => {
                        responding_to_tools = true;
                        let mut messages = conversation_store.read(*id)?;
                        messages.push(Message {
                            role: MessageRole::Assistant,
                            content: message,
                            tool_call_id: None,
                        });
                        conversation_store.write(*id, &messages);
                        callback(Response::ToolCallReplyRecieved { id: *id });

                        for tool_call in tool_calls {
                            let tool = tools
                                .iter()
                                .find(|t| t.tool.function.name == tool_call.function.name);
                            if let Some(tool) = tool {
                                let result = tool.handler.call(tool_call.function.arguments);
                                messages.push(Message {
                                    role: MessageRole::Tool,
                                    content: result,
                                    tool_call_id: Some(tool_call.id),
                                });
                            } else {
                                messages.push(Message {
                                    role: MessageRole::Assistant,
                                    content: "Tool not found".to_string(),
                                    tool_call_id: Some(tool_call.id),
                                });
                            }
                            conversation_store.write(*id, &messages);
                            callback(Response::ToolCallResponseCreated { id: *id });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
