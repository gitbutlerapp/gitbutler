//! The AI section: which provider writes commit messages and pull request descriptions,
//! with whose key, and which model. GitButler's other apps share it. It's read only when
//! shown, as reading it checks the keychain for stored keys.

use gpui::{Context, Entity, SharedString, Window, prelude::*, px};

use super::{Row, SettingsView, field_with_save};
use crate::backend::run;
use crate::backend::settings::{self as backend, AiConfiguration, AiConfigurationUpdate};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{button, h_flex};

const PROVIDERS: [(&str, &str); 4] = [
    ("openai", "OpenAI"),
    ("anthropic", "Anthropic"),
    ("ollama", "Ollama"),
    ("lmstudio", "LM Studio"),
];

const GITBUTLERS_KEY: &str = "butlerAPI";
const OWN_KEY: &str = "bringYourOwn";

pub(super) struct Ai {
    config: Option<AiConfiguration>,
    loading: bool,
    /// "Your own" is picked but no key is stored yet; it's saved with the key, as a key
    /// option without a key can't be.
    own_pending: bool,
    key: Entity<TextInput>,
    model: Entity<TextInput>,
    endpoint: Entity<TextInput>,
}

impl Ai {
    pub(super) fn new(cx: &mut Context<SettingsView>) -> Self {
        let key = cx.new(|cx| TextInput::new("Paste a key", false, cx).masked());
        let model = cx.new(|cx| TextInput::new("Model", false, cx));
        let endpoint = cx.new(|cx| TextInput::new("http://localhost:11434", false, cx));
        for input in [&key, &model, &endpoint] {
            cx.subscribe(input, |this, input, event, cx| match event {
                InputEvent::Submit => this.save_ai_field(&input, cx),
                InputEvent::Changed => cx.notify(),
                InputEvent::Cancel => this.sync_ai_fields(cx),
            })
            .detach();
        }
        Self {
            config: None,
            loading: false,
            own_pending: false,
            key,
            model,
            endpoint,
        }
    }
}

fn update_from(config: &AiConfiguration) -> AiConfigurationUpdate {
    AiConfigurationUpdate {
        provider: config.provider.clone(),
        openai_key_option: config.openai_key_option.clone(),
        openai_model: config.openai_model.clone(),
        openai_custom_endpoint: config.openai_custom_endpoint.clone(),
        openai_api_key: None,
        anthropic_key_option: config.anthropic_key_option.clone(),
        anthropic_model: config.anthropic_model.clone(),
        anthropic_api_key: None,
        ollama_endpoint: config.ollama_endpoint.clone(),
        ollama_model: config.ollama_model.clone(),
        lmstudio_endpoint: config.lmstudio_endpoint.clone(),
        lmstudio_model: config.lmstudio_model.clone(),
    }
}

/// The chosen provider's model and endpoint, and for the hosted ones whose key, and
/// whether one is stored.
fn provider_parts(config: &AiConfiguration) -> (&str, Option<&str>, Option<(&str, bool)>) {
    match config.provider.as_str() {
        "anthropic" => (
            &config.anthropic_model,
            None,
            Some((&config.anthropic_key_option, config.anthropic_has_api_key)),
        ),
        "ollama" => (&config.ollama_model, Some(&config.ollama_endpoint), None),
        "lmstudio" => (
            &config.lmstudio_model,
            Some(&config.lmstudio_endpoint),
            None,
        ),
        _ => (
            &config.openai_model,
            None,
            Some((&config.openai_key_option, config.openai_has_api_key)),
        ),
    }
}

impl SettingsView {
    /// Read the AI settings again next time they're shown, as after signing in or out.
    pub(super) fn forget_ai(&mut self) {
        self.ai.config = None;
    }

    /// Read the AI settings the first time they're needed.
    pub(super) fn ensure_ai(&mut self, cx: &mut Context<Self>) {
        if self.ai.config.is_some() || self.ai.loading {
            return;
        }
        self.ai.loading = true;
        cx.spawn(async move |this, cx| {
            let read = run(backend::ai_configuration).await;
            this.update(cx, |this, cx| {
                this.ai.loading = false;
                match read {
                    Ok(config) => {
                        this.ai.config = Some(config);
                        this.sync_ai_fields(cx);
                    }
                    Err(err) => this.error("Couldn't read the AI settings", err, cx),
                }
            })
            .ok();
        })
        .detach();
    }

    /// Show the stored model and endpoint in their fields again.
    fn sync_ai_fields(&mut self, cx: &mut Context<Self>) {
        let Some(config) = &self.ai.config else {
            return;
        };
        let (model, endpoint, _) = provider_parts(config);
        let (model, endpoint) = (model.to_owned(), endpoint.unwrap_or_default().to_owned());
        self.ai
            .model
            .update(cx, |input, cx| input.set_text(model, cx));
        self.ai
            .endpoint
            .update(cx, |input, cx| input.set_text(endpoint, cx));
        self.ai.key.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }

