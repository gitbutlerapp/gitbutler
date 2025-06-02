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

#[derive(Deserialize, Clone)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}

#[derive(Deserialize, Clone)]
struct OpenRouterAPIResponse {
    message: Option<OpenRouterMessage>,
    choices: Option<Vec<OpenRouterChoice>>,
}

pub struct OpenRouter {
    pub model: String,
    pub provider: Option<String>,
    pub token: Option<gitbutler_secret::Sensitive<String>>,
}

impl LLM for OpenRouter {
    fn perform(&self, LLMParams { messages, tools }: LLMParams) -> LLMResponse {
        let client = reqwest::blocking::Client::new();
        let mut request = client
            .post("http://127.0.0.1:11434/api/chat")
            // .bearer_auth(&self.token.0)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_string(&OpenRouterAPIBody {
                    model: self.model.clone(),
                    messages,
                    provider: self.provider.clone().map(|provider| OpenRouterProvider {
                        only: Some(vec![provider]),
                    }),
                    tools,
                })
                .unwrap(),
            );

        if let Some(token) = self.token.clone() {
            request = request.bearer_auth((*token).clone());
        };

        let result = request.send().unwrap();

        let response = result.text().unwrap();
        dbg!(&response);
        let response: OpenRouterAPIResponse = serde_json::from_str(&response).unwrap();

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
