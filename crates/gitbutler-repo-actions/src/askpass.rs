//! This is the global askpass broker. Its purpose is to ferry prompts and responses for
//! credentials from Git (or more accurately, from SSH) to our app.
//!
//! On application startup, the broker must be explicitly initialized with [init] or disabled with
//! [disable]. This allows us to use the state of the broker as a proxy for whether we should
//! divert SSH_ASKPASS to our custom askpass machinery, or if we should simply let the current
//! configuration decide. If we were to default to either initialized or disabled, utilizing the
//! state to decide whether or not to use the broker could lead to subtle bugs.
//!
//! The GUI utilizes the broker to be able to prompt the user in-app, and as such the broker should
//! always be enabled when running the GUI. The CLI has no use of this mechanism at present as
//! there are well-defined ways to handle prompting in the terminal. It may however make sense to
//! incorporate this broker for a TUI, however.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use gitbutler_stack::StackId;
use serde::Serialize;
use tokio::sync::{Mutex, oneshot};

static GLOBAL_ASKPASS_BROKER: OnceLock<Option<AskpassBroker>> = OnceLock::new();

/// Initialize the global askpass broker.
///
/// # Panics
/// This function should be called **exactly once** during startup if the custom askpass broker
/// needs to be used (currently only needed for GUI functionality). Otherwise, call [disable] at
/// startup instead.
pub fn init(submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static) {
    GLOBAL_ASKPASS_BROKER
        .set(Some(AskpassBroker::init(submit_prompt)))
        .unwrap_or_else(|_| panic!("broker already configured"))
}

/// Explicitly disable the global askpass broker.
///
/// # Panics
/// This function should be called **exactly once** during startup if the custom askpass broker
/// should **not** be used (currently the sensible approach for CLI). Otherwise, call [init] at
/// startup instead.
pub fn disable() {
    GLOBAL_ASKPASS_BROKER
        .set(None)
        .unwrap_or_else(|_| panic!("broker already configured"))
}

/// Get the global askpass broker, assuming it's configured.
///
/// Panics if neither [init] nor [disable] has been called. This is an important property as we use
/// the state of the broker to determine whether to use our askpass overrides or not. If it's not
/// explicitly set, there is no way to tell the intent and bugs may hide in unexpected places as a
/// consequence. For example, if not initialized for the GUI, the prompt may show up in the
/// terminal that started the GUI.
pub fn get_broker() -> Option<AskpassBroker> {
    match GLOBAL_ASKPASS_BROKER.get() {
        Some(broker_state) => broker_state.to_owned(),
        None => panic!("broker has not been configured"),
    }
}

pub struct AskpassRequest {
    sender: oneshot::Sender<Option<String>>,
}

/// An ID for the askpass request.
pub type AskpassRequestId = but_core::Id<'A'>;

#[derive(Debug, Clone, serde::Serialize)]
// This is needed to end up with a struct with either `branch_id` or `action`
#[serde(untagged)]
pub enum Context {
    Push { branch_id: Option<StackId> },
    Fetch { action: String },
    SignedCommit { branch_id: Option<StackId> },
    Clone { url: String },
}

#[derive(Clone)]
pub struct AskpassBroker {
    pending_requests: Arc<Mutex<HashMap<AskpassRequestId, AskpassRequest>>>,
    submit_prompt_event: Arc<dyn Fn(PromptEvent<Context>) + Send + Sync>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptEvent<C: Serialize + Clone> {
    id: AskpassRequestId,
    prompt: String,
    context: C,
}

impl AskpassBroker {
    pub fn init(submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static) -> Self {
        Self {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            submit_prompt_event: Arc::new(submit_prompt),
        }
    }

    pub async fn submit_prompt(&self, prompt: String, context: Context) -> Option<String> {
        let (sender, receiver) = oneshot::channel();
        let id = AskpassRequestId::generate();
        let request = AskpassRequest { sender };
        self.pending_requests.lock().await.insert(id, request);
        (self.submit_prompt_event)(PromptEvent {
            id,
            prompt,
            context,
        });
        receiver.await.unwrap()
    }

    pub async fn handle_response(&self, id: AskpassRequestId, response: Option<String>) {
        let mut pending_requests = self.pending_requests.lock().await;
        if let Some(request) = pending_requests.remove(&id) {
            let _ = request.sender.send(response);
        } else {
            log::warn!("received response for unknown askpass request: {id}");
        }
    }
}
