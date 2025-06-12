use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "role")]
#[serde(rename_all = "lowercase")]
pub enum OpenAIChatCompletionMessage {
    System(ChatMessage),
    User(ChatMessage),
    Assistant(ChatMessage),
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
    pub response: Option<T>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "role")]
#[serde(rename_all = "lowercase")]
pub enum AnthropicChatCompletionMessage {
    System(ChatMessage),
    User(ChatMessage),
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
    pub response: Option<T>,
}

pub struct GBClient {
    client: Client,
    token: String,
    base_url: String,
}

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

    let schema = schema_for!(CommitMessage);
    let json_schema = serde_json::to_value(schema).unwrap();

    let request = OpenAIStructuredOutput {
        messages: vec![
            OpenAIChatCompletionMessage::System(ChatMessage {
                content: system_message,
            }),
            OpenAIChatCompletionMessage::User(ChatMessage {
                content: user_message,
            }),
        ],
        max_tokens: None,
        temperature: None,
        json_schema,
        tools: None,
    };

    let response: OpenAIStructuredOutputResponse<CommitMessage> =
        client.open_ai_structured_output(&request).await?;

    response
        .response
        .map(|cm| cm.commit_message)
        .ok_or_else(|| anyhow::anyhow!("No commit message returned from the AI model"))
}

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

    let schema = schema_for!(CommitMessage);
    let json_schema = serde_json::to_value(schema).unwrap();

    let request = AnthropicStructuredOutput {
        system: Some(system_message),
        messages: vec![AnthropicChatCompletionMessage::User(ChatMessage {
            content: user_message,
        })],
        max_tokens: None,
        temperature: None,
        json_schema,
        tools: None,
    };

    let response: AnthropicStructuredOutputResponse<CommitMessage> =
        client.anthropic_structured_output(&request).await?;

    response
        .response
        .map(|cm| cm.commit_message)
        .ok_or_else(|| anyhow::anyhow!("No commit message returned from the AI model"))
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct CommitMessage {
    pub commit_message: String,
}
