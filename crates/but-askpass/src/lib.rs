//! This is the global askpass broker. Its purpose is to ferry prompts and responses for
//! credentials from Git (or more accurately, from SSH) to our app.
//!
//! On application startup, the broker must be explicitly initialized with [`init`] or disabled with
//! [`disable`]. This allows us to use the state of the broker as a proxy for whether we should
//! divert `SSH_ASKPASS` to our custom askpass machinery, or if we should simply let the current
//! configuration decide. If we were to default to either initialized or disabled, utilizing the
//! state to decide whether or not to use the broker could lead to subtle bugs.
//!
//! The GUI utilizes the broker to be able to prompt the user in-app, and as such the broker should
//! always be enabled when running the GUI. The CLI has no use of this mechanism at present as
//! there are well-defined ways to handle prompting in the terminal. It may however make sense to
//! incorporate this broker for a TUI, however.

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    sync::{Arc, OnceLock},
};

use but_core::ref_metadata::StackId;
use serde::Serialize;
use tokio::sync::{Mutex, oneshot};

static GLOBAL_ASKPASS_BROKER: OnceLock<Option<AskpassBroker>> = OnceLock::new();

/// Initialize the global askpass broker.
///
/// # Panics
/// This function should be called **exactly once** during startup if the custom askpass broker
/// needs to be used (currently only needed for GUI functionality). Otherwise, call [`disable`] at
/// startup instead.
pub fn init(submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static) {
    try_init(submit_prompt).unwrap_or_else(|_| panic!("broker already configured"));
}

/// The askpass broker has already been explicitly initialized or disabled.
#[derive(Debug, Clone, Copy)]
pub struct BrokerAlreadyConfigured;

impl fmt::Display for BrokerAlreadyConfigured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("broker already configured")
    }
}

impl Error for BrokerAlreadyConfigured {}

/// Fallibly initialize the global askpass broker.
///
/// This is useful for runtime bindings that need to report startup errors to their host instead of
/// panicking across an FFI boundary.
pub fn try_init(
    submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static,
) -> Result<(), BrokerAlreadyConfigured> {
    GLOBAL_ASKPASS_BROKER
        .set(Some(AskpassBroker::init(submit_prompt)))
        .map_err(|_| BrokerAlreadyConfigured)
}

/// Explicitly disable the global askpass broker.
///
/// # Panics
/// This function should be called **exactly once** during startup if the custom askpass broker
/// should **not** be used (currently the sensible approach for CLI). Otherwise, call [`init`] at
/// startup instead.
pub fn disable() {
    GLOBAL_ASKPASS_BROKER
        .set(None)
        .unwrap_or_else(|_| panic!("broker already configured"))
}

/// Get the global askpass broker, assuming it's configured.
///
/// # Panics
/// Panics if neither [`init`] nor [`disable`] has been called. This is an important property as we
/// use the state of the broker to determine whether to use our askpass overrides or not. If it's
/// not explicitly set, there is no way to tell the intent and bugs may hide in unexpected places
/// as a consequence. For example, if not initialized for the GUI, the prompt may show up in the
/// terminal that started the GUI.
pub fn get_broker() -> Option<AskpassBroker> {
    try_get_broker().unwrap_or_else(|| panic!("broker has not been configured"))
}

/// Fallibly get the global askpass broker state.
///
/// Returns `None` if neither [`init`], [`try_init`], nor [`disable`] has configured the broker.
pub fn try_get_broker() -> Option<Option<AskpassBroker>> {
    GLOBAL_ASKPASS_BROKER.get().cloned()
}

struct AskpassRequest {
    sender: oneshot::Sender<Option<String>>,
}

/// An ID for an askpass request.
pub type AskpassRequestId = but_core::Id<'A'>;

/// Additional context sent alongside a credential prompt.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Context {
    /// A prompt encountered while pushing a stack.
    Push {
        /// The stack being pushed, if one is associated with the prompt.
        branch_id: Option<StackId>,
    },
    /// A prompt encountered while fetching.
    Fetch {
        /// The user-visible action associated with the fetch.
        action: String,
    },
    /// A prompt encountered while signing a commit.
    SignedCommit {
        /// The stack being signed, if one is associated with the prompt.
        branch_id: Option<StackId>,
    },
    /// A prompt encountered while cloning.
    Clone {
        /// The URL being cloned.
        url: String,
    },
}

/// A process-global broker for pending askpass requests.
#[derive(Clone)]
pub struct AskpassBroker {
    pending_requests: Arc<Mutex<HashMap<AskpassRequestId, AskpassRequest>>>,
    submit_prompt_event: Arc<dyn Fn(PromptEvent<Context>) + Send + Sync>,
}

/// A prompt emitted to the application so it can provide a response.
#[derive(Debug, Clone, Serialize)]
pub struct PromptEvent<C: Serialize + Clone> {
    pub id: AskpassRequestId,
    pub prompt: String,
    pub context: C,
}

impl AskpassBroker {
    /// Create a new broker with the given event sink.
    pub fn init(submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static) -> Self {
        Self {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            submit_prompt_event: Arc::new(submit_prompt),
        }
    }

    /// Submit a prompt and wait for the application response.
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

    /// Fulfill a previously submitted prompt.
    pub async fn handle_response(&self, id: AskpassRequestId, response: Option<String>) {
        let mut pending_requests = self.pending_requests.lock().await;
        if let Some(request) = pending_requests.remove(&id) {
            request.sender.send(response).ok();
        } else {
            tracing::warn!("received response for unknown askpass request: {id}");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn try_init_reports_duplicate_configuration() {
        super::try_init(|_| {}).expect("first broker init should succeed");

        let err = super::try_init(|_| {}).expect_err("second broker init should fail");
        assert_eq!(
            err.to_string(),
            "broker already configured",
            "duplicate init should be reported without panicking"
        );
    }
}
