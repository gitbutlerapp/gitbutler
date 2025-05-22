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
    return Some(gitbutler_secret::Sensitive("this is secret".into()));
    // gitbutler_secret::secret::retrieve(
    //     "gitbutler-agent-token",
    //     gitbutler_secret::secret::Namespace::Global,
    // )
    // .unwrap()
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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Message {
    role: MessageRole,
    content: String,
}

pub enum Action {
    Exit,
    DoAIStuff,
    /// Starts a new thread that the user can send messages to.
    StartNewThread,
}

pub enum Response {
    ThreadCreated { id: ConversationId },
}

pub struct Agent {
    thread: std::thread::JoinHandle<()>,
    actions_tx: std::sync::mpsc::Sender<Action>,
}

pub trait LLM {
    fn perform(&self);
}

pub struct OpenRouter {
    token: gitbutler_secret::Sensitive<String>,
}

impl LLM for OpenRouter {
    fn perform(&self) {
        println!("Did AI stuff :D");
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
enum ConversationStoreReadError {
    NotFound,
    FailedToRead,
}

pub trait ConversationStore {
    fn read(&self, id: ConversationId) -> Result<Vec<Message>, ConversationStoreReadError>;
    fn write(&mut self, id: ConversationId, messages: Vec<Message>);
}

struct InMemoryConversationStore {
    map: HashMap<ConversationId, Vec<Message>>,
}

// Construction
impl InMemoryConversationStore {
    pub fn new() -> Self {
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

    fn write(&mut self, id: ConversationId, messages: Vec<Message>) {
        self.map.insert(id, messages);
    }
}

impl Agent {
    pub fn start<F>(
        llm: Box<dyn LLM + Sync + Send>,
        conversation_store: std::sync::Arc<std::sync::Mutex<dyn ConversationStore + Sync + Send>>,
        callback: F,
    ) -> Agent
    where
        F: Fn(Response) + Send + 'static,
    {
        let (actions_tx, actions_rx) = std::sync::mpsc::channel::<Action>();

        let thread = std::thread::spawn(move || {
            'thread_loop: loop {
                let action = actions_rx.recv().unwrap();

                match action {
                    Action::StartNewThread => {
                        let id = ConversationId::generate();
                        {
                            let mut conversation_store = conversation_store.lock().unwrap();
                            conversation_store.write(
                                id,
                                vec![Message {
                                    role: MessageRole::System,
                                    content: "You are a helpful agent".into(),
                                }],
                            );
                        }
                        callback(Response::ThreadCreated { id });
                    }
                    Action::DoAIStuff => {
                        llm.perform();
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

    /// Basic debug thingy to exiserice basic behaviours :D
    #[test]
    fn test_start() {
        let callback = |response| {
            match response {
                Response::ThreadCreated { id } => println!("Created conversation: {:?}", id),
            };
        };

        let llm = OpenRouter {
            token: get_token().unwrap(),
        };

        let converation_store =
            std::sync::Arc::new(std::sync::Mutex::new(InMemoryConversationStore::new()));

        let agent = Agent::start(Box::new(llm), converation_store, callback);
        agent.perform_action(Action::StartNewThread);
        agent.stop()
    }

    #[test]
    fn test_read_conversation_store_after_event() {
        let (tx, rx) = std::sync::mpsc::channel::<Response>();

        let callback = move |response| {
            tx.send(response).unwrap();
        };

        let llm = OpenRouter {
            token: get_token().unwrap(),
        };

        let conversation_store =
            std::sync::Arc::new(std::sync::Mutex::new(InMemoryConversationStore::new()));

        let agent = Agent::start(Box::new(llm), conversation_store.clone(), callback);

        let handle = std::thread::spawn(move || {
            agent.perform_action(Action::StartNewThread);
            loop {
                let message = rx.recv().unwrap();

                match message {
                    Response::ThreadCreated { id } => {
                        {
                            let conversation_store = conversation_store.lock().unwrap();
                            println!("{:?}", conversation_store.read(id).unwrap());
                        }
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
