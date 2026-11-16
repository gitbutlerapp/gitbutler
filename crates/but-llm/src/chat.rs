use std::fmt::Display;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "camelCase")]
pub enum ChatMessage {
    User(String),
    Assistant(String),
}

impl Display for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatMessage::User(content) => write!(f, "<user_message>\n{content}\n</user_message>"),
            ChatMessage::Assistant(content) => write!(f, "<but-bot\n{content}\n</but-bot>"),
        }
    }
}

impl From<&str> for ChatMessage {
    fn from(msg: &str) -> Self {
        ChatMessage::User(msg.to_string())
    }
}

impl From<String> for ChatMessage {
    fn from(msg: String) -> Self {
        ChatMessage::User(msg)
    }
}
