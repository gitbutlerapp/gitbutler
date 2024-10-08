use std::{collections::HashMap, path::Path, sync::Arc};

use gitbutler_id::id::Id;
use gitbutler_stack::StackId;
use serde::Serialize;
use tokio::sync::{oneshot, Mutex};

static mut GLOBAL_ASKPASS_BROKER: Option<AskpassBroker> = None;

/// Initialize the global askpass broker.
///
/// # Safety
/// This function **must** be called **at least once**, from only one thread at a time,
/// before any other function from this module is called. **Calls to [`get_broker`] before [`init`] will panic.**
///
/// This function is **NOT** thread safe.
pub unsafe fn init(submit_prompt: impl Fn(PromptEvent<Context>) + Send + Sync + 'static) {
    GLOBAL_ASKPASS_BROKER.replace(AskpassBroker::init(submit_prompt));
}

/// Get the global askpass broker.
///
/// # Panics
/// Will panic if [`init`] was not called before this function.
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

#[derive(Debug, Clone, serde::Serialize)]
// This is needed to end up with a struct with either `branch_id` or `action`
#[serde(untagged)]
pub enum Context {
    Push { branch_id: Option<StackId> },
    Fetch { action: String },
    SignedCommit { branch_id: Option<StackId> },
}

#[derive(Clone)]
pub struct AskpassBroker {
    pending_requests: Arc<Mutex<HashMap<Id<AskpassRequest>, AskpassRequest>>>,
    submit_prompt_event: Arc<dyn Fn(PromptEvent<Context>) + Send + Sync>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptEvent<C: Serialize + Clone> {
    id: Id<AskpassRequest>,
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
        let id = Id::generate();
        let request = AskpassRequest { sender };
        self.pending_requests.lock().await.insert(id, request);
        (self.submit_prompt_event)(PromptEvent {
            id,
            prompt,
            context,
        });
        receiver.await.unwrap()
    }

    pub async fn handle_response(&self, id: Id<AskpassRequest>, response: Option<String>) {
        let mut pending_requests = self.pending_requests.lock().await;
        if let Some(request) = pending_requests.remove(&id) {
            let _ = request.sender.send(response);
        } else {
            log::warn!("received response for unknown askpass request: {}", id);
        }
    }
}

async fn handle_git_prompt_commit_sign_sync(
    prompt: String,
    branch_id: Option<StackId>,
) -> Option<String> {
    tracing::info!("received prompt for synchronous signed commit {branch_id:?}: {prompt:?}");
    get_broker()
        .submit_prompt(prompt, Context::SignedCommit { branch_id })
        .await
}

/// Utility to synchronously sign a commit.
/// Uses the Tokio runner to run the async function,
/// and the global askpass broker to handle any prompts.
pub fn sign_commit_sync(
    repo_path: impl AsRef<Path>,
    base_commitish: impl AsRef<str>,
    branch_id: Option<StackId>,
) -> Result<String, impl std::error::Error> {
    let repo_path = repo_path.as_ref().to_path_buf();
    let base_commitish: &str = base_commitish.as_ref();
    let base_commitish = base_commitish.to_string();

    // Run as sync
    let handle = std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(gitbutler_git::sign_commit(
                &repo_path,
                gitbutler_git::tokio::TokioExecutor,
                base_commitish,
                handle_git_prompt_commit_sign_sync,
                branch_id,
            ))
    });

    tokio::task::block_in_place(|| handle.join().unwrap())
}
