use anyhow::Result;
use uuid::Uuid;

impl TryFrom<but_db::ClaudeSession> for crate::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudeSession) -> Result<Self, Self::Error> {
        Ok(crate::ClaudeSession {
            id: Uuid::parse_str(&value.id)?,
            current_id: Uuid::parse_str(&value.current_id)?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<crate::ClaudeSession> for but_db::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeSession) -> Result<Self, Self::Error> {
        Ok(but_db::ClaudeSession {
            id: value.id.to_string(),
            current_id: value.current_id.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone, Copy, strum::EnumString, strum::Display)]
enum ClaudeMessageDbContentType {
    ClaudeOutput,
    UserInput,
}

impl TryFrom<but_db::ClaudeMessage> for crate::ClaudeMessage {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudeMessage) -> Result<Self, Self::Error> {
        let content_type: ClaudeMessageDbContentType = value.content_type.parse()?;
        let content = match content_type {
            ClaudeMessageDbContentType::ClaudeOutput => {
                crate::ClaudeMessageContent::ClaudeOutput(serde_json::from_str(&value.content)?)
            }
            ClaudeMessageDbContentType::UserInput => {
                crate::ClaudeMessageContent::UserInput(serde_json::from_str(&value.content)?)
            }
        };
        Ok(crate::ClaudeMessage {
            id: Uuid::parse_str(&value.id)?,
            session_id: Uuid::parse_str(&value.session_id)?,
            created_at: value.created_at,
            content,
        })
    }
}

impl TryFrom<crate::ClaudeMessage> for but_db::ClaudeMessage {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeMessage) -> Result<Self, Self::Error> {
        let content_type = match value.content {
            crate::ClaudeMessageContent::ClaudeOutput(_) => {
                ClaudeMessageDbContentType::ClaudeOutput
            }
            crate::ClaudeMessageContent::UserInput(_) => ClaudeMessageDbContentType::UserInput,
        };
        Ok(but_db::ClaudeMessage {
            id: value.id.to_string(),
            session_id: value.session_id.to_string(),
            created_at: value.created_at,
            content_type: content_type.to_string(),
            content: serde_json::to_string(&value.content)?,
        })
    }
}
