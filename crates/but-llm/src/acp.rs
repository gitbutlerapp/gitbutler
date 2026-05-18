use anyhow::{Context as _, Result};
use but_acp::AcpCommandConfig;
use but_tools::tool::Toolset;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    AI_ACP_AGENT_ID_KEY, AI_ACP_ARGS_KEY, AI_ACP_COMMAND_KEY, AI_ACP_ENV_KEY,
    AI_ACP_MODEL_NAME_KEY, chat::ChatMessage, client::LLMClient,
};

#[derive(Debug, Clone)]
pub struct AcpProvider {
    config: AcpCommandConfig,
    model: Option<String>,
}

impl AcpProvider {
    pub fn new(config: AcpCommandConfig, model: Option<String>) -> Self {
        Self { config, model }
    }

    fn prompt(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        on_token: Option<impl Fn(&str) + Send + Sync + 'static>,
    ) -> Result<String> {
        let prompt = render_prompt(system_message, &chat_messages);
        let cwd = std::env::current_dir().context("failed to determine ACP working directory")?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to create ACP runtime")?;
        let response = runtime.block_on(but_acp::prompt_once(self.config.clone(), cwd, prompt))?;
        if let Some(on_token) = on_token {
            on_token(&response);
        }
        Ok(response)
    }
}

impl LLMClient for AcpProvider {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self>
    where
        Self: Sized,
    {
        let command = config.string(AI_ACP_COMMAND_KEY).map(|v| v.to_string())?;
        let id = config
            .string(AI_ACP_AGENT_ID_KEY)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "configured".to_string());
        let args = config
            .string(AI_ACP_ARGS_KEY)
            .and_then(|value| serde_json::from_str::<Vec<String>>(&value.to_string()).ok())
            .unwrap_or_default();
        let env = config
            .string(AI_ACP_ENV_KEY)
            .and_then(|value| {
                serde_json::from_str::<std::collections::BTreeMap<String, String>>(
                    &value.to_string(),
                )
                .ok()
            })
            .unwrap_or_default();
        let model = config.string(AI_ACP_MODEL_NAME_KEY).map(|v| v.to_string());
        Some(Self::new(
            AcpCommandConfig {
                id,
                name: "Configured ACP Agent".to_string(),
                command,
                args,
                env,
            },
            model,
        ))
    }

    fn model(&self) -> Option<String> {
        self.model.clone()
    }

    fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        _tool_set: &mut impl Toolset,
        _model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<(String, Vec<ChatMessage>)> {
        let response = self.prompt(system_message, chat_messages.clone(), Some(on_token))?;
        Ok((response, chat_messages))
    }

    fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        _tool_set: &mut impl Toolset,
        _model: &str,
    ) -> Result<String> {
        self.prompt(system_message, chat_messages, None::<fn(&str)>)
    }

    fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        _model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<Option<String>> {
        self.prompt(system_message, chat_messages, Some(on_token))
            .map(Some)
    }

    fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        _model: &str,
    ) -> Result<Option<String>> {
        self.prompt(system_message, chat_messages, None::<fn(&str)>)
            .map(Some)
    }

    fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        _model: &str,
    ) -> Result<Option<T>> {
        let schema = serde_json::to_string_pretty(&schemars::schema_for!(T))
            .context("failed to serialize ACP JSON schema")?;
        let prompt = format!(
            "{system_message}\n\nReturn only JSON matching this schema. Do not include markdown fences.\n\nSchema:\n{schema}"
        );
        let response = self.prompt(&prompt, chat_messages, None::<fn(&str)>)?;
        Ok(Some(parse_json_response(&response)?))
    }
}

fn render_prompt(system_message: &str, chat_messages: &[ChatMessage]) -> String {
    let mut prompt = String::new();
    if !system_message.is_empty() {
        prompt.push_str("<system>\n");
        prompt.push_str(system_message);
        prompt.push_str("\n</system>\n\n");
    }
    for message in chat_messages {
        prompt.push_str(&message.to_string());
        prompt.push_str("\n\n");
    }
    prompt
}

fn parse_json_response<T: DeserializeOwned>(response: &str) -> Result<T> {
    let trimmed = response.trim();
    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    serde_json::from_str(stripped).context("ACP response did not match the requested JSON schema")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Output {
        value: String,
    }

    #[test]
    fn parses_plain_and_fenced_json() {
        assert_eq!(
            parse_json_response::<Output>(r#"{"value":"ok"}"#).unwrap(),
            Output {
                value: "ok".to_string()
            }
        );
        assert_eq!(
            parse_json_response::<Output>("```json\n{\"value\":\"ok\"}\n```").unwrap(),
            Output {
                value: "ok".to_string()
            }
        );
    }
}