    fn update_ai(
        &mut self,
        change: impl FnOnce(&mut AiConfigurationUpdate) + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = &self.ai.config else {
            return;
        };
        let mut update = update_from(config);
        change(&mut update);
        cx.spawn(async move |this, cx| {
            let saved = run(move || backend::update_ai(update)).await;
            this.update(cx, |this, cx| match saved {
                Ok(config) => {
                    this.ai.config = Some(config);
                    this.ai.own_pending = false;
                    this.sync_ai_fields(cx);
                }
                Err(err) => this.error("Couldn't save the AI settings", err, cx),
            })
            .ok();
        })
        .detach();
    }

    fn save_ai_field(&mut self, input: &Entity<TextInput>, cx: &mut Context<Self>) {
        let text = input.read(cx).text().trim().to_owned();
        let Some(provider) = self.ai.config.as_ref().map(|c| c.provider.clone()) else {
            return;
        };
        if *input == self.ai.key {
            if text.is_empty() {
                return;
            }
            self.update_ai(
                move |update| match provider.as_str() {
                    "anthropic" => {
                        update.anthropic_key_option = OWN_KEY.into();
                        update.anthropic_api_key = Some(text);
                    }
                    _ => {
                        update.openai_key_option = OWN_KEY.into();
                        update.openai_api_key = Some(text);
                    }
                },
                cx,
            );
        } else if *input == self.ai.model {
            self.update_ai(
                move |update| match provider.as_str() {
                    "anthropic" => update.anthropic_model = text,
                    "ollama" => update.ollama_model = text,
                    "lmstudio" => update.lmstudio_model = text,
                    _ => update.openai_model = text,
                },
                cx,
            );
        } else {
            self.update_ai(
                move |update| match provider.as_str() {
                    "lmstudio" => update.lmstudio_endpoint = text,
                    _ => update.ollama_endpoint = text,
                },
                cx,
            );
        }
    }

    pub(super) fn ai_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<Row> {
        let Some(config) = &self.ai.config else {
            return Vec::new();
        };
        // A field with Save while it differs from what's stored.
        let ai_field = |input: &Entity<TextInput>, stored: &str| {
            let changed = input.read(cx).text().trim() != stored;
            field_with_save(input, changed, Self::save_ai_field, window, cx)
        };
        // A provider chosen in GitButler's other apps that these settings can't change, like
        // OpenRouter: only picking another one here can be saved.
        let known = PROVIDERS.iter().find(|(id, _)| *id == config.provider);
        let provider_name = match (known, config.provider.as_str()) {
            (Some((_, name)), _) => *name,
            (None, "openrouter") => "OpenRouter",
            (None, other) => other,
        };
        let (model, endpoint, key) = provider_parts(config);
        let control = self.choice(
            "ai-provider",
            provider_name,
            PROVIDERS.map(|(id, name)| (id.to_owned(), name.to_owned())),
            |this, provider: String, cx| {
                this.update_ai(move |update| update.provider = provider, cx)
            },
            cx,
        );
        let description = if known.is_none() {
            format!(
                "{provider_name} was set up in another GitButler app, which is where it changes. \
                 Pick a provider here to use that instead."
            )
        } else if config.is_configured {
            "Writes commit messages and pull request descriptions when you ask.".into()
        } else {
            format!("{provider_name} needs a key before it can write anything.")
        };
        let mut rows = vec![Row::new("Provider", description, control)];
        if known.is_none() {
            return rows;
        }
        if let Some((option, has_key)) = key {
            let option = if self.ai.own_pending { OWN_KEY } else { option };
            let provider = config.provider.clone();
            let choice = |id: &'static str, label: &'static str| {
                let provider = provider.clone();
                button(SharedString::from(format!("ai-key-{id}")), label)
                    .small()
                    .selected(option == id)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Without a key stored yet, wait for one before saving.
                        this.ai.own_pending = id == OWN_KEY && !has_key;
                        if this.ai.own_pending {
                            cx.notify();
                            return;
                        }
                        let provider = provider.clone();
                        this.update_ai(
                            move |update| match provider.as_str() {
                                "anthropic" => update.anthropic_key_option = id.into(),
                                _ => update.openai_key_option = id.into(),
                            },
                            cx,
                        )
                    }))
            };
            rows.push(Row::new(
                "Key",
                "GitButler's needs you signed in to GitButler; your own stays in the keychain.",
                h_flex()
                    .gap(px(2.))
                    .child(choice(GITBUTLERS_KEY, "GitButler's"))
                    .child(choice(OWN_KEY, "Your own")),
            ));
            if option == OWN_KEY {
                let description = if has_key {
                    "A key is stored. Paste another to replace it.".into()
                } else {
                    format!("Paste your {provider_name} key.")
                };
                rows.push(Row::new("API key", description, ai_field(&self.ai.key, "")));
            }
        }
        if let Some(endpoint) = endpoint {
            rows.push(Row::new(
                "Endpoint",
                format!("Where {provider_name} listens."),
                ai_field(&self.ai.endpoint, endpoint),
            ));
        }
        rows.push(Row::new(
            "Model",
            format!("The {provider_name} model to ask."),
            ai_field(&self.ai.model, model),
        ));
        rows
    }
}
