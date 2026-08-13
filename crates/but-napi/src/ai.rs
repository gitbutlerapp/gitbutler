//! App-level AI configuration and streaming.

use anyhow::{Context as _, Result, bail};
use but_core::git_config::edit_config;
use but_llm::{
    AI_ANTHROPIC_SECRET_HANDLE, AI_OPENAI_SECRET_HANDLE, AI_OPENROUTER_SECRET_HANDLE,
    AiConfiguration as DomainConfiguration, AnthropicConfiguration, ChatMessage,
    CredentialsKeyOption, GITBUTLER_ACCESS_TOKEN_HANDLE, LLMProvider, LLMProviderKind,
    LmStudioConfiguration, OllamaConfiguration, OpenAiConfiguration, clear_ai_configuration,
};
use but_secret::{Sensitive, secret};
use napi::{
    Status,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use napi_derive::napi;

use crate::to_napi_err;

#[derive(Clone)]
#[napi(object)]
pub struct AiConfiguration {
    #[napi(ts_type = "'openai' | 'anthropic' | 'ollama' | 'lmstudio' | 'openrouter'")]
    pub provider: String,
    #[napi(ts_type = "'butlerAPI' | 'bringYourOwn'")]
    pub openai_key_option: String,
    pub openai_model: String,
    pub openai_custom_endpoint: Option<String>,
    pub openai_has_api_key: bool,
    #[napi(ts_type = "'butlerAPI' | 'bringYourOwn'")]
    pub anthropic_key_option: String,
    pub anthropic_model: String,
    pub anthropic_has_api_key: bool,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    pub lmstudio_endpoint: String,
    pub lmstudio_model: String,
    pub is_configured: bool,
}

#[derive(Clone)]
#[napi(object)]
pub struct AiConfigurationUpdate {
    #[napi(ts_type = "'openai' | 'anthropic' | 'ollama' | 'lmstudio'")]
    pub provider: String,
    #[napi(ts_type = "'butlerAPI' | 'bringYourOwn'")]
    pub openai_key_option: String,
    pub openai_model: String,
    pub openai_custom_endpoint: Option<String>,
    pub openai_api_key: Option<String>,
    #[napi(ts_type = "'butlerAPI' | 'bringYourOwn'")]
    pub anthropic_key_option: String,
    pub anthropic_model: String,
    pub anthropic_api_key: Option<String>,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    pub lmstudio_endpoint: String,
    pub lmstudio_model: String,
}

fn has_secret(handle: &str, namespace: secret::Namespace) -> Result<bool> {
    Ok(secret::retrieve(handle, namespace)?.is_some())
}

fn get_configuration() -> Result<AiConfiguration> {
    let config = gix::config::File::from_globals()?;
    let configuration = DomainConfiguration::from_git_config(&config)?;

    let openai_has_api_key = has_secret(AI_OPENAI_SECRET_HANDLE, secret::Namespace::Global)?;
    let anthropic_has_api_key = has_secret(AI_ANTHROPIC_SECRET_HANDLE, secret::Namespace::Global)?;
    let has_gitbutler_token =
        has_secret(GITBUTLER_ACCESS_TOKEN_HANDLE, secret::Namespace::BuildKind)?;
    let is_configured = configuration.is_configured(
        openai_has_api_key,
        anthropic_has_api_key,
        has_gitbutler_token,
    );

    Ok(AiConfiguration {
        provider: configuration.provider.as_git_config_value().into(),
        openai_key_option: configuration.openai.key_option.as_git_config_value().into(),
        openai_model: configuration.openai.model,
        openai_custom_endpoint: configuration.openai.custom_endpoint,
        openai_has_api_key,
        anthropic_key_option: configuration
            .anthropic
            .key_option
            .as_git_config_value()
            .into(),
        anthropic_model: configuration.anthropic.model,
        anthropic_has_api_key,
        ollama_endpoint: configuration.ollama.endpoint,
        ollama_model: configuration.ollama.model,
        lmstudio_endpoint: configuration.lmstudio.endpoint,
        lmstudio_model: configuration.lmstudio.model,
        is_configured,
    })
}

fn provider(value: &str) -> Result<LLMProviderKind> {
    match LLMProviderKind::from_git_config_value(value) {
        Some(
            provider @ (LLMProviderKind::OpenAi
            | LLMProviderKind::Anthropic
            | LLMProviderKind::Ollama
            | LLMProviderKind::LMStudio),
        ) => Ok(provider),
        _ => bail!("Unsupported AI provider '{value}'"),
    }
}

fn key_option(provider: &str, value: &str) -> Result<CredentialsKeyOption> {
    CredentialsKeyOption::from_git_config_value(value)
        .with_context(|| format!("Unsupported {provider} credential source '{value}'"))
}

fn submitted_key(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn validate_update(
    update: &AiConfigurationUpdate,
    openai_has_key: bool,
    anthropic_has_key: bool,
) -> Result<()> {
    let configuration = domain_configuration(update, DomainConfiguration::default())?;

    if configuration.provider == LLMProviderKind::OpenAi
        && configuration.openai.key_option == CredentialsKeyOption::BringYourOwn
        && submitted_key(update.openai_api_key.clone()).is_none()
        && !openai_has_key
    {
        bail!("Enter an OpenAI API key")
    }
    if configuration.provider == LLMProviderKind::Anthropic
        && configuration.anthropic.key_option == CredentialsKeyOption::BringYourOwn
        && submitted_key(update.anthropic_api_key.clone()).is_none()
        && !anthropic_has_key
    {
        bail!("Enter an Anthropic API key")
    }
    Ok(())
}

fn domain_configuration(
    update: &AiConfigurationUpdate,
    mut configuration: DomainConfiguration,
) -> Result<DomainConfiguration> {
    configuration.provider = provider(&update.provider)?;
    configuration.openai = OpenAiConfiguration {
        key_option: key_option("OpenAI", &update.openai_key_option)?,
        model: update.openai_model.clone(),
        custom_endpoint: update.openai_custom_endpoint.clone(),
    };
    configuration.anthropic = AnthropicConfiguration {
        key_option: key_option("Anthropic", &update.anthropic_key_option)?,
        model: update.anthropic_model.clone(),
    };
    configuration.ollama = OllamaConfiguration {
        endpoint: update.ollama_endpoint.clone(),
        model: update.ollama_model.clone(),
    };
    configuration.lmstudio = LmStudioConfiguration {
        endpoint: update.lmstudio_endpoint.clone(),
        model: update.lmstudio_model.clone(),
    };
    configuration.validate()?;
    Ok(configuration)
}

fn update_configuration(update: AiConfigurationUpdate) -> Result<AiConfiguration> {
    let openai_has_key = has_secret(AI_OPENAI_SECRET_HANDLE, secret::Namespace::Global)?;
    let anthropic_has_key = has_secret(AI_ANTHROPIC_SECRET_HANDLE, secret::Namespace::Global)?;
    validate_update(&update, openai_has_key, anthropic_has_key)?;

    if let Some(value) = submitted_key(update.openai_api_key.clone()) {
        secret::persist(
            AI_OPENAI_SECRET_HANDLE,
            &Sensitive(value),
            secret::Namespace::Global,
        )?;
    }
    if let Some(value) = submitted_key(update.anthropic_api_key.clone()) {
        secret::persist(
            AI_ANTHROPIC_SECRET_HANDLE,
            &Sensitive(value),
            secret::Namespace::Global,
        )?;
    }

    let config = gix::config::File::from_globals()?;
    let configuration =
        domain_configuration(&update, DomainConfiguration::from_git_config(&config)?)?;
    edit_config(None, gix::config::Source::User, |config| {
        configuration.apply(config)
    })?;

    get_configuration()
}

/// Read application-global AI configuration without exposing stored secrets.
#[napi]
pub async fn get_ai_configuration() -> napi::Result<AiConfiguration> {
    get_configuration().map_err(to_napi_err)
}

/// Validate and save one complete application-global AI configuration.
#[napi]
pub async fn update_ai_configuration(
    update: AiConfigurationUpdate,
) -> napi::Result<AiConfiguration> {
    update_configuration(update).map_err(to_napi_err)
}

/// Clear application-global AI configuration and stored provider API keys.
#[napi]
pub async fn reset_ai_configuration() -> napi::Result<AiConfiguration> {
    edit_config(None, gix::config::Source::User, clear_ai_configuration).map_err(to_napi_err)?;
    for handle in [
        AI_OPENAI_SECRET_HANDLE,
        AI_ANTHROPIC_SECRET_HANDLE,
        AI_OPENROUTER_SECRET_HANDLE,
    ] {
        secret::delete(handle, secret::Namespace::Global).map_err(to_napi_err)?;
    }
    get_configuration().map_err(to_napi_err)
}

/// Stream a text response from the configured provider.
#[napi]
pub async fn stream_ai_response(
    system_message: String,
    prompt: String,
    on_token: ThreadsafeFunction<String, ()>,
) -> napi::Result<String> {
    let response = tokio::task::spawn_blocking(move || -> Result<String> {
        let config = gix::config::File::from_globals()?;
        let provider = LLMProvider::from_git_config(&config)
            .context("AI provider is not completely configured")?;
        let model = provider.model_or_default();
        let response = provider.stream_response(
            required_message(&system_message, "System message")?,
            vec![ChatMessage::User(
                required_message(&prompt, "Prompt")?.into(),
            )],
            &model,
            move |token| {
                let status = on_token.call(
                    Ok(token.to_string()),
                    ThreadsafeFunctionCallMode::NonBlocking,
                );
                if status != Status::Ok {
                    tracing::warn!(%status, "AI token callback failed");
                }
            },
        )?;
        response
            .filter(|response| !response.trim().is_empty())
            .context("AI provider returned an empty response")
    })
    .await
    .context("AI response task failed")
    .and_then(|result| result)
    .map_err(to_napi_err)?;

    Ok(response)
}

fn required_message<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} is required")
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_llm::{
        AI_LMSTUDIO_MODEL_NAME_KEY, AI_MODEL_PROVIDER_KEY, AI_OPENAI_CUSTOM_ENDPOINT_KEY,
        DEFAULT_ANTHROPIC_MODEL, DEFAULT_LMSTUDIO_ENDPOINT, DEFAULT_LMSTUDIO_MODEL,
        DEFAULT_OLLAMA_ENDPOINT, DEFAULT_OLLAMA_MODEL, DEFAULT_OPENAI_MODEL,
    };

    fn valid_update() -> AiConfigurationUpdate {
        AiConfigurationUpdate {
            provider: "openai".into(),
            openai_key_option: "butlerAPI".into(),
            openai_model: DEFAULT_OPENAI_MODEL.into(),
            openai_custom_endpoint: None,
            openai_api_key: None,
            anthropic_key_option: "butlerAPI".into(),
            anthropic_model: DEFAULT_ANTHROPIC_MODEL.into(),
            anthropic_api_key: None,
            ollama_endpoint: DEFAULT_OLLAMA_ENDPOINT.into(),
            ollama_model: DEFAULT_OLLAMA_MODEL.into(),
            lmstudio_endpoint: DEFAULT_LMSTUDIO_ENDPOINT.into(),
            lmstudio_model: DEFAULT_LMSTUDIO_MODEL.into(),
        }
    }

    #[test]
    fn validates_provider_and_endpoints() {
        let mut update = valid_update();
        update.provider = "openrouter".into();
        assert!(
            validate_update(&update, false, false).is_err(),
            "unsupported providers must fail"
        );

        update = valid_update();
        update.ollama_endpoint = "http://localhost:11434".into();
        assert!(
            validate_update(&update, false, false).is_err(),
            "Ollama requires host:port"
        );
    }

    #[test]
    fn preserves_custom_models_and_write_only_keys() {
        let mut update = valid_update();
        update.openai_model = "my-custom-model".into();
        update.openai_key_option = "bringYourOwn".into();
        assert!(
            validate_update(&update, true, false).is_ok(),
            "a stored key may remain write-only"
        );
        assert_eq!(
            domain_configuration(&update, DomainConfiguration::default())
                .unwrap()
                .openai
                .model,
            "my-custom-model",
            "custom model names must survive the transport conversion"
        );

        assert!(
            validate_update(&update, false, false).is_err(),
            "BYOK requires a stored or submitted key"
        );
        update.openai_api_key = Some("new-key".into());
        assert!(
            validate_update(&update, false, false).is_ok(),
            "a submitted BYOK key is sufficient"
        );
    }

    #[test]
    fn maps_the_complete_update_to_shared_git_keys() {
        let mut update = valid_update();
        update.provider = "lmstudio".into();
        update.openai_custom_endpoint = Some("https://example.com/v1".into());
        let mut config =
            gix::config::File::new(gix::config::file::Metadata::from(gix::config::Source::User));

        domain_configuration(&update, DomainConfiguration::default())
            .unwrap()
            .apply(&mut config)
            .unwrap();

        assert_eq!(
            config.string(AI_MODEL_PROVIDER_KEY).unwrap(),
            "lmstudio",
            "the active provider uses the shared AI key"
        );
        assert_eq!(
            config.string(AI_OPENAI_CUSTOM_ENDPOINT_KEY).unwrap(),
            "https://example.com/v1",
            "provider settings retain their shared desktop and CLI keys"
        );
        assert_eq!(
            config.string(AI_LMSTUDIO_MODEL_NAME_KEY).unwrap(),
            DEFAULT_LMSTUDIO_MODEL,
            "the local model is persisted alongside the provider"
        );
    }
}
