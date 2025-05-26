use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct ConversationId(uuid::Uuid);

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
}

impl Response {
    pub fn id(&self) -> ConversationId {
        match self {
            Response::ThreadCreated { id } => *id,
            Response::MessageRecieved { id } => *id,
            Response::ReplyReceived { id } => *id,
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
    pub required: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolFunctionParameters {
    #[serde(rename = "type")]
    pub parameters_type: ToolFunctionParametersType,
    pub properties: std::collections::HashMap<String, ToolFunctionParameter>,
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

#[cfg(test)]
mod test {
    use super::*;

    // Example structure taken from the OpenAI API reference
    #[test]
    fn serialize_tool() {
        let mut properties = std::collections::HashMap::new();

        properties.insert(
            "location".to_string(),
            ToolFunctionParameter {
                parameter_type: ToolFunctionParameterType::String,
                description: "City and country e.g. Bogotá, Colombia".to_string(),
                required: true,
            },
        );

        properties.insert(
            "units".to_string(),
            ToolFunctionParameter {
                parameter_type: ToolFunctionParameterType::String,
                description: "Units the temperature will be returned in.".to_string(),
                required: true,
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
          "description": "City and country e.g. Bogotá, Colombia",
          "required": true
        },
        "units": {
          "type": "string",
          "description": "Units the temperature will be returned in.",
          "required": true
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
}
