use std::collections::HashMap;
use std::io::{Write, stdout};

use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolType,
    FunctionObjectArgs,
};
use async_openai::{Client, types::CreateChatCompletionRequestArgs};
use futures::StreamExt;
// use rand::seq::SliceRandom;
// use rand::{Rng, thread_rng};
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let user_prompt = "What's the weather like in Boston and Atlanta?";

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u32)
        .model("gpt-4-1106-preview")
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(user_prompt)
            .build()?
            .into()])
        .tools(vec![
            ChatCompletionToolArgs::default()
                .r#type(ChatCompletionToolType::Function)
                .function(
                    FunctionObjectArgs::default()
                        .name("get_current_weather")
                        .description("Get the current weather in a given location")
                        .parameters(json!({
                            "type": "object",
                            "properties": {
                                "location": {
                                    "type": "string",
                                    "description": "The city and state, e.g. San Francisco, CA",
                                },
                                "unit": { "type": "string", "enum": ["celsius", "fahrenheit"] },
                            },
                            "required": ["location"],
                        }))
                        .build()?,
                )
                .build()?,
        ])
        .build()?;

    let response_message = client
        .chat()
        .create(request)
        .await?
        .choices
        .first()
        .unwrap()
        .message
        .clone();

    if let Some(tool_calls) = response_message.tool_calls {
        let mut handles = Vec::new();
        for tool_call in tool_calls {
            let name = tool_call.function.name.clone();
            let args = tool_call.function.arguments.clone();
            let tool_call_clone = tool_call.clone();

            let handle =
                tokio::spawn(async move { call_fn(&name, &args).await.unwrap_or_default() });
            handles.push((handle, tool_call_clone));
        }

        let mut function_responses = Vec::new();

        for (handle, tool_call_clone) in handles {
            if let Ok(response_content) = handle.await {
                function_responses.push((tool_call_clone, response_content));
            }
        }

        let mut messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()?
                .into(),
        ];

        let tool_calls: Vec<ChatCompletionMessageToolCall> = function_responses
            .iter()
            .map(|(tool_call, _response_content)| tool_call.clone())
            .collect();

        let assistant_messages: ChatCompletionRequestMessage =
            ChatCompletionRequestAssistantMessageArgs::default()
                .tool_calls(tool_calls)
                .build()?
                .into();

        let tool_messages: Vec<ChatCompletionRequestMessage> = function_responses
            .iter()
            .map(|(tool_call, response_content)| {
                ChatCompletionRequestToolMessageArgs::default()
                    .content(response_content.to_string())
                    .tool_call_id(tool_call.id.clone())
                    .build()
                    .unwrap()
                    .into()
            })
            .collect();

        messages.push(assistant_messages);
        messages.extend(tool_messages);

        let subsequent_request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u32)
            .model("gpt-4-1106-preview")
            .messages(messages)
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        let mut stream = client.chat().create_stream(subsequent_request).await?;

        let mut response_content = String::new();
        let mut lock = stdout().lock();
        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    for chat_choice in response.choices.iter() {
                        if let Some(ref content) = chat_choice.delta.content {
                            write!(lock, "{}", content).unwrap();
                            response_content.push_str(content);
                        }
                    }
                }
                Err(err) => {
                    return Err(Box::new(err) as Box<dyn std::error::Error>);
                }
            }
        }
    }

    Ok(())
}

async fn call_fn(name: &str, args: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut available_functions: HashMap<&str, fn(&str, &str) -> serde_json::Value> =
        HashMap::new();
    available_functions.insert("get_current_weather", get_current_weather);

    let function_args: serde_json::Value = args.parse().unwrap();

    let location = function_args["location"].as_str().unwrap();
    let unit = function_args["unit"].as_str().unwrap_or("fahrenheit");
    let function = available_functions.get(name).unwrap();
    let function_response = function(location, unit);
    Ok(function_response)
}

fn get_current_weather(location: &str, unit: &str) -> serde_json::Value {
    // let mut rng = thread_rng();

    // let temperature: i32 = rng.gen_range(20..=55);

    // let forecasts = [
    //     "sunny", "cloudy", "overcast", "rainy", "windy", "foggy", "snowy",
    // ];

    // let forecast = forecasts.choose(&mut rng).unwrap_or(&"sunny");

    let weather_info = json!({
        "location": location,
        "temperature": "123".to_string(),
        "unit": unit,
        "forecast": "sunny"
    });

    weather_info
}
