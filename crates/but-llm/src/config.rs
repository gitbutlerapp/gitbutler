use anyhow::{Context as _, Result, bail};
use but_core::git_config::{remove_config_value, set_config_value};

use crate::{CredentialsKeyOption, LLMProviderKind};

pub const AI_MODEL_PROVIDER_KEY: &str = "gitbutler.aiModelProvider";
pub const AI_OPENAI_KEY_OPTION_KEY: &str = "gitbutler.aiOpenAIKeyOption";
pub const AI_OPENAI_MODEL_NAME_KEY: &str = "gitbutler.aiOpenAIModelName";
pub const AI_OPENAI_CUSTOM_ENDPOINT_KEY: &str = "gitbutler.aiOpenAICustomEndpoint";
pub const AI_ANTHROPIC_KEY_OPTION_KEY: &str = "gitbutler.aiAnthropicKeyOption";
pub const AI_ANTHROPIC_MODEL_NAME_KEY: &str = "gitbutler.aiAnthropicModelName";
pub const AI_OLLAMA_ENDPOINT_KEY: &str = "gitbutler.aiOllamaEndpoint";
pub const AI_OLLAMA_MODEL_NAME_KEY: &str = "gitbutler.aiOllamaModelName";
pub const AI_LMSTUDIO_ENDPOINT_KEY: &str = "gitbutler.aiLMStudioEndpoint";
pub const AI_LMSTUDIO_MODEL_NAME_KEY: &str = "gitbutler.aiLMStudioModelName";
pub const AI_OPENROUTER_MODEL_NAME_KEY: &str = "gitbutler.aiOpenRouterModelName";
pub const AI_OPENROUTER_ENDPOINT_KEY: &str = "gitbutler.aiOpenRouterEndpoint";

pub const AI_OPENAI_SECRET_HANDLE: &str = "aiOpenAIKey";
pub const AI_ANTHROPIC_SECRET_HANDLE: &str = "aiAnthropicKey";
pub const AI_OPENROUTER_SECRET_HANDLE: &str = "aiOpenRouterKey";

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.4-nano";
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-haiku-4-5";
pub const DEFAULT_OLLAMA_ENDPOINT: &str = "127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3";
pub const DEFAULT_LMSTUDIO_ENDPOINT: &str = "http://127.0.0.1:1234/v1";
pub const DEFAULT_LMSTUDIO_MODEL: &str = "default";
pub const DEFAULT_OPENROUTER_MODEL: &str = "openai/gpt-4.1-mini";

