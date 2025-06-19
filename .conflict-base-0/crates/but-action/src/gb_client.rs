use std::vec;

use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::serialize::StringOrObject;

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct ChatMessage {
    /// The contents of the developer message.
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Tool {
    /// The name of the tool to call.
    /// Should be unique within the context of the chat completion.
    pub name: String,
    /// The description of the tool.
    /// This should be a human-readable description of what the tool does. The more detailed the better.
    pub description: String,
    /// The JSON schema for the tool's input parameters.
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct OpenAIFunction {
    /// The name of the function to call.
    pub name: String,
    /// Stringified JSON object of the function's parameters.
    pub arguments: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct OpenAIToolCall {
    /// The ID of the tool call.
    pub id: String,
    /// The type of the tool call.
    /// This should be set to "function" for function calls.
    #[serde(rename = "type")]
    call_type: String,
    /// The name and parameters of the function to call.
    function: OpenAIFunction,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct OpenAIAssistantChatMessage {
    /// The contents of the developer message.
    pub content: Option<String>,
    /// The tool calls made by the model, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct OpenAIToolResultChatMessage {
    /// The ID of the tool call that this message is responding to.
    pub tool_call_id: String,
    /// The contents of the developer message.
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "role")]
#[serde(rename_all = "lowercase")]
pub enum OpenAIChatCompletionMessage {
    System(ChatMessage),
    User(ChatMessage),
    Assistant(OpenAIAssistantChatMessage),
    Tool(OpenAIToolResultChatMessage),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct OpenAIStructuredOutput {
    /// The list of messages to be completed.
    pub messages: Vec<OpenAIChatCompletionMessage>,
    /// The maximum number of tokens to generate in the completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// The temperature to use for sampling.
    /// From 0.0 to 2.0, where lower values make the output more deterministic and higher values make it more random.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// The schema for the response format, described as a JSON Schema object.
    pub json_schema: serde_json::Value,
    /// The available tools for the llm to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct OpenAIStructuredOutputResponse<T> {
    /// The response in the format specified by the json_schema.
    pub response: Option<T>,
    /// The tool calls made by the model, if any.
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AntrhopicToolResult {
    /// The type of the content.
    /// This should be set to "tool_result" for tool results.
    #[serde(rename = "type")]
    pub constent_type: String,
    /// The ID of the tool call that this message is responding to.
    pub tool_use_id: String,
    /// The result of the tool call.
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AnthropicUserChatMessage {
    /// The contents of the user message.
    /// This can be a string or a tool result object.
    pub content: StringOrObject<Vec<AntrhopicToolResult>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct AnthropicToolCall {
    /// The ID of the tool call.
    pub id: String,
    /// The type of the tool call.
    /// This should be set to "tool_use" for tool calls.
    #[serde(rename = "type")]
    pub call_type: String,
    /// The name of the function to call.
    pub name: String,
    /// The input parameters for the tool call.
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AnthropicAssistantChatMessage {
    /// The contents of the developer message.
    pub content: Option<Vec<AnthropicToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "role")]
#[serde(rename_all = "lowercase")]
pub enum AnthropicChatCompletionMessage {
    Assistant(AnthropicAssistantChatMessage),
    User(AnthropicUserChatMessage),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
struct AnthropicStructuredOutputInternal {
    /// The type of model to use.
    /// This should be set to "anthropic".
    /// TODO: Not sure if this is the best way of expessing this
    model_kind: String,
    /// The list of messages to be completed.
    pub messages: Vec<AnthropicChatCompletionMessage>,
    /// The maximum number of tokens to generate in the completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// The temperature to use for sampling.
    /// From 0.0 to 2.0, where lower values make the output more deterministic and higher values make it more random.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// The schema for the response format, described as a JSON Schema object.
    pub json_schema: serde_json::Value,
    /// The available tools for the llm to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

impl From<&AnthropicStructuredOutput> for AnthropicStructuredOutputInternal {
    fn from(value: &AnthropicStructuredOutput) -> Self {
        Self {
            model_kind: "anthropic".to_string(),
            messages: value.messages.clone(),
            max_tokens: value.max_tokens,
            temperature: value.temperature,
            json_schema: value.json_schema.clone(),
            tools: value.tools.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct AnthropicStructuredOutput {
    /// The system message to use for the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The list of messages to be completed.
    pub messages: Vec<AnthropicChatCompletionMessage>,
    /// The maximum number of tokens to generate in the completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// The temperature to use for sampling.
    /// From 0.0 to 2.0, where lower values make the output more deterministic and higher values make it more random.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// The schema for the response format, described as a JSON Schema object.
    pub json_schema: serde_json::Value,
    /// The available tools for the llm to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct AnthropicStructuredOutputResponse<T> {
    /// The response in the format specified by the json_schema.
    pub response: Option<T>,
    /// The tool calls made by the model, if any.
    pub tool_calls: Option<Vec<AnthropicToolCall>>,
}

#[allow(unused)]
pub struct GBClient {
    client: Client,
    token: String,
    base_url: String,
}

#[allow(unused)]
impl GBClient {
    pub fn new(base_url: String, token: String) -> Self {
        let client = Client::builder()
            .user_agent("gb-client/1.0")
            .build()
            .expect("Failed to create HTTP client");
        GBClient {
            base_url,
            client,
            token,
        }
    }
    async fn post_json<TReq, TResp>(&self, url: &str, body: &TReq) -> Result<TResp, reqwest::Error>
    where
        TReq: Serialize + ?Sized,
        TResp: DeserializeOwned,
    {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("X-Auth-Token", HeaderValue::from_str(&self.token).unwrap());

        let url = format!("{}/{}", self.base_url, url);

        let resp_result = self
            .client
            .post(&url)
            .json(body)
            .headers(headers)
            .send()
            .await;

        match resp_result {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json = resp.json::<TResp>().await?;
                    Ok(json)
                } else {
                    println!("Request failed:");
                    println!("  Status: {}", resp.status());
                    let err = resp.error_for_status_ref().unwrap_err();
                    let text = resp.text().await?;
                    println!("  Error: {}", text);
                    Err(err)
                }
            }
            Err(err) => {
                eprintln!("Request error:");
                eprintln!("  Error: {}", err);
                Err(err)
            }
        }
    }

    async fn structured_output<TReq, TResp>(&self, body: &TReq) -> Result<TResp, reqwest::Error>
    where
        TReq: Serialize + ?Sized,
        TResp: DeserializeOwned,
    {
        self.post_json("evaluate_prompt/structured", body).await
    }

    /// Sends a structured output request to the GB API using Open AI
    pub async fn open_ai_structured_output<TResp>(
        &self,
        request: &OpenAIStructuredOutput,
    ) -> Result<OpenAIStructuredOutputResponse<TResp>, reqwest::Error>
    where
        TResp: DeserializeOwned,
    {
        self.structured_output(request).await
    }

    /// Sends a structured output request to the GB API using Anthropic
    pub async fn anthropic_structured_output<TResp>(
        &self,
        request: &AnthropicStructuredOutput,
    ) -> Result<AnthropicStructuredOutputResponse<TResp>, reqwest::Error>
    where
        TResp: DeserializeOwned,
    {
        let internal_request: AnthropicStructuredOutputInternal = request.into();
        self.structured_output(&internal_request).await
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct CommitMessage {
    pub commit_message: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct GetCommitContextParams {
    pub prompt: String,
}

#[allow(unused)]
fn get_commit_context(params: &GetCommitContextParams) -> String {
    println!("Getting commit context for prompt: {}", params.prompt);
    "Make the commit message description should be written as a haiku poem.".to_string()
}

#[allow(unused)]
pub fn commit_message_blocking_open_ai(
    api_key: &str,
    external_summary: &str,
    external_prompt: &str,
    diff: &str,
) -> anyhow::Result<String> {
    let api_key_owned = api_key.to_string();
    let change_summary_owned = external_summary.to_string();
    let external_prompt_owned = external_prompt.to_string();
    let diff_owned = diff.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(commit_message_open_ai(
                &api_key_owned,
                &change_summary_owned,
                &external_prompt_owned,
                &diff_owned,
            ))
    })
    .join()
    .unwrap()
}

#[allow(unused)]
pub async fn commit_message_open_ai(
    api_key: &str,
    external_summary: &str,
    external_prompt: &str,
    diff: &str,
) -> anyhow::Result<String> {
    let system_message =
        "You are a version control assistant that helps with Git branch committing.".to_string();
    let user_message = format!(
        "Extract the git commit data from the prompt, summary and diff output. Return the commit message. Determine from this AI prompt, summary and diff output what the git commit data should be.\n\n{}\n\nHere is the data:\n\nPrompt: {}\n\nSummary: {}\n\nDiff:\n```\n{}\n```\n\n",
        "Default commit message instructions", external_prompt, external_summary, diff
    );

    let client = GBClient::new(
        "https://app.gitbutler.com/api".to_string(),
        api_key.to_string(),
    );

    let commit_message_schema = schema_for!(CommitMessage);
    let json_schema = serde_json::to_value(commit_message_schema).unwrap();

    let commit_context_params_schema = schema_for!(GetCommitContextParams);

    let tool = Tool {
        name: "get_commit_context".to_string(),
        description: "Get the commit context based on the prompt".to_string(),
        parameters: serde_json::to_value(commit_context_params_schema).unwrap(),
    };

    let messages = vec![
        OpenAIChatCompletionMessage::System(ChatMessage {
            content: system_message,
        }),
        OpenAIChatCompletionMessage::User(ChatMessage {
            content: user_message,
        }),
    ];

    let request = OpenAIStructuredOutput {
        messages,
        max_tokens: None,
        temperature: None,
        json_schema,
        tools: Some(vec![tool]),
    };

    let mut request = request;

    // This is the function calling loop.
    // - If we get tool calls, we handle them and continue the loop with the updated request.
    // - We brake if we get a commit message response.
    // - If we get no response and no tool calls, we break with an error.
    loop {
        let response: OpenAIStructuredOutputResponse<CommitMessage> =
            client.open_ai_structured_output(&request).await?;

        // If there are tool calls, handle them first
        if let Some(tool_calls) = response.tool_calls {
            // For now, only handle "get_commit_context"
            let mut new_messages = request.messages.clone();
            for tool_call in tool_calls {
                if tool_call.call_type == "function" {
                    match tool_call.function.name.as_str() {
                        "get_commit_context" => {
                            let params: GetCommitContextParams =
                                serde_json::from_str(&tool_call.function.arguments)?;
                            let result = get_commit_context(&params);

                            // Add the tool call result as an assistant message
                            new_messages.push(OpenAIChatCompletionMessage::Assistant(
                                OpenAIAssistantChatMessage {
                                    content: None,
                                    tool_calls: Some(vec![tool_call.clone()]),
                                },
                            ));

                            // Add the tool response as a user message
                            new_messages.push(OpenAIChatCompletionMessage::Tool(
                                OpenAIToolResultChatMessage {
                                    tool_call_id: tool_call.id,
                                    content: result,
                                },
                            ));
                        }
                        _ => {
                            // TODO: Handle unknown tool calls gracefully
                            return Err(anyhow::anyhow!(
                                "Unknown tool call: {}",
                                tool_call.function.name
                            ));
                        }
                    }
                }
            }
            // Update the request with the new messages
            request.messages = new_messages;
            // Continue the loop to send the updated request
            continue;
        }

        // If we have a commit message, return it
        if let Some(cm) = response.response {
            return Ok(cm.commit_message);
        }

        // No response and no tool calls, break with error
        break Err(anyhow::anyhow!(
            "No commit message or tool calls returned from the AI model"
        ));
    }
}

#[allow(unused)]
pub fn commit_message_blocking_anthropic(
    api_key: &str,
    external_summary: &str,
    external_prompt: &str,
    diff: &str,
) -> anyhow::Result<String> {
    let api_key_owned = api_key.to_string();
    let change_summary_owned = external_summary.to_string();
    let external_prompt_owned = external_prompt.to_string();
    let diff_owned = diff.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(commit_message_anthropic(
                &api_key_owned,
                &change_summary_owned,
                &external_prompt_owned,
                &diff_owned,
            ))
    })
    .join()
    .unwrap()
}

#[allow(unused)]
pub async fn commit_message_anthropic(
    api_key: &str,
    external_summary: &str,
    external_prompt: &str,
    diff: &str,
) -> anyhow::Result<String> {
    let system_message =
        "You are a version control assistant that helps with Git branch committing.".to_string();
    let user_message = format!(
        "Extract the git commit data from the prompt, summary and diff output. Return the commit message. Determine from this AI prompt, summary and diff output what the git commit data should be.\n\n{}\n\nHere is the data:\n\nPrompt: {}\n\nSummary: {}\n\nDiff:\n```\n{}\n```\n\n",
        "Default commit message instructions", external_prompt, external_summary, diff
    );

    let client = GBClient::new(
        "https://app.gitbutler.com/api".to_string(),
        api_key.to_string(),
    );

    let commit_message_schema = schema_for!(CommitMessage);
    let json_schema = serde_json::to_value(commit_message_schema).unwrap();

    let commit_context_params_schema = schema_for!(GetCommitContextParams);

    let tool = Tool {
        name: "get_commit_context".to_string(),
        description: "Get the commit context based on the prompt".to_string(),
        parameters: serde_json::to_value(commit_context_params_schema).unwrap(),
    };

    let request = AnthropicStructuredOutput {
        system: Some(system_message),
        messages: vec![AnthropicChatCompletionMessage::User(
            AnthropicUserChatMessage {
                content: StringOrObject::String(user_message),
            },
        )],
        max_tokens: None,
        temperature: None,
        json_schema,
        tools: Some(vec![tool]),
    };

    let mut request = request;

    // This is the function calling loop.
    // - If we get tool calls, we handle them and continue the loop with the updated request.
    // - We break if we get a commit message response.
    // - If we get no response and no tool calls, we break with an error.
    loop {
        let response: AnthropicStructuredOutputResponse<CommitMessage> =
            client.anthropic_structured_output(&request).await?;

        // If there are tool calls, handle them first
        if let Some(tool_calls) = response.tool_calls {
            // For now, only handle "get_commit_context"
            let mut new_messages = request.messages.clone();
            for tool_call in tool_calls {
                if tool_call.call_type == "tool_use" {
                    match tool_call.name.as_str() {
                        "get_commit_context" => {
                            // Add the tool call result as an assistant message
                            new_messages.push(AnthropicChatCompletionMessage::Assistant(
                                AnthropicAssistantChatMessage {
                                    content: Some(vec![tool_call.clone()]),
                                },
                            ));

                            let params: GetCommitContextParams =
                                serde_json::from_value(tool_call.input)?;
                            let result = get_commit_context(&params);

                            // Add the tool call result as a user message (Anthropic format)
                            new_messages.push(AnthropicChatCompletionMessage::User(
                                AnthropicUserChatMessage {
                                    content: StringOrObject::Object(vec![AntrhopicToolResult {
                                        constent_type: "tool_result".to_string(),
                                        tool_use_id: tool_call.id,
                                        content: result,
                                    }]),
                                },
                            ));
                        }
                        _ => {
                            // TODO: Handle unknown tool calls gracefully
                            return Err(anyhow::anyhow!("Unknown tool call: {}", tool_call.name));
                        }
                    }
                }
            }
            // Update the request with the new messages
            request.messages = new_messages;
            // Continue the loop to send the updated request
            continue;
        }

        // If we have a commit message, return it
        if let Some(cm) = response.response {
            return Ok(cm.commit_message);
        }

        // No response and no tool calls, break with error
        break Err(anyhow::anyhow!(
            "No commit message or tool calls returned from the AI model"
        ));
    }
}
