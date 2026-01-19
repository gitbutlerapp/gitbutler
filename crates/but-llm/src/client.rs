use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use but_tools::tool::Toolset;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::ChatMessage;

pub trait LLMClient: Debug + Clone {
    fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: String,
        on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
    ) -> Result<(String, Vec<ChatMessage>)>;

    fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: String,
    ) -> Result<String>;

    fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: String,
        on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
    ) -> Result<Option<String>>;

    fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: String,
    ) -> Result<Option<String>>;

    fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: String,
    ) -> Result<Option<T>>;
}
