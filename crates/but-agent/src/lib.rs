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

use std::{collections::HashMap, ops::Deref};

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
    Exit,
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

pub struct Agent {
    thread: std::thread::JoinHandle<()>,
    actions_tx: std::sync::mpsc::Sender<Action>,
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

struct MockLLM<CB: Fn(String) -> String> {
    callback: CB,
}

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

impl Deref for ConversationId {
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

struct InMemoryConversationStore {
    map: HashMap<ConversationId, Vec<Message>>,
}

// Construction
impl InMemoryConversationStore {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

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
    pub llm: Box<dyn LLM + Sync + Send>,
    pub conversation_store: std::sync::Arc<std::sync::Mutex<dyn ConversationStore + Sync + Send>>,
    pub callback: CB,
    pub system_prompt: String,
}

impl Agent {
    pub fn start<CB: Fn(Response) + Send + 'static>(
        AgentConfig {
            llm,
            conversation_store,
            callback,
            system_prompt,
        }: AgentConfig<CB>,
    ) -> Agent {
        let (actions_tx, actions_rx) = std::sync::mpsc::channel::<Action>();

        let thread = std::thread::spawn(move || {
            'thread_loop: loop {
                let action = actions_rx.recv().unwrap();

                match action {
                    Action::StartNewThread => {
                        let id = ConversationId::generate();
                        let mut conversation_store = conversation_store.lock().unwrap();
                        conversation_store.write(
                            id,
                            &[Message {
                                role: MessageRole::System,
                                content: system_prompt.clone(),
                            }],
                        );
                        core::mem::drop(conversation_store);
                        callback(Response::ThreadCreated { id });
                    }
                    Action::SendMessage { id, message } => {
                        // Persist and acknowledge the message
                        let messages = {
                            let mut conversation_store = conversation_store.lock().unwrap();
                            let mut messages = conversation_store.read(id).unwrap();
                            messages.push(Message {
                                role: MessageRole::User,
                                content: message,
                            });
                            conversation_store.write(id, &messages);
                            core::mem::drop(conversation_store);
                            callback(Response::MessageRecieved { id });
                            messages
                        };

                        let response = llm.perform(LLMParams::Message { messages });

                        match response {
                            LLMResponse::Message { message } => {
                                let mut conversation_store = conversation_store.lock().unwrap();
                                let mut messages = conversation_store.read(id).unwrap();
                                messages.push(Message {
                                    role: MessageRole::Assistant,
                                    content: message,
                                });
                                conversation_store.write(id, &messages);
                                core::mem::drop(conversation_store);
                                callback(Response::ReplyReceived { id });
                            }
                        }
                    }
                    Action::Exit => break 'thread_loop,
                }
            }
        });

        Self { thread, actions_tx }
    }

    pub fn perform_action(&self, action: Action) {
        self.actions_tx.send(action).unwrap();
    }

