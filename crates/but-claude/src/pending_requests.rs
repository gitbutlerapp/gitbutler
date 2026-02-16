//! In-memory storage for pending permission requests and user questions.
//!
//! This module provides ephemeral storage for requests that are waiting for user responses.
//! These requests don't need to survive restarts since they are tied to active Claude sessions.
//!
//! # Mutex Poisoning
//!
//! This module uses `Mutex::lock().unwrap()` rather than error handling because:
//! - A poisoned mutex indicates a thread panicked while holding the lock
//! - The operations inside the lock are simple HashMap operations that shouldn't panic
//! - If a panic did occur, the pending requests state is likely corrupted
//! - Continuing with corrupted state could cause user responses to be lost or misrouted
//! - Failing fast is preferable to silent data corruption in this context

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{ClaudeAskUserQuestionRequest, ClaudePermissionRequest, PermissionDecision};

/// Default timeout for pending requests (24 hours).
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Response to a permission request, including the decision and wildcard preference.
pub type PermissionResponse = (PermissionDecision, bool);

/// A pending permission request with its response channel.
struct PendingPermission {
    pub request: ClaudePermissionRequest,
    pub sender: oneshot::Sender<PermissionResponse>,
    pub created_at: Instant,
    pub session_id: Uuid,
}

/// A pending user question request with its response channel.
struct PendingQuestion {
    pub request: ClaudeAskUserQuestionRequest,
    pub sender: oneshot::Sender<HashMap<String, String>>,
    pub created_at: Instant,
    pub session_id: Uuid,
}

/// In-memory storage for pending requests awaiting user responses.
///
/// This replaces the database-based polling mechanism for ephemeral request/response
/// patterns. Requests are stored with oneshot channels that allow the waiting async
/// task to be notified immediately when a response is received.
#[derive(Default)]
pub struct PendingRequests {
    permissions: Mutex<HashMap<String, PendingPermission>>,
    questions: Mutex<HashMap<String, PendingQuestion>>,
}

impl PendingRequests {
    /// Creates a new empty pending requests store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a pending permission request and returns a receiver for the response.
    ///
    /// The returned receiver will resolve when `respond_permission` is called with
    /// the same request ID, or will return an error if the sender is dropped
    /// (e.g., on session cancellation).
    pub fn insert_permission(
        &self,
        request: ClaudePermissionRequest,
        session_id: Uuid,
    ) -> oneshot::Receiver<PermissionResponse> {
        let (sender, receiver) = oneshot::channel();
        let id = request.id.clone();

        let pending = PendingPermission {
            request,
            sender,
            created_at: Instant::now(),
            session_id,
        };

        let mut permissions = self.permissions.lock().unwrap();
        permissions.insert(id, pending);

        receiver
    }

    /// Gets a pending permission request by ID without removing it.
    pub fn get_permission(&self, id: &str) -> Option<ClaudePermissionRequest> {
        let permissions = self.permissions.lock().unwrap();
        permissions.get(id).map(|p| p.request.clone())
    }

    /// Lists all pending permission requests.
    pub fn list_permissions(&self) -> Vec<ClaudePermissionRequest> {
        let permissions = self.permissions.lock().unwrap();
        permissions.values().map(|p| p.request.clone()).collect()
    }

    /// Responds to a pending permission request.
    ///
    /// This removes the request from storage and sends the decision to the waiting task.
    /// Returns `Ok(())` if the request was found and responded to, or an error if not found.
    pub fn respond_permission(&self, id: &str, decision: PermissionDecision, use_wildcard: bool) -> anyhow::Result<()> {
        let mut permissions = self.permissions.lock().unwrap();
        let pending = permissions
            .remove(id)
            .ok_or_else(|| anyhow::anyhow!("Permission request not found: {id}"))?;

        // Send the decision - ignore error if receiver was dropped (e.g., timeout)
        let _ = pending.sender.send((decision, use_wildcard));
        Ok(())
    }

    /// Removes a pending permission request without responding.
    ///
    /// This is useful for cleanup after timeout.
    pub fn remove_permission(&self, id: &str) -> Option<ClaudePermissionRequest> {
        let mut permissions = self.permissions.lock().unwrap();
        permissions.remove(id).map(|p| p.request)
    }

    /// Inserts a pending user question request and returns a receiver for the response.
    pub fn insert_question(
        &self,
        request: ClaudeAskUserQuestionRequest,
        session_id: Uuid,
    ) -> oneshot::Receiver<HashMap<String, String>> {
        let (sender, receiver) = oneshot::channel();
        let id = request.id.clone();

        let pending = PendingQuestion {
            request,
            sender,
            created_at: Instant::now(),
            session_id,
        };

        let mut questions = self.questions.lock().unwrap();
        questions.insert(id, pending);

        receiver
    }

    /// Gets a pending question request by ID without removing it.
    pub fn get_question(&self, id: &str) -> Option<ClaudeAskUserQuestionRequest> {
        let questions = self.questions.lock().unwrap();
        questions.get(id).map(|p| p.request.clone())
    }

    /// Gets a pending question request by stack ID.
    pub fn get_question_by_stack(&self, stack_id: &gitbutler_stack::StackId) -> Option<ClaudeAskUserQuestionRequest> {
        let questions = self.questions.lock().unwrap();
        questions
            .values()
            .find(|p| p.request.stack_id.as_ref() == Some(stack_id))
            .map(|p| p.request.clone())
    }

