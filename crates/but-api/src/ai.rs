//! App-level AI configuration.
//!
//! These are app-scoped rather than project-scoped, so they take no `Context`.
//! They live here, and not in `but-napi` beside the streaming call, so the
//! `#[but_api]` macro generates their wrappers: clients then reach them
//! through the same generated endpoint table as every other call.

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use but_core::git_config::edit_config;
use but_llm::{
    AI_ANTHROPIC_SECRET_HANDLE, AI_OPENAI_SECRET_HANDLE, AI_OPENROUTER_SECRET_HANDLE,
    AiConfiguration as DomainConfiguration, AnthropicConfiguration, CredentialsKeyOption,
    GITBUTLER_ACCESS_TOKEN_HANDLE, LLMProviderKind, LmStudioConfiguration, OllamaConfiguration,
    OpenAiConfiguration, clear_ai_configuration,
};
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

/// The AI configuration as clients see it, with secrets reduced to whether
/// they are present.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "napi", napi_derive::napi(object))]
pub struct AiConfiguration {
    /// The provider requests go to.
    #[cfg_attr(
        feature = "napi",
        napi(ts_type = "'openai' | 'anthropic' | 'ollama' | 'lmstudio' | 'openrouter'")
    )]
    pub provider: String,
    /// Whether OpenAI calls use GitButler's key or the user's own.
    #[cfg_attr(feature = "napi", napi(ts_type = "'butlerAPI' | 'bringYourOwn'"))]
    pub openai_key_option: String,
    /// The OpenAI model to request.
    pub openai_model: String,
    /// An OpenAI-compatible endpoint to use instead of OpenAI's own.
    pub openai_custom_endpoint: Option<String>,
    /// Whether an OpenAI key is stored, never the key itself.
    pub openai_has_api_key: bool,
    /// Whether Anthropic calls use GitButler's key or the user's own.
    #[cfg_attr(feature = "napi", napi(ts_type = "'butlerAPI' | 'bringYourOwn'"))]
    pub anthropic_key_option: String,
    /// The Anthropic model to request.
    pub anthropic_model: String,
    /// Whether an Anthropic key is stored, never the key itself.
    pub anthropic_has_api_key: bool,
    /// Where the local Ollama server listens.
    pub ollama_endpoint: String,
    /// The Ollama model to request.
    pub ollama_model: String,
    /// Where the local LM Studio server listens.
    pub lmstudio_endpoint: String,
    /// The LM Studio model to request.
    pub lmstudio_model: String,
    /// Whether the active provider has everything it needs to answer.
    pub is_configured: bool,
}

/// One complete AI configuration to save, with any newly entered API keys.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "napi", napi_derive::napi(object))]
pub struct AiConfigurationUpdate {
    /// The provider requests should go to. Streaming-only providers cannot be chosen here.
    #[cfg_attr(
        feature = "napi",
        napi(ts_type = "'openai' | 'anthropic' | 'ollama' | 'lmstudio'")
    )]
    pub provider: String,
    /// Whether OpenAI calls should use GitButler's key or the user's own.
    #[cfg_attr(feature = "napi", napi(ts_type = "'butlerAPI' | 'bringYourOwn'"))]
    pub openai_key_option: String,
    /// The OpenAI model to request.
    pub openai_model: String,
    /// An OpenAI-compatible endpoint to use instead of OpenAI's own.
    pub openai_custom_endpoint: Option<String>,
    /// A newly entered OpenAI key to store; omitted leaves any stored key alone.
    pub openai_api_key: Option<String>,
    /// Whether Anthropic calls should use GitButler's key or the user's own.
    #[cfg_attr(feature = "napi", napi(ts_type = "'butlerAPI' | 'bringYourOwn'"))]
    pub anthropic_key_option: String,
    /// The Anthropic model to request.
    pub anthropic_model: String,
    /// A newly entered Anthropic key to store; omitted leaves any stored key alone.
    pub anthropic_api_key: Option<String>,
    /// Where the local Ollama server listens.
    pub ollama_endpoint: String,
    /// The Ollama model to request.
    pub ollama_model: String,
    /// Where the local LM Studio server listens.
    pub lmstudio_endpoint: String,
    /// The LM Studio model to request.
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

/// Read application-global AI configuration without exposing stored secrets.
#[but_api(napi, provides = [AiConfiguration])]
pub fn get_ai_configuration() -> Result<AiConfiguration> {
    get_configuration()
}

/// Validate and save one complete application-global AI configuration.
#[but_api(napi, invalidates = [AiConfiguration])]
pub fn update_ai_configuration(update: AiConfigurationUpdate) -> Result<AiConfiguration> {
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

/// Clear application-global AI configuration and stored provider API keys.
#[but_api(napi, invalidates = [AiConfiguration])]
pub fn reset_ai_configuration() -> Result<AiConfiguration> {
    edit_config(None, gix::config::Source::User, clear_ai_configuration)?;
    for handle in [
        AI_OPENAI_SECRET_HANDLE,
        AI_ANTHROPIC_SECRET_HANDLE,
        AI_OPENROUTER_SECRET_HANDLE,
    ] {
        secret::delete(handle, secret::Namespace::Global)?;
    }
    get_configuration()
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
