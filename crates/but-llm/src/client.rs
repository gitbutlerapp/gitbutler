use std::{fmt::Debug, time::Duration};

use anyhow::Result;
use but_tools::tool::Toolset;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::ChatMessage;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) trait GitConfigReader {
    fn string_value(&self, key: &str) -> Option<String>;
}

impl GitConfigReader for gix::config::File<'_> {
    fn string_value(&self, key: &str) -> Option<String> {
        self.string(key).map(|value| value.to_string())
    }
}

impl GitConfigReader for gix::config::Snapshot<'_> {
    fn string_value(&self, key: &str) -> Option<String> {
        self.string(key).map(|value| value.to_string())
    }
}

/// Creates an HTTP client builder with a bounded connection timeout.
pub(crate) fn http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder().connect_timeout(DEFAULT_CONNECT_TIMEOUT)
}

pub trait LLMClient: Debug + Clone {
    fn from_git_config(config: &impl GitConfigReader) -> Option<Self>
    where
        Self: Sized;

    fn model(&self) -> Option<String>;

    fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<(String, Vec<ChatMessage>)>;

    fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: &str,
    ) -> Result<String>;

    fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<Option<String>>;

    fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<String>>;

    fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<T>>;
}
