use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::sync::{oneshot, Mutex};

use crate::id::Id;

pub struct AskpassRequest {
    sender: oneshot::Sender<Option<String>>,
}

#[derive(Clone)]
pub struct AskpassBroker {
    pending_requests: Arc<Mutex<HashMap<Id<AskpassRequest>, AskpassRequest>>>,
    handle: AppHandle,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PromptEvent<C: Serialize + Clone> {
    id: Id<AskpassRequest>,
    prompt: String,
    context: C,
}

impl AskpassBroker {
    pub fn init(handle: AppHandle) -> Self {
        Self {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            handle,
        }
    }

    pub async fn submit_prompt<C: Serialize + Clone>(
        &self,
        prompt: String,
        context: C,
    ) -> Option<String> {
        let (sender, receiver) = oneshot::channel();
        let id = Id::generate();
        let request = AskpassRequest { sender };
        self.pending_requests.lock().await.insert(id, request);
        self.handle
            .emit_all(
                "git_prompt",
                PromptEvent {
                    id,
                    prompt,
                    context,
                },
            )
            .expect("failed to emit askpass event");
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

pub mod commands {
    use super::{AppHandle, AskpassBroker, AskpassRequest, Id, Manager};
    #[tauri::command(async)]
    #[tracing::instrument(skip(handle))]
    pub async fn submit_prompt_response(
        handle: AppHandle,
        id: Id<AskpassRequest>,
        response: Option<String>,
    ) -> Result<(), ()> {
        handle
            .state::<AskpassBroker>()
            .handle_response(id, response)
            .await;
        Ok(())
    }
}
