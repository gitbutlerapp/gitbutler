use serde::{Deserialize, Serialize};

use crate::{
    llm::{LLM, LLMParams, LLMResponse},
    types::{Message, Tool, ToolCall},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct APIMessage {
    role: String,
    content: String,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize)]
struct APIProvider {
    only: Option<Vec<String>>,
}

#[derive(Serialize)]
struct APIBody {
    model: String,
    messages: Vec<Message>,
    provider: Option<APIProvider>,
    tools: Vec<Tool>,
}

#[derive(Deserialize, Clone)]
struct APIChoice {
    message: APIMessage,
}

#[derive(Deserialize, Clone)]
struct APIResponse {
    message: Option<APIMessage>,
    choices: Option<Vec<APIChoice>>,
}

/// An LLM implementation for providers that are compatible with OpenAI's
/// interface.
pub struct OpenAILikeLLM {
    pub completion_url: String,
    pub model: String,
    pub provider: Option<String>,
    pub token: Option<String>,
}

impl LLM for OpenAILikeLLM {
    fn perform(&self, LLMParams { messages, tools }: LLMParams) -> LLMResponse {
        let client = reqwest::blocking::Client::new();
        let mut request = client
            .post(&self.completion_url)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_string(&APIBody {
                    model: self.model.clone(),
                    messages,
                    provider: self.provider.clone().map(|provider| APIProvider {
                        only: Some(vec![provider]),
                    }),
                    tools,
                })
                .unwrap(),
            );

        if let Some(token) = self.token.clone() {
            request = request.bearer_auth(&token);
        };

        let result = request.send().unwrap();

        let response = result.text().unwrap();
        let response: APIResponse = serde_json::from_str(&response).unwrap();

        let message = response
            .message
            .unwrap_or_else(|| response.choices.unwrap().first().unwrap().message.clone());

        if let Some(tool_calls) = &message.tool_calls {
            LLMResponse::ToolCalls {
                message: message.content.clone(),
                tool_calls: tool_calls.clone(),
            }
        } else {
            LLMResponse::Message {
                message: message.content.clone(),
            }
        }
    }
}
