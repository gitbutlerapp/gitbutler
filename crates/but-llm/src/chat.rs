use std::fmt::Display;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallContent {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponseContent {
    pub id: String,
    pub result: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "camelCase")]
pub enum ChatMessage {
    User(String),
    Assistant(String),
    ToolCall(ToolCallContent),
    ToolResponse(ToolResponseContent),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

pub type StreamToolCallResult = (Option<Vec<ToolCall>>, Option<String>);

impl Display for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatMessage::User(content) => write!(f, "<user_message>\n{content}\n</user_message>"),
            ChatMessage::Assistant(content) => write!(f, "<but-bot\n{content}\n</but-bot>"),
            ChatMessage::ToolCall(content) => write!(
                f,
                "
<but-bot-tool-call>
    <id>{}</id>
    <name>{}</name>
    <arguments>{}</arguments>
</but-bot-tool-call>",
                content.id, content.name, content.arguments
            ),
            ChatMessage::ToolResponse(content) => write!(
                f,
                "
<but-bot-tool-response>
    <id>{}</id>
    <result>{}</result>
</but-bot-tool-response>",
                content.id,
                clamp_result_content(content)
            ),
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

fn clamp_result_content(result: &ToolResponseContent) -> String {
    if result.result.len() > 500 {
        "Result too big to be displayed".to_string()
    } else {
        result.result.to_owned()
    }
}
/// Result of a tool calling loop with streaming
pub struct ConversationResult {
    pub final_response: String,
    pub message_history: Vec<ChatMessage>,
}
