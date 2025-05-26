use crate::types::Message;
use serde::{Deserialize, Serialize};

pub enum LLMParams {
    Message { messages: Vec<Message> },
}

pub enum LLMResponse {
    Message { message: String },
}

pub trait LLM {
    fn perform(&self, params: LLMParams) -> LLMResponse;
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
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: Message,
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
    fn perform(&self, params: LLMParams) -> LLMResponse {
        match params {
            LLMParams::Message { messages } => {
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
                        })
                        .unwrap(),
                    )
                    .send()
                    .unwrap();

                let reponse: OpenRouterAPIResponse = result.json().unwrap();

                LLMResponse::Message {
                    message: reponse.choices.first().unwrap().message.content.clone(),
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    pub struct MockLLM<CB: Fn(String) -> String> {
        pub callback: CB,
    }

    impl<CB: Fn(String) -> String> LLM for MockLLM<CB> {
        fn perform(&self, params: LLMParams) -> LLMResponse {
            match params {
                LLMParams::Message { messages } => {
                    let last = messages.last().unwrap();
                    LLMResponse::Message {
                        message: (self.callback)(last.content.clone()),
                    }
                }
            }
        }
    }
}
