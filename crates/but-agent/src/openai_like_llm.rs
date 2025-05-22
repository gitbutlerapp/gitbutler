use serde::{Deserialize, Serialize};

use crate::{
    llm::{LLM, LLMParams, LLMResponse},
    types::{Message, Tool, ToolCall},
};
use anyhow::Result;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    fn perform(&self, LLMParams { messages, tools }: LLMParams) -> Result<LLMResponse> {
        let client = reqwest::blocking::Client::new();
        let mut request = client
            .post(&self.completion_url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&APIBody {
                model: self.model.clone(),
                messages,
                provider: self.provider.clone().map(|provider| APIProvider {
                    only: Some(vec![provider]),
                }),
                tools,
            })?);

        if let Some(token) = self.token.clone() {
            request = request.bearer_auth(&token);
        };

        let result = request.send()?;

        let response = result.text()?;
        let response: APIResponse = serde_json::from_str(&response)?;

        let message = response
            .message
            .ok_or(anyhow::anyhow!("No message found"))
            .or_else(|_| {
                Ok::<_, anyhow::Error>(
                    response
                        .choices
                        .ok_or(anyhow::anyhow!("No choices found"))?
                        .first()
                        .ok_or(anyhow::anyhow!("No choice found"))?
                        .message
                        .clone(),
                )
            })?;

        if let Some(tool_calls) = &message.tool_calls {
            Ok(LLMResponse::ToolCalls {
                message: message.content.clone(),
                tool_calls: tool_calls.clone(),
            })
        } else {
            Ok(LLMResponse::Message {
                message: message.content.clone(),
            })
        }
    }
}
