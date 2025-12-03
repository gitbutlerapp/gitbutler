use std::{collections::HashMap, sync::Arc};

use gitbutler_stack::StackId;
use serde::Serialize;
use tokio::sync::{Mutex, oneshot};

static mut GLOBAL_ASKPASS_BROKER: Option<AskpassBroker> = None;

/// Initialize the global askpass broker.
///
/// # Safety
/// This function **must** be called **at least once**, from only one thread at a time,
/// before any other function from this module is called. **Calls to [`get_broker`] before [`init`] will panic.**
///
/// This function is **NOT** thread safe.
#[expect(static_mut_refs)]
pub unsafe fn init(submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static) {
    unsafe {
        GLOBAL_ASKPASS_BROKER.replace(AskpassBroker::init(submit_prompt));
    }
}

/// Get the global askpass broker.
///
/// # Panics
/// Will panic if [`init`] was not called before this function.
#[expect(static_mut_refs)]
pub fn get_broker() -> &'static AskpassBroker {
    unsafe {
        GLOBAL_ASKPASS_BROKER
            .as_ref()
            .expect("askpass broker not initialized")
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
