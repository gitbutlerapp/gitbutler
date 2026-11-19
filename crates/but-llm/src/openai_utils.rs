use anyhow::{Context, Result};
use async_openai::{
    Client,
    config::OpenAIConfig,
    error::OpenAIError,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
};
use futures::StreamExt;
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;

use crate::ChatMessage;

pub trait OpenAIClientProvider {
    fn client(&self) -> Result<Client<OpenAIConfig>>;
}

/// `err`, with the reason of an endpoint that answers errors as `{"error": "…"}`, as
/// GitButler's proxy does, rather than in OpenAI's shape, which async-openai can't read.
fn readable(err: OpenAIError) -> anyhow::Error {
    #[derive(serde::Deserialize)]
    struct Said {
        error: String,
    }
    if let OpenAIError::JSONDeserialize(_, body) = &err
        && let Ok(Said { error }) = serde_json::from_str(body)
    {
        return anyhow::anyhow!("The AI provider answered: {error}");
    }
    err.into()
}

impl From<ChatMessage> for ChatCompletionRequestMessage {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::User(content) => ChatCompletionRequestMessage::User(content.into()),
        }
    }
}

pub fn structured_output_blocking<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<T>> {
    let client = provider.client()?;
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];
    let model = model.to_string();

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(structured_output::<T>(&client, messages, model))
    })
    .join()
    .unwrap()
}

pub fn response_blocking(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<String>> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    let client = provider.client()?;
    let messages_owned = messages.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(response(&client, messages_owned, model))
    })
    .join()
    .unwrap()
}

pub fn stream_response_blocking(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<Option<String>> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    let client = provider.client()?;
    let messages_owned = messages.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(stream_response(&client, messages_owned, model, on_token))
    })
    .join()
    .unwrap()
}

async fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema>(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> anyhow::Result<Option<T>> {
    let schema = schema_for!(T);
    let mut schema_value = serde_json::to_value(&schema)?;
    make_schema_strict(&mut schema_value);
    // `strict` makes OpenAI constrain the output to the schema. Without it the
    // schema is only advisory, and models occasionally answer with the schema
    // document itself instead of an instance of it.
    let response_format = ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: None,
            name: "structured_response".into(),
            schema: Some(schema_value),
            strict: Some(true),
        },
    };

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .response_format(response_format)
        .build()?;

    let response = client.chat().create(request).await.map_err(readable)?;

    for choice in response.choices {
        if let Some(content) = choice.message.content {
            return Ok(Some(serde_json::from_str::<T>(&content)?));
        }
    }

    Ok(None)
}

/// Rewrite a schemars-generated schema to satisfy OpenAI's strict
/// structured-output requirements: every object schema must list all of its
/// properties as required and forbid additional properties, and the `$schema`
/// marker is not part of the accepted vocabulary.
fn make_schema_strict(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("$schema");
            // Emitted by schemars for `#[serde(default)]` fields; not part of
            // the strict-mode vocabulary.
            map.remove("default");
            if let Some(serde_json::Value::Object(properties)) = map.get("properties") {
                let all_properties = properties
                    .keys()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect();
                map.insert("required".into(), serde_json::Value::Array(all_properties));
                map.insert("additionalProperties".into(), false.into());
            }
            for nested in map.values_mut() {
                make_schema_strict(nested);
            }
        }
        serde_json::Value::Array(items) => {
            for nested in items {
                make_schema_strict(nested);
            }
        }
        _ => {}
    }
}

async fn response(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> anyhow::Result<Option<String>> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .build()?;

    let response = client.chat().create(request).await.map_err(readable)?;

    for choice in response.choices {
        if let Some(content) = choice.message.content {
            return Ok(Some(content));
        }
    }

    Ok(None)
}

async fn stream_response(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<Option<String>> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages.clone())
        .build()?;

    let mut stream = client
        .chat()
        .create_stream(request)
        .await
        .map_err(readable)?;

    let mut response_text: Option<String> = None;

    while let Some(result) = stream.next().await {
        let response =
            (result.map_err(readable)).context("Failed to receive response from OpenAI stream")?;
        if let Some(chat_choice) = response.choices.first()
            && let Some(content) = &chat_choice.delta.content
        {
            if response_text.is_none() {
                response_text = Some(String::new());
            }

            if let Some(text) = response_text.as_mut() {
                text.push_str(content);
            }

            let content_str = content.as_str();
            on_token(content_str);
        }
    }

    Ok(response_text)
}

#[cfg(test)]
mod tests {
    use super::make_schema_strict;
    use serde_json::json;

    #[test]
    fn a_string_error_from_the_proxy_says_its_reason() {
        let body = r#"{"error":"401 Unauthorized"}"#;
        let unread = serde_json::from_str::<async_openai::error::WrappedError>(body)
            .expect_err("async-openai expects an object under `error`");
        let err = super::readable(super::OpenAIError::JSONDeserialize(unread, body.into()));
        assert_eq!(
            err.to_string(),
            "The AI provider answered: 401 Unauthorized",
            "the proxy's reason, not a decoding error"
        );
    }

    #[test]
    fn schemas_are_rewritten_for_strict_mode() {
        let mut schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Response",
            "type": "object",
            "properties": {
                "summary": { "type": ["string", "null"], "default": null },
                "items": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/Item" }
                }
            },
            "required": ["items"],
            "$defs": {
                "Item": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        });
        make_schema_strict(&mut schema);

        assert_eq!(schema.get("$schema"), None);
        assert_eq!(schema["properties"]["summary"].get("default"), None);
        let mut required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        required.sort_unstable();
        assert_eq!(
            required,
            ["items", "summary"],
            "every property must be required, including nullable ones"
        );
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(schema["$defs"]["Item"]["required"], json!(["name"]));
        assert_eq!(
            schema["$defs"]["Item"]["additionalProperties"],
            json!(false)
        );
    }
}