    pub fn stop(self) {
        // It is VERY important that we ask the thread to exit, otherwise it will just hang forever.
        self.perform_action(Action::Exit);
        self.thread.join().unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn system_prompt() -> String {
        "You are a great agent :flower:".into()
    }

    #[test]
    fn playgroud() {
        let (tx, rx) = std::sync::mpsc::channel::<Response>();

        let callback = move |response| {
            tx.send(response).unwrap();
        };

        let llm = OpenRouter {
            token: get_token().unwrap(),
            model: "qwen/qwen3-32b".into(),
            provider: "Cerebras".into(),
        };

        let conversation_store =
            std::sync::Arc::new(std::sync::Mutex::new(InMemoryConversationStore::new()));

        let start = std::time::SystemTime::now();

        let agent = Agent::start(AgentConfig {
            llm: Box::new(llm),
            conversation_store: conversation_store.clone(),
            callback,
            system_prompt: system_prompt(),
        });

        let handle = std::thread::spawn(move || {
            agent.perform_action(Action::StartNewThread);
            loop {
                let message = rx.recv().unwrap();

                match message {
                    Response::ThreadCreated { id } => {
                        agent.perform_action(Action::SendMessage {
                            id,
                            message: "Generate a 1000 word poem about rust programming.".into(),
                        });
                    }
                    Response::ReplyReceived { id } => {
                        let conversation_store = conversation_store.lock().unwrap();
                        let conversation = conversation_store.read(id).unwrap();

                        println!("{}", conversation.last().unwrap().content);

                        agent.stop();
                        break;
                    }
                    _ => {}
                };
            }
        });

        handle.join().unwrap();

        let end = std::time::SystemTime::now();
        println!("Took: {}", end.duration_since(start).unwrap().as_millis());
    }

    /// Basic debug thingy to exiserice basic behaviours :D
    #[test]
    fn test_start() {
        let callback = |response| {
            if let Response::ThreadCreated { id } = response {
                println!("Created conversation: {:?}", id)
            };
        };

        let llm = MockLLM {
            callback: |string| string,
        };

        let conversation_store =
            std::sync::Arc::new(std::sync::Mutex::new(InMemoryConversationStore::new()));

        let agent = Agent::start(AgentConfig {
            llm: Box::new(llm),
            conversation_store,
            callback,
            system_prompt: system_prompt(),
        });
        agent.perform_action(Action::StartNewThread);
        agent.stop()
    }

    #[test]
    fn test_read_conversation_store_after_event() {
        let (tx, rx) = std::sync::mpsc::channel::<Response>();

        let callback = move |response| {
            tx.send(response).unwrap();
        };

        let llm = MockLLM {
            callback: |string| string,
        };

        let conversation_store =
            std::sync::Arc::new(std::sync::Mutex::new(InMemoryConversationStore::new()));

        let agent = Agent::start(AgentConfig {
            llm: Box::new(llm),
            conversation_store: conversation_store.clone(),
            callback,
            system_prompt: system_prompt(),
        });

        let handle = std::thread::spawn(move || {
            agent.perform_action(Action::StartNewThread);
            loop {
                let message = rx.recv().unwrap();

                #[allow(irrefutable_let_patterns)]
                if let Response::ThreadCreated { id } = message {
                    {
                        let conversation_store = conversation_store.lock().unwrap();
                        let conversation = conversation_store.read(id).unwrap();
                        assert_eq!(
                            conversation,
                            vec![Message {
                                role: MessageRole::System,
                                content: system_prompt()
                            }]
                        )
                    }
                    agent.stop();
                    break;
                };
            }
        });
        handle.join().unwrap();
    }

    #[test]
    fn test_llm_message_response() {
        let (tx, rx) = std::sync::mpsc::channel::<Response>();

        let callback = move |response| {
            tx.send(response).unwrap();
        };

        let llm = MockLLM {
            callback: |string| format!("response: {}", string),
        };

        let conversation_store =
            std::sync::Arc::new(std::sync::Mutex::new(InMemoryConversationStore::new()));

        let agent = Agent::start(AgentConfig {
            llm: Box::new(llm),
            conversation_store: conversation_store.clone(),
            callback,
            system_prompt: system_prompt(),
        });

        let handle = std::thread::spawn(move || {
            agent.perform_action(Action::StartNewThread);
            loop {
                let message = rx.recv().unwrap();

                match message {
                    Response::ThreadCreated { id } => {
                        agent.perform_action(Action::SendMessage {
                            id,
                            message: "Hello world!".into(),
                        });
                    }
                    Response::ReplyReceived { id } => {
                        let conversation_store = conversation_store.lock().unwrap();
                        let conversation = conversation_store.read(id).unwrap();
                        assert_eq!(
                            conversation,
                            vec![
                                Message {
                                    role: MessageRole::System,
                                    content: system_prompt()
                                },
                                Message {
                                    role: MessageRole::User,
                                    content: "Hello world!".into()
                                },
                                Message {
                                    role: MessageRole::Assistant,
                                    content: "response: Hello world!".into()
                                }
                            ]
                        );

                        agent.stop();
                        break;
                    }
                    _ => {}
                };
            }
        });
        handle.join().unwrap();
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
