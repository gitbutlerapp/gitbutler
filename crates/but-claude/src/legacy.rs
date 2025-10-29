//! Legacy types for deserializing older Claude message formats.
//!

use serde::{Deserialize, Serialize};

/// Represents the kind of content in a Claude message.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum ClaudeMessageContent {
    /// Came from Claude standard out stream
    ClaudeOutput(serde_json::Value),
    /// Inserted via  GitButler (what the user typed)
    UserInput(UserInput),
    /// Metadata provided by GitButler around the Claude Code statuts
    GitButlerMessage(GitButlerMessage),
}

impl From<ClaudeMessageContent> for crate::MessagePayload {
    fn from(value: ClaudeMessageContent) -> Self {
        match value {
            ClaudeMessageContent::ClaudeOutput(data) => {
                crate::MessagePayload::Claude(crate::ClaudeOutput { data })
            }
            ClaudeMessageContent::UserInput(user_input) => {
                crate::MessagePayload::User(user_input.into())
            }
            ClaudeMessageContent::GitButlerMessage(gb_message) => {
                crate::MessagePayload::System(gb_message.into())
            }
        }
    }
}

/// Metadata provided by GitButler.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum GitButlerMessage {
    /// Claude code has exited naturally.
    ClaudeExit {
        code: i32,
        message: String,
    },
    /// Claude code has exited due to a user abortion.
    UserAbort,
    UnhandledException {
        message: String,
    },
    /// Compact operation has started.
    CompactStart,
    /// Compact operation has finished.
    CompactFinished {
        summary: String,
    },
}

impl From<GitButlerMessage> for crate::SystemMessage {
    fn from(value: GitButlerMessage) -> Self {
        match value {
            GitButlerMessage::ClaudeExit { code, message } => {
                crate::SystemMessage::ClaudeExit { code, message }
            }
            GitButlerMessage::UserAbort => crate::SystemMessage::UserAbort,
            GitButlerMessage::UnhandledException { message } => {
                crate::SystemMessage::UnhandledException { message }
            }
            GitButlerMessage::CompactStart => crate::SystemMessage::CompactStart,
            GitButlerMessage::CompactFinished { summary } => {
                crate::SystemMessage::CompactFinished { summary }
            }
        }
    }
}

/// Represents user input in a Claude session.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    /// The user message
    pub message: String,
    /// Optional attached file references
    pub attachments: Option<Vec<PromptAttachment>>,
}

impl From<UserInput> for crate::UserInput {
    fn from(value: UserInput) -> Self {
        crate::UserInput {
            message: value.message,
            attachments: value
                .attachments
                .map(|atts| atts.into_iter().map(|a| a.into()).collect()),
        }
    }
}

/// Represents a file attachment with full content (used in API input).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PromptAttachment {
    Lines(LinesAttachment),
    File(FileAttachment),
    Commit(CommitAttachment),
}

impl From<PromptAttachment> for crate::PromptAttachment {
    fn from(value: PromptAttachment) -> Self {
        match value {
            PromptAttachment::Lines(lines) => crate::PromptAttachment::Lines(lines.into()),
            PromptAttachment::File(file) => crate::PromptAttachment::File(file.into()),
            PromptAttachment::Commit(commit) => crate::PromptAttachment::Commit(commit.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachment {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_id: Option<String>,
}

impl From<FileAttachment> for crate::FileAttachment {
    fn from(value: FileAttachment) -> Self {
        crate::FileAttachment {
            commit_id: value.commit_id,
            path: value.path,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinesAttachment {
    path: String,
    start: usize,
    end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_id: Option<String>,
}

impl From<LinesAttachment> for crate::LinesAttachment {
    fn from(value: LinesAttachment) -> Self {
        crate::LinesAttachment {
            commit_id: value.commit_id,
            path: value.path,
            start: value.start,
            end: value.end,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommitAttachment {
    commit_id: String,
}

impl From<CommitAttachment> for crate::CommitAttachment {
    fn from(value: CommitAttachment) -> Self {
        crate::CommitAttachment {
            commit_id: value.commit_id,
        }
    }
}
