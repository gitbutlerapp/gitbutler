use serde::{Deserialize, Serialize};

use crate::{
    llm::{LLM, LLMParams, LLMResponse},
    types::{Message, Tool, ToolCall},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct OpenRouterMessage {
    role: String,
    content: String,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize)]
struct OpenRouterProvider {
    only: Option<Vec<String>>,
}

#[derive(Serialize)]
struct OpenRouterAPIBody {
    model: String,
    messages: Vec<Message>,
    provider: Option<OpenRouterProvider>,
    tools: Vec<Tool>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}

#[derive(Deserialize)]
struct OpenRouterAPIResponse {
    choices: Vec<OpenRouterChoice>,
}

pub struct OpenRouter {
    model: String,
    provider: String,
    token: gitbutler_secret::Sensitive<String>,
}

impl LLM for OpenRouter {
    fn perform(&self, LLMParams { messages, tools }: LLMParams) -> LLMResponse {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&self.token.0)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_string(&OpenRouterAPIBody {
                    model: self.model.clone(),
                    messages,
                    provider: Some(OpenRouterProvider {
                        only: Some(vec![self.provider.clone()]),
                    }),
                    tools,
                })
                .unwrap(),
            )
            .send()
            .unwrap();

        let reponse: OpenRouterAPIResponse = result.json().unwrap();

        let choice = reponse.choices.first().unwrap();

        if let Some(tool_calls) = &choice.message.tool_calls {
            LLMResponse::ToolCalls {
                message: choice.message.content.clone(),
                tool_calls: tool_calls.clone(),
            }
        } else {
            LLMResponse::Message {
                message: choice.message.content.clone(),
            }
        }
    }
}
