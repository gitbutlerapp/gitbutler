use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug)]
pub enum MessageRoleParseError {
    InvalidRole(String),
}

impl FromStr for MessageRole {
    type Err = MessageRoleParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(MessageRole::System),
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            "tool" => Ok(MessageRole::Tool),
            _ => Err(MessageRoleParseError::InvalidRole(s.to_string())),
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        write!(f, "{}", s)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ToolCallType {
    Function,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallFunction {
    pub name: String,
    // A stringified JSON object
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_call_type: ToolCallType,
    pub function: ToolCallFunction,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct ConversationId(pub uuid::Uuid);

// Creating Conversation Ids;
impl ConversationId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::ops::Deref for ConversationId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum Action {
    /// Starts a new thread that the user can send messages to.
    StartNewThread,
    SendMessage {
        id: ConversationId,
        message: String,
    },
}

#[derive(Debug, PartialEq)]
pub enum Response {
    ThreadCreated {
        id: ConversationId,
    },
    /// Acknoledges that a user messages has been sent; Will be sent after the
    /// user message has been persisted to the conversation store
    MessageRecieved {
        id: ConversationId,
    },
    /// Sent whenver a reponse from an LLM has been recieved
    ReplyReceived {
        id: ConversationId,
    },
    /// Sent whenver a tool call reponse from an LLM has been recieved
    ToolCallReplyRecieved {
        id: ConversationId,
    },
    /// Sent whenver a tool call response has been created
    ToolCallResponseCreated {
        id: ConversationId,
    },
}

impl Response {
    pub fn id(&self) -> ConversationId {
        match self {
            Response::ThreadCreated { id } => *id,
            Response::MessageRecieved { id } => *id,
            Response::ReplyReceived { id } => *id,
            Response::ToolCallReplyRecieved { id } => *id,
            Response::ToolCallResponseCreated { id } => *id,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ToolType {
    Function,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ToolFunctionParametersType {
    Object,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ToolFunctionParameterType {
    String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolFunctionParameter {
    #[serde(rename = "type")]
    pub parameter_type: ToolFunctionParameterType,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolFunctionParameters {
    #[serde(rename = "type")]
    pub parameters_type: ToolFunctionParametersType,
    // A BTreeMap is used to ensure some form of a stable order of properties,
    // which may or may not be helpful.
    pub properties: std::collections::BTreeMap<String, ToolFunctionParameter>,
    pub additional_properties: bool,
    pub required: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: ToolFunctionParameters,
    pub strict: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub function: ToolFunction,
}

pub enum ToolHandler {
    /// Placeholder for MCP stuff
    RawHandler(Box<dyn Fn(String) -> String>),
    ParsedHandler(Box<dyn Fn(std::collections::HashMap<String, serde_json::Value>) -> String>),
}

impl ToolHandler {
    pub(crate) fn call(&self, args: String) -> String {
        match self {
            ToolHandler::RawHandler(handler) => handler(args),
            ToolHandler::ParsedHandler(handler) => handler(serde_json::from_str(&args).unwrap()),
        }
    }
}

pub struct ToolWithHandler {
    pub tool: Tool,
    pub handler: ToolHandler,
}

#[cfg(test)]
mod test {
    use super::*;

    // Example structure taken from the OpenAI API reference
    #[test]
    fn serialize_tool() {
        let mut properties = std::collections::BTreeMap::new();

        properties.insert(
            "location".to_string(),
            ToolFunctionParameter {
                parameter_type: ToolFunctionParameterType::String,
                description: "City and country e.g. Bogotá, Colombia".to_string(),
            },
        );

        properties.insert(
            "units".to_string(),
            ToolFunctionParameter {
                parameter_type: ToolFunctionParameterType::String,
                description: "Units the temperature will be returned in.".to_string(),
            },
        );

        let tool = Tool {
            tool_type: ToolType::Function,
            function: ToolFunction {
                name: "get_weather".to_string(),
                description: "Retrieves current weather for the given location.".to_string(),
                parameters: ToolFunctionParameters {
                    parameters_type: ToolFunctionParametersType::Object,
                    properties,
                    additional_properties: false,
                    required: vec!["location".to_string(), "units".to_string()],
                },
                strict: true,
            },
        };

        assert_eq!(
            serde_json::to_string_pretty(&tool).unwrap(),
            r#"{
  "type": "function",
  "function": {
    "name": "get_weather",
    "description": "Retrieves current weather for the given location.",
    "parameters": {
      "type": "object",
      "properties": {
        "location": {
          "type": "string",
          "description": "City and country e.g. Bogotá, Colombia"
        },
        "units": {
          "type": "string",
          "description": "Units the temperature will be returned in."
        }
      },
      "additionalProperties": false,
      "required": [
        "location",
        "units"
      ]
    },
    "strict": true
  }
}"#
        );
    }

    #[test]
    fn serialize_message_author() {
        assert_eq!(
            serde_json::to_string(&MessageRole::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::User).unwrap(),
            "\"user\""
        );
    }

    #[test]
    fn serialize_message() {
        assert_eq!(
            serde_json::to_string(&Message {
                role: MessageRole::Assistant,
                content: "Hello!".into(),
                tool_call_id: None,
            })
            .unwrap(),
            r#"{"role":"assistant","content":"Hello!"}"#
        );
    }
}