    /// Lists all pending question requests.
    pub fn list_questions(&self) -> Vec<ClaudeAskUserQuestionRequest> {
        let questions = self.questions.lock().unwrap();
        questions.values().map(|p| p.request.clone()).collect()
    }

    /// Responds to a pending question request with user answers.
    ///
    /// This removes the request from storage and sends the answers to the waiting task.
    pub fn respond_question(&self, id: &str, answers: HashMap<String, String>) -> anyhow::Result<()> {
        let mut questions = self.questions.lock().unwrap();
        let pending = questions
            .remove(id)
            .ok_or_else(|| anyhow::anyhow!("Question request not found: {id}"))?;

        let _ = pending.sender.send(answers);
        Ok(())
    }

    /// Removes a pending question request without responding.
    pub fn remove_question(&self, id: &str) -> Option<ClaudeAskUserQuestionRequest> {
        let mut questions = self.questions.lock().unwrap();
        questions.remove(id).map(|p| p.request)
    }

    /// Cancels all pending requests for a given session.
    ///
    /// This drops all senders for the session, causing the receivers to return errors.
    /// Returns the number of requests cancelled.
    pub fn cancel_session(&self, session_id: Uuid) -> usize {
        let mut count = 0;

        {
            let mut permissions = self.permissions.lock().unwrap();
            let ids_to_remove: Vec<_> = permissions
                .iter()
                .filter(|(_, p)| p.session_id == session_id)
                .map(|(id, _)| id.clone())
                .collect();

            for id in ids_to_remove {
                permissions.remove(&id);
                count += 1;
            }
        }

        {
            let mut questions = self.questions.lock().unwrap();
            let ids_to_remove: Vec<_> = questions
                .iter()
                .filter(|(_, p)| p.session_id == session_id)
                .map(|(id, _)| id.clone())
                .collect();

            for id in ids_to_remove {
                questions.remove(&id);
                count += 1;
            }
        }

        count
    }

    /// Removes all requests that have exceeded the given timeout.
    ///
    /// Returns the number of requests removed.
    pub fn cleanup_expired(&self, timeout: Duration) -> usize {
        let now = Instant::now();
        let mut count = 0;

        {
            let mut permissions = self.permissions.lock().unwrap();
            let ids_to_remove: Vec<_> = permissions
                .iter()
                .filter(|(_, p)| now.duration_since(p.created_at) > timeout)
                .map(|(id, _)| id.clone())
                .collect();

            for id in ids_to_remove {
                permissions.remove(&id);
                count += 1;
            }
        }

        {
            let mut questions = self.questions.lock().unwrap();
            let ids_to_remove: Vec<_> = questions
                .iter()
                .filter(|(_, p)| now.duration_since(p.created_at) > timeout)
                .map(|(id, _)| id.clone())
                .collect();

            for id in ids_to_remove {
                questions.remove(&id);
                count += 1;
            }
        }

        count
    }
}

/// Global instance of pending requests.
///
/// This is used to share state between the Claude callbacks and the API endpoints.
static PENDING_REQUESTS: std::sync::OnceLock<PendingRequests> = std::sync::OnceLock::new();

/// Gets the global pending requests instance.
pub fn pending_requests() -> &'static PendingRequests {
    PENDING_REQUESTS.get_or_init(PendingRequests::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_permission_request_response() {
        let store = PendingRequests::new();
        let session_id = Uuid::new_v4();

        let request = ClaudePermissionRequest {
            id: "test-1".to_string(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            tool_name: "Write".to_string(),
            input: serde_json::json!({"file_path": "/test.txt"}),
            decision: None,
            use_wildcard: false,
        };

        let receiver = store.insert_permission(request, session_id);

        // Respond in another task
        store
            .respond_permission("test-1", PermissionDecision::AllowOnce, false)
            .unwrap();

        // Receiver should get the response
        let (decision, use_wildcard) = receiver.await.unwrap();
        assert_eq!(decision, PermissionDecision::AllowOnce);
        assert!(!use_wildcard);
    }

    #[tokio::test]
    async fn test_question_request_response() {
        let store = PendingRequests::new();
        let session_id = Uuid::new_v4();

        let request = ClaudeAskUserQuestionRequest {
            id: "test-q-1".to_string(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            questions: vec![],
            answers: None,
            stack_id: None,
        };

        let receiver = store.insert_question(request, session_id);

        let mut answers = HashMap::new();
        answers.insert("q1".to_string(), "answer1".to_string());

        store.respond_question("test-q-1", answers.clone()).unwrap();

        let received = receiver.await.unwrap();
        assert_eq!(received, answers);
    }

    #[tokio::test]
    async fn test_session_cancellation() {
        let store = PendingRequests::new();
        let session_id = Uuid::new_v4();

        let request = ClaudePermissionRequest {
            id: "test-cancel".to_string(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            tool_name: "Write".to_string(),
            input: serde_json::json!({}),
            decision: None,
            use_wildcard: false,
        };

        let receiver = store.insert_permission(request, session_id);

        // Cancel the session
        let cancelled = store.cancel_session(session_id);
        assert_eq!(cancelled, 1);

        // Receiver should error because sender was dropped
        assert!(receiver.await.is_err());
    }
}
