//! A butler agent
//!
//! 1 thread that lives for the lifetime of the app.
//!
//! It gets sent messages to process a new input, wether that is message from
//! the user, or responding to a tool call request.
//!
//! Input queue that the thread feeds from (likly a channel).
//! Some way of getting an output.
//!
//! Store message logs, because this might be used in a CLI.
use serde::{Deserialize, Serialize};

fn get_token() -> Option<gitbutler_secret::Sensitive<String>> {
    // return Some(gitbutler_secret::Sensitive("this is secret".into()));
    gitbutler_secret::secret::retrieve(
        "gitbutler-agent-token",
        gitbutler_secret::secret::Namespace::Global,
    )
    .unwrap()
}

fn set_token(token: Option<&str>) {
    if let Some(token) = token {
        gitbutler_secret::secret::persist(
            "gitbutler-agent-token",
            &gitbutler_secret::Sensitive(token.into()),
            gitbutler_secret::secret::Namespace::Global,
        )
        .unwrap();
    } else {
        gitbutler_secret::secret::delete(
            "gitbutler-agent-token",
            gitbutler_secret::secret::Namespace::Global,
        )
        .unwrap();
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Message {
    role: MessageRole,
    content: String,
}

pub enum Action {
    /// Starts a new thread that the user can send messages to.
    StartNewThread,
    SendMessage {
        id: ConversationId,
        message: String,
    },
}

pub enum Response {
    ThreadCreated {
        id: ConversationId,
    },
    /// Acknoledges that a user messages has been sent; Will be sent after the
    /// user message has been persisted to the conversation store
    MessageRecieved {
        id: ConversationId,
    },
    /// Sent whenver a reponse from an LLM has been recieved
    ReplyReceived {
        id: ConversationId,
    },
}

impl Response {
    pub fn id(&self) -> ConversationId {
        match self {
            Response::ThreadCreated { id } => *id,
            Response::MessageRecieved { id } => *id,
            Response::ReplyReceived { id } => *id,
        }
    }
}

pub enum LLMParams {
    Message { messages: Vec<Message> },
}

pub enum LLMResponse {
    Message { message: String },
}

pub trait LLM {
    fn perform(&self, params: LLMParams) -> LLMResponse;
}

#[derive(Serialize)]
struct OpenRouterProvider {
    only: Option<Vec<String>>,
}

#[derive(Serialize)]
struct OpenRouterAPIBody {
    model: String,
    messages: Vec<Message>,
    provider: Option<OpenRouterProvider>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: Message,
}

#[derive(Deserialize)]
struct OpenRouterAPIResponse {
    choices: Vec<OpenRouterChoice>,
}

pub struct OpenRouter {
    model: String,
    provider: String,
    token: gitbutler_secret::Sensitive<String>,
}

impl LLM for OpenRouter {
    fn perform(&self, params: LLMParams) -> LLMResponse {
        match params {
            LLMParams::Message { messages } => {
                let client = reqwest::blocking::Client::new();
                let result = client
                    .post("https://openrouter.ai/api/v1/chat/completions")
                    .bearer_auth(&self.token.0)
                    .header("Content-Type", "application/json")
                    .body(
                        serde_json::to_string(&OpenRouterAPIBody {
                            model: self.model.clone(),
                            messages,
                            provider: Some(OpenRouterProvider {
                                only: Some(vec![self.provider.clone()]),
                            }),
                        })
                        .unwrap(),
                    )
                    .send()
                    .unwrap();

                let reponse: OpenRouterAPIResponse = result.json().unwrap();

                LLMResponse::Message {
                    message: reponse.choices.first().unwrap().message.content.clone(),
                }
            }
        }
    }
}

#[cfg(test)]
struct MockLLM<CB: Fn(String) -> String> {
    callback: CB,
}

#[cfg(test)]
impl<CB: Fn(String) -> String> LLM for MockLLM<CB> {
    fn perform(&self, params: LLMParams) -> LLMResponse {
        match params {
            LLMParams::Message { messages } => {
                let last = messages.last().unwrap();
                LLMResponse::Message {
                    message: (self.callback)(last.content.clone()),
                }
            }
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct ConversationId(uuid::Uuid);

// Creating Conversation Ids;
impl ConversationId {
    fn generate() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::ops::Deref for ConversationId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub enum ConversationStoreReadError {
    NotFound,
    FailedToRead,
}

pub trait ConversationStore {
    fn read(&self, id: ConversationId) -> Result<Vec<Message>, ConversationStoreReadError>;
    fn write(&mut self, id: ConversationId, messages: &[Message]);
}

#[cfg(test)]
struct InMemoryConversationStore {
    map: std::collections::HashMap<ConversationId, Vec<Message>>,
}

// Construction
#[cfg(test)]
impl InMemoryConversationStore {
    fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
impl ConversationStore for InMemoryConversationStore {
    fn read(&self, id: ConversationId) -> Result<Vec<Message>, ConversationStoreReadError> {
        self.map
            .get(&id)
            .cloned()
            .ok_or(ConversationStoreReadError::NotFound)
    }

    fn write(&mut self, id: ConversationId, messages: &[Message]) {
        self.map.insert(id, messages.to_owned());
    }
}

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

#[cfg(test)]
mod test {
    use super::*;

    fn system_prompt() -> String {
        "You are a great agent :flower:".into()
    }

    #[test]
    fn send_message() {
        let llm = MockLLM {
            callback: |message| format!("Mocked response: {}", message),
        };
        let conversation_store =
            std::cell::RefCell::new(Box::new(InMemoryConversationStore::new()));

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let config = AgentConfig {
            llm: Box::new(llm),
            conversation_store,
            callback,
            system_prompt: system_prompt(),
        };

        agent_perform(&config, Action::StartNewThread);

        let id = responses.lock().unwrap().last().unwrap().id();

        agent_perform(
            &config,
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
            config.conversation_store.borrow().read(id).unwrap(),
            vec![
                Message {
                    role: MessageRole::System,
                    content: system_prompt(),
                },
                Message {
                    role: MessageRole::User,
                    content: "Hello!".into(),
                },
                Message {
                    role: MessageRole::Assistant,
                    content: "Mocked response: Hello!".into(),
                },
            ],
        );
    }

    #[test]
    fn create_thread() {
        let llm = MockLLM {
            callback: |message| format!("Mocked response: {}", message),
        };
        let conversation_store =
            std::cell::RefCell::new(Box::new(InMemoryConversationStore::new()));

        let responses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let moved_responses = responses.clone();
        let callback = move |response| {
            moved_responses.lock().unwrap().push(response);
        };

        let config = AgentConfig {
            llm: Box::new(llm),
            conversation_store,
            callback,
            system_prompt: system_prompt(),
        };

        agent_perform(&config, Action::StartNewThread);

        assert_eq!(responses.lock().unwrap().len(), 1);
        assert!(matches!(
            responses.lock().unwrap().last().unwrap(),
            Response::ThreadCreated { .. }
        ));

        let id = responses.lock().unwrap().last().unwrap().id();

        assert_eq!(
            config.conversation_store.borrow().read(id).unwrap(),
            vec![Message {
                role: MessageRole::System,
                content: system_prompt(),
            }],
        );
    }

    #[test]
    fn serialize_message_author() {
        assert_eq!(
            serde_json::to_string(&MessageRole::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::User).unwrap(),
            "\"user\""
        );
    }

    #[test]
    fn serialize_message() {
        assert_eq!(
            serde_json::to_string(&Message {
                role: MessageRole::Assistant,
                content: "Hello!".into()
            })
            .unwrap(),
            r#"{"role":"assistant","content":"Hello!"}"#
        );
    }
}
