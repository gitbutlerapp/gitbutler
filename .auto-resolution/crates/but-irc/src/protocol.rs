//! GitButler IRC protocol types and serialization.
//!
//! This module defines the protocol messages used for GitButler-specific
//! communication over IRC, including session management, prompts, responses,
//! and permissions.

use serde::{Deserialize, Serialize};

/// Calculate the channel name for a given project.
pub fn project_channel(repo_name: &str) -> String {
    format!("#{}", repo_name.replace('/', "-"))
}

/// Calculate the session channel name: `#<prefix>-<username>/<branch_name>`
pub fn session_channel(username: &str, branch_name: &str, prefix: &str) -> String {
    format!(
        "#{}-{}/{}",
        sanitize(prefix),
        sanitize(username),
        sanitize(branch_name)
    )
}

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '/')
        .collect()
}

/// GitButler protocol message wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum GitButlerMessage {
    /// Session started
    SessionStart(SessionStartMessage),
    /// Session ended
    SessionEnd(SessionEndMessage),
    /// Session status update
    SessionStatus(SessionStatusMessage),
    /// User prompt
    Prompt(PromptMessage),
    /// Claude response
    Response(ResponseMessage),
    /// Permission request
    PermissionRequest(PermissionRequestMessage),
    /// Permission decision
    PermissionDecision(PermissionDecisionMessage),
    /// Question from Claude
    Question(QuestionMessage),
    /// Answer to question
    Answer(AnswerMessage),
    /// Abort session
    Abort(AbortMessage),
    /// Generic text message wrapper
    Text(TextMessage),
}

impl GitButlerMessage {
    /// Serialize the message to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize a message from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Session start message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartMessage {
    /// Project ID
    pub project_id: String,
    /// Stack ID
    pub stack_id: String,
    /// Stack name
    pub stack_name: Option<String>,
}

/// Session end message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndMessage {
    /// Exit code
    pub code: i32,
}

/// Session status message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusMessage {
    /// Status type
    pub status: String,
    /// Optional message
    pub message: Option<String>,
}

/// Prompt message from user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Prompt text
    pub text: String,
}

/// Response message from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    /// Response text (may be chunked)
    pub text: String,
    /// Whether this is the final chunk
    pub done: bool,
}

/// Permission request from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestMessage {
    /// Permission ID
    pub id: String,
    /// Tool being requested
    pub tool: String,
    /// Operation description
    pub operation: String,
}

/// Permission decision from user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecisionMessage {
    /// Permission ID (matches request)
    pub id: String,
    /// Whether granted
    pub granted: bool,
}

/// Question from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionMessage {
    /// Question ID
    pub id: String,
    /// Question text
    pub question: String,
    /// Answer options
    pub options: Vec<String>,
}

/// Answer to question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerMessage {
    /// Question ID (matches question)
    pub id: String,
    /// Selected answer
    pub answer: String,
}

/// Abort session message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortMessage {
    /// Reason for abort
    pub reason: Option<String>,
}

/// Generic text message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessage {
    /// Message text
    pub text: String,
}

/// Protocol implementation helper.
pub struct GitButlerProtocol;

impl GitButlerProtocol {
    /// Create a session start message.
    pub fn session_start(
        project_id: String,
        stack_id: String,
        stack_name: Option<String>,
    ) -> GitButlerMessage {
        GitButlerMessage::SessionStart(SessionStartMessage {
            project_id,
            stack_id,
            stack_name,
        })
    }

    /// Create a session end message.
    pub fn session_end(code: i32) -> GitButlerMessage {
        GitButlerMessage::SessionEnd(SessionEndMessage { code })
    }

    /// Create a prompt message.
    pub fn prompt(text: String) -> GitButlerMessage {
        GitButlerMessage::Prompt(PromptMessage { text })
    }

    /// Create a response message.
    pub fn response(text: String, done: bool) -> GitButlerMessage {
        GitButlerMessage::Response(ResponseMessage { text, done })
    }

    /// Create a permission request.
    pub fn permission_request(id: String, tool: String, operation: String) -> GitButlerMessage {
        GitButlerMessage::PermissionRequest(PermissionRequestMessage {
            id,
            tool,
            operation,
        })
    }

    /// Create a permission decision.
    pub fn permission_decision(id: String, granted: bool) -> GitButlerMessage {
        GitButlerMessage::PermissionDecision(PermissionDecisionMessage { id, granted })
    }

    /// Create a question.
    pub fn question(id: String, question: String, options: Vec<String>) -> GitButlerMessage {
        GitButlerMessage::Question(QuestionMessage {
            id,
            question,
            options,
        })
    }

    /// Create an answer.
    pub fn answer(id: String, answer: String) -> GitButlerMessage {
        GitButlerMessage::Answer(AnswerMessage { id, answer })
    }

    /// Create an abort message.
    pub fn abort(reason: Option<String>) -> GitButlerMessage {
        GitButlerMessage::Abort(AbortMessage { reason })
    }

    /// Create a text message.
    pub fn text(text: String) -> GitButlerMessage {
        GitButlerMessage::Text(TextMessage { text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_channel() {
        assert_eq!(project_channel("myorg/myrepo"), "#myorg-myrepo");
    }

    #[test]
    fn test_session_channel() {
        assert_eq!(
            session_channel("mattias", "my-feature", "claude"),
            "#claude-mattias/my-feature"
        );
    }

    #[test]
    fn test_session_channel_with_branch_slash() {
        assert_eq!(
            session_channel("mattias", "feature/my-branch", "claude"),
            "#claude-mattias/feature/my-branch"
        );
    }

    #[test]
    fn test_message_serialization() {
        let msg = GitButlerProtocol::prompt("Hello world".to_string());
        let json = msg.to_json().unwrap();
        assert!(json.contains("prompt"));
        assert!(json.contains("Hello world"));

        let parsed = GitButlerMessage::from_json(&json).unwrap();
        match parsed {
            GitButlerMessage::Prompt(p) => assert_eq!(p.text, "Hello world"),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_session_start() {
        let msg = GitButlerProtocol::session_start(
            "proj123".to_string(),
            "stack456".to_string(),
            Some("feature".to_string()),
        );
        let json = msg.to_json().unwrap();
        assert!(json.contains("proj123"));
        assert!(json.contains("stack456"));
        assert!(json.contains("feature"));
    }
}