const AI_CONFIG_KEYS: &[&str] = &[
    AI_MODEL_PROVIDER_KEY,
    AI_OPENAI_KEY_OPTION_KEY,
    AI_OPENAI_MODEL_NAME_KEY,
    AI_OPENAI_CUSTOM_ENDPOINT_KEY,
    AI_ANTHROPIC_KEY_OPTION_KEY,
    AI_ANTHROPIC_MODEL_NAME_KEY,
    AI_OLLAMA_ENDPOINT_KEY,
    AI_OLLAMA_MODEL_NAME_KEY,
    AI_LMSTUDIO_ENDPOINT_KEY,
    AI_LMSTUDIO_MODEL_NAME_KEY,
    AI_OPENROUTER_MODEL_NAME_KEY,
    AI_OPENROUTER_ENDPOINT_KEY,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiConfiguration {
    pub key_option: CredentialsKeyOption,
    pub model: String,
    pub custom_endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicConfiguration {
    pub key_option: CredentialsKeyOption,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaConfiguration {
    pub endpoint: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmStudioConfiguration {
    pub endpoint: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConfiguration {
    pub provider: LLMProviderKind,
    pub openai: OpenAiConfiguration,
    pub anthropic: AnthropicConfiguration,
    pub ollama: OllamaConfiguration,
    pub lmstudio: LmStudioConfiguration,
}

impl Default for AiConfiguration {
    fn default() -> Self {
        Self {
            provider: LLMProviderKind::OpenAi,
            openai: OpenAiConfiguration {
                key_option: CredentialsKeyOption::ButlerApi,
                model: DEFAULT_OPENAI_MODEL.into(),
                custom_endpoint: None,
            },
            anthropic: AnthropicConfiguration {
                key_option: CredentialsKeyOption::ButlerApi,
                model: DEFAULT_ANTHROPIC_MODEL.into(),
            },
            ollama: OllamaConfiguration {
                endpoint: DEFAULT_OLLAMA_ENDPOINT.into(),
                model: DEFAULT_OLLAMA_MODEL.into(),
            },
            lmstudio: LmStudioConfiguration {
                endpoint: DEFAULT_LMSTUDIO_ENDPOINT.into(),
                model: DEFAULT_LMSTUDIO_MODEL.into(),
            },
        }
    }
}

impl AiConfiguration {
    pub fn from_git_config(config: &gix::config::File) -> Result<Self> {
        let values = AiConfigurationSnapshot::from_git_config(config);
        let defaults = Self::default();
        let provider = match values.provider.as_deref() {
            Some(value) => LLMProviderKind::from_git_config_value(value)
                .with_context(|| format!("Unsupported AI provider '{value}'"))?,
            None => defaults.provider,
        };
        let openai_key_option = match values.openai_key_option.as_deref() {
            Some(value) => CredentialsKeyOption::from_git_config_value(value)
                .with_context(|| format!("Unsupported OpenAI credential source '{value}'"))?,
            None => defaults.openai.key_option,
        };
        let anthropic_key_option = match values.anthropic_key_option.as_deref() {
            Some(value) => CredentialsKeyOption::from_git_config_value(value)
                .with_context(|| format!("Unsupported Anthropic credential source '{value}'"))?,
            None => defaults.anthropic.key_option,
        };

        let configuration = Self {
            provider,
            openai: OpenAiConfiguration {
                key_option: openai_key_option,
                model: values.openai_model.unwrap_or(defaults.openai.model),
                custom_endpoint: values.openai_endpoint,
            },
            anthropic: AnthropicConfiguration {
                key_option: anthropic_key_option,
                model: values.anthropic_model.unwrap_or(defaults.anthropic.model),
            },
            ollama: OllamaConfiguration {
                endpoint: values.ollama_endpoint.unwrap_or(defaults.ollama.endpoint),
                model: values.ollama_model.unwrap_or(defaults.ollama.model),
            },
            lmstudio: LmStudioConfiguration {
                endpoint: values
                    .lmstudio_endpoint
                    .unwrap_or(defaults.lmstudio.endpoint),
                model: values.lmstudio_model.unwrap_or(defaults.lmstudio.model),
            },
        };
        Ok(configuration)
    }

    pub fn validate(&self) -> Result<()> {
        required(&self.openai.model, "OpenAI model")?;
        required(&self.anthropic.model, "Anthropic model")?;
        required(&self.ollama.model, "Ollama model")?;
        required(&self.lmstudio.model, "LM Studio model")?;
        validate_ollama_endpoint(&self.ollama.endpoint)?;
        validate_url(&self.lmstudio.endpoint, "LM Studio endpoint")?;
        if self.openai.key_option == CredentialsKeyOption::BringYourOwn
            && let Some(endpoint) = self
                .openai
                .custom_endpoint
                .as_deref()
                .filter(|endpoint| !endpoint.trim().is_empty())
        {
            validate_url(endpoint, "OpenAI custom endpoint")?;
        }
        Ok(())
    }

    pub fn validate_active(&self) -> Result<()> {
        match self.provider {
            LLMProviderKind::OpenAi => {
                required(&self.openai.model, "OpenAI model")?;
                if self.openai.key_option == CredentialsKeyOption::BringYourOwn
                    && let Some(endpoint) = nonempty(self.openai.custom_endpoint.as_deref())
                {
                    validate_url(endpoint, "OpenAI custom endpoint")?;
                }
            }
            LLMProviderKind::Anthropic => {
                required(&self.anthropic.model, "Anthropic model")?;
            }
            LLMProviderKind::Ollama => {
                required(&self.ollama.model, "Ollama model")?;
                validate_ollama_endpoint(&self.ollama.endpoint)?;
            }
            LLMProviderKind::LMStudio => {
                required(&self.lmstudio.model, "LM Studio model")?;
                validate_url(&self.lmstudio.endpoint, "LM Studio endpoint")?;
            }
            LLMProviderKind::OpenRouter => {}
        }
        Ok(())
    }

    pub fn apply(&self, config: &mut gix::config::File) -> Result<()> {
        self.validate()?;
        set_config_value(
            config,
            AI_MODEL_PROVIDER_KEY,
            self.provider.as_git_config_value(),
        )?;
        set_config_value(
            config,
            AI_OPENAI_KEY_OPTION_KEY,
            self.openai.key_option.as_git_config_value(),
        )?;
        set_config_value(config, AI_OPENAI_MODEL_NAME_KEY, self.openai.model.trim())?;
        set_optional(
            config,
            AI_OPENAI_CUSTOM_ENDPOINT_KEY,
            self.openai.custom_endpoint.as_deref(),
        )?;
        set_config_value(
            config,
            AI_ANTHROPIC_KEY_OPTION_KEY,
            self.anthropic.key_option.as_git_config_value(),
        )?;
        set_config_value(
            config,
            AI_ANTHROPIC_MODEL_NAME_KEY,
            self.anthropic.model.trim(),
        )?;
        set_config_value(config, AI_OLLAMA_ENDPOINT_KEY, self.ollama.endpoint.trim())?;
        set_config_value(config, AI_OLLAMA_MODEL_NAME_KEY, self.ollama.model.trim())?;
        set_config_value(
            config,
            AI_LMSTUDIO_ENDPOINT_KEY,
            self.lmstudio.endpoint.trim(),
        )?;
        set_config_value(
            config,
            AI_LMSTUDIO_MODEL_NAME_KEY,
            self.lmstudio.model.trim(),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AiConfigurationSnapshot {
    pub provider: Option<String>,
    pub openai_key_option: Option<String>,
    pub openai_model: Option<String>,
    pub openai_endpoint: Option<String>,
    pub anthropic_key_option: Option<String>,
    pub anthropic_model: Option<String>,
    pub ollama_endpoint: Option<String>,
    pub ollama_model: Option<String>,
    pub lmstudio_endpoint: Option<String>,
    pub lmstudio_model: Option<String>,
}

impl AiConfigurationSnapshot {
    pub fn from_git_config(config: &gix::config::File) -> Self {
        Self {
            provider: config_value(config, AI_MODEL_PROVIDER_KEY),
            openai_key_option: config_value(config, AI_OPENAI_KEY_OPTION_KEY),
            openai_model: config_value(config, AI_OPENAI_MODEL_NAME_KEY),
            openai_endpoint: config_value(config, AI_OPENAI_CUSTOM_ENDPOINT_KEY),
            anthropic_key_option: config_value(config, AI_ANTHROPIC_KEY_OPTION_KEY),
            anthropic_model: config_value(config, AI_ANTHROPIC_MODEL_NAME_KEY),
            ollama_endpoint: config_value(config, AI_OLLAMA_ENDPOINT_KEY),
            ollama_model: config_value(config, AI_OLLAMA_MODEL_NAME_KEY),
            lmstudio_endpoint: config_value(config, AI_LMSTUDIO_ENDPOINT_KEY),
            lmstudio_model: config_value(config, AI_LMSTUDIO_MODEL_NAME_KEY),
        }
    }
}

pub fn apply_openai_configuration(
    config: &mut gix::config::File,
    key_option: CredentialsKeyOption,
    model: Option<&str>,
    endpoint: Option<&str>,
) -> Result<()> {
    validate_optional(model, "OpenAI model")?;
    if let Some(endpoint) = nonempty(endpoint) {
        validate_url(endpoint, "OpenAI custom endpoint")?;
    }
    set_provider(config, LLMProviderKind::OpenAi)?;
    set_config_value(
        config,
        AI_OPENAI_KEY_OPTION_KEY,
        key_option.as_git_config_value(),
    )?;
    set_optional(config, AI_OPENAI_MODEL_NAME_KEY, model)?;
    set_optional(config, AI_OPENAI_CUSTOM_ENDPOINT_KEY, endpoint)
}

pub fn apply_anthropic_configuration(
    config: &mut gix::config::File,
    key_option: CredentialsKeyOption,
    model: Option<&str>,
) -> Result<()> {
    validate_optional(model, "Anthropic model")?;
    set_provider(config, LLMProviderKind::Anthropic)?;
    set_config_value(
        config,
        AI_ANTHROPIC_KEY_OPTION_KEY,
        key_option.as_git_config_value(),
    )?;
    set_optional(config, AI_ANTHROPIC_MODEL_NAME_KEY, model)
}

pub fn apply_ollama_configuration(
    config: &mut gix::config::File,
    endpoint: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    if let Some(endpoint) = nonempty(endpoint) {
        validate_ollama_endpoint(endpoint)?;
    }
    validate_optional(model, "Ollama model")?;
    set_provider(config, LLMProviderKind::Ollama)?;
    set_optional(config, AI_OLLAMA_ENDPOINT_KEY, endpoint)?;
    set_optional(config, AI_OLLAMA_MODEL_NAME_KEY, model)
}

pub fn apply_lmstudio_configuration(
    config: &mut gix::config::File,
    endpoint: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    if let Some(endpoint) = nonempty(endpoint) {
        validate_url(endpoint, "LM Studio endpoint")?;
    }
    validate_optional(model, "LM Studio model")?;
    set_provider(config, LLMProviderKind::LMStudio)?;
    set_optional(config, AI_LMSTUDIO_ENDPOINT_KEY, endpoint)?;
    set_optional(config, AI_LMSTUDIO_MODEL_NAME_KEY, model)
}

pub fn apply_openrouter_configuration(
    config: &mut gix::config::File,
    model: Option<&str>,
) -> Result<()> {
    validate_optional(model, "OpenRouter model")?;
    set_provider(config, LLMProviderKind::OpenRouter)?;
    set_optional(config, AI_OPENROUTER_MODEL_NAME_KEY, model)
}

pub fn clear_ai_configuration(config: &mut gix::config::File) -> Result<()> {
    for key in AI_CONFIG_KEYS {
        remove_config_value(config, key)?;
    }
    Ok(())
}

fn set_provider(config: &mut gix::config::File, provider: LLMProviderKind) -> Result<()> {
    set_config_value(
        config,
        AI_MODEL_PROVIDER_KEY,
        provider.as_git_config_value(),
    )
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn validate_optional(value: Option<&str>, field: &str) -> Result<()> {
    if value.is_some() && nonempty(value).is_none() {
        bail!("{field} is required when provided")
    }
    Ok(())
}

fn config_value(config: &gix::config::File, key: &str) -> Option<String> {
    config.string(key).map(|value| value.to_string())
}

fn required<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} is required")
    }
    Ok(value)
}

fn validate_url(value: &str, field: &str) -> Result<()> {
    let value = required(value, field)?;
    let url = url::Url::parse(value).with_context(|| format!("{field} must be a valid URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("{field} must be an HTTP or HTTPS URL")
    }
    Ok(())
}

fn validate_ollama_endpoint(value: &str) -> Result<()> {
    let value = required(value, "Ollama endpoint")?;
    if value.contains("://") {
        bail!("Ollama endpoint must use host:port format")
    }
    let (host, port) = value
        .rsplit_once(':')
        .context("Ollama endpoint must use host:port format")?;
    if host.trim().is_empty() || port.parse::<u16>().is_err() {
        bail!("Ollama endpoint must use host:port format")
    }
    Ok(())
}

fn set_optional(config: &mut gix::config::File, key: &str, value: Option<&str>) -> Result<()> {
    match nonempty(value) {
        Some(value) => set_config_value(config, key, value),
        None => remove_config_value(config, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_roundtrip_through_git_config() {
        let expected = AiConfiguration::default();
        let mut config =
            gix::config::File::new(gix::config::file::Metadata::from(gix::config::Source::User));

        expected.apply(&mut config).unwrap();

        assert_eq!(
            AiConfiguration::from_git_config(&config).unwrap(),
            expected,
            "one shared model must own config keys and defaults"
        );
    }

    #[test]
    fn clear_removes_all_ai_keys() {
        let mut config =
            gix::config::File::new(gix::config::file::Metadata::from(gix::config::Source::User));
        AiConfiguration::default().apply(&mut config).unwrap();
        set_config_value(&mut config, AI_OPENROUTER_MODEL_NAME_KEY, "model").unwrap();
        set_config_value(
            &mut config,
            AI_OPENROUTER_ENDPOINT_KEY,
            "https://example.com",
        )
        .unwrap();

        clear_ai_configuration(&mut config).unwrap();

        assert!(
            AI_CONFIG_KEYS
                .iter()
                .all(|key| config.string(key).is_none()),
            "reset must remove every shared AI configuration key"
        );
    }

    #[test]
    fn custom_openai_endpoint_only_applies_to_own_keys() {
        let mut configuration = AiConfiguration::default();
        configuration.openai.custom_endpoint = Some("not a URL".into());

        assert!(
            configuration.validate().is_ok(),
            "the GitButler proxy ignores custom endpoints"
        );

        configuration.openai.key_option = CredentialsKeyOption::BringYourOwn;
        assert!(
            configuration.validate().is_err(),
            "own-key endpoints must remain valid URLs"
        );
    }
}
