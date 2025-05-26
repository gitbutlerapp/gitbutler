use crate::llm::{LLM, LLMParams, LLMResponse};
use crate::store::ConversationStore;
use crate::types::{Action, ConversationId, Message, MessageRole, Response};

pub struct AgentConfig<CB: Fn(Response) + Send + 'static> {
    pub llm: Box<dyn LLM>,
    pub conversation_store: std::cell::RefCell<Box<dyn ConversationStore>>,
    pub callback: CB,
    pub system_prompt: String,
}

pub fn agent_perform<CB: Fn(Response) + Send + 'static>(
    AgentConfig {
        llm,
        conversation_store,
        callback,
        system_prompt,
    }: &AgentConfig<CB>,
    action: Action,
) {
    #[allow(clippy::never_loop)]
    loop {
        match &action {
            Action::StartNewThread => {
                let id = ConversationId::generate();
                conversation_store.borrow_mut().write(
                    id,
                    &[Message {
                        role: MessageRole::System,
                        content: system_prompt.clone(),
                    }],
                );
                callback(Response::ThreadCreated { id });
                break;
            }
            Action::SendMessage { id, message } => {
                // Persist and acknowledge the message
                let messages = {
                    let mut messages = conversation_store.borrow().read(*id).unwrap();
                    messages.push(Message {
                        role: MessageRole::User,
                        content: message.to_string(),
                    });
                    conversation_store.borrow_mut().write(*id, &messages);
                    callback(Response::MessageRecieved { id: *id });
                    messages
                };

                let response = llm.perform(LLMParams::Message { messages });

                match response {
                    LLMResponse::Message { message } => {
                        let mut messages = conversation_store.borrow().read(*id).unwrap();
                        messages.push(Message {
                            role: MessageRole::Assistant,
                            content: message,
                        });
                        conversation_store.borrow_mut().write(*id, &messages);
                        callback(Response::ReplyReceived { id: *id });
                        break;
                    }
                }
            }
        }
    }
}
