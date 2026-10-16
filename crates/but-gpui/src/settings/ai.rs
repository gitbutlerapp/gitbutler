//! The AI section: which provider writes commit messages and pull request descriptions,
//! with whose key, and which model. GitButler's other apps share it. It's read only when
//! shown, as reading it checks the keychain for stored keys.

use gpui::{Context, Entity, Focusable as _, SharedString, Window, prelude::*, px};

use super::{Row, Section, SettingsView};
use crate::backend::run;
use crate::backend::settings::{self as backend, AiConfiguration, AiConfigurationUpdate};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, button, dropdown, field, h_flex};

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
                    "anthropic" => update.anthropic_api_key = Some(text),
                    _ => update.openai_api_key = Some(text),
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

    /// A field with Save beside it while it differs from what's stored.
    fn ai_field(
        &self,
        input: &Entity<TextInput>,
        stored: &str,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let changed = input.read(cx).text().trim() != stored;
        let focused = input.focus_handle(cx).is_focused(window);
        let save = input.clone();
        h_flex()
            .gap(px(6.))
            .when(changed, |el| {
                el.child(
                    button(
                        SharedString::from(format!("save-{}", input.entity_id())),
                        "Save",
                    )
                    .small()
                    .style(ButtonStyle::Gray)
                    .on_click(cx.listener(move |this, _, _, cx| this.save_ai_field(&save, cx))),
                )
            })
            .child(field(input, focused, theme).w(px(220.)))
            .into_any_element()
    }

    pub(super) fn ai_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<Row> {
        let theme = Theme::get(cx);
        let Some(config) = &self.ai.config else {
            return Vec::new();
        };
        let provider_name = PROVIDERS
            .iter()
            .find(|(id, _)| *id == config.provider)
            .map_or("OpenAI", |(_, name)| *name);
        let (model, endpoint, key) = provider_parts(config);
        let items = self.choices(
            PROVIDERS.map(|(id, name)| (id.to_owned(), name.to_owned())),
            |this, provider: String, cx| {
                this.update_ai(move |update| update.provider = provider, cx)
            },
            cx,
        );
        let this = cx.entity().downgrade();
        let mut rows = vec![Row {
            section: Section::Ai,
            title: "Provider".into(),
            description: if config.is_configured {
                "Writes commit messages and pull request descriptions when you ask.".into()
            } else {
                format!("{provider_name} needs a key before it can write anything.").into()
            },
            control: dropdown("ai-provider", provider_name, theme)
                .on_click(move |event, _, cx| {
                    this.update(cx, |this, cx| this.open_menu(event, items.clone(), cx))
                        .ok();
                })
                .into_any_element(),
        }];
        if let Some((option, has_key)) = key {
            let provider = config.provider.clone();
            let choice = |id: &'static str, label: &'static str| {
                let provider = provider.clone();
                button(SharedString::from(format!("ai-key-{id}")), label)
                    .small()
                    .style(if option == id {
                        ButtonStyle::Gray
                    } else {
                        ButtonStyle::Ghost
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
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
            rows.push(Row {
                section: Section::Ai,
                title: "Key".into(),
                description: "GitButler's needs you signed in to GitButler; your own stays in \
                              the keychain."
                    .into(),
                control: h_flex()
                    .gap(px(2.))
                    .child(choice(GITBUTLERS_KEY, "GitButler's"))
                    .child(choice(OWN_KEY, "Your own"))
                    .into_any_element(),
            });
            if option == OWN_KEY {
                rows.push(Row {
                    section: Section::Ai,
                    title: "API key".into(),
                    description: if has_key {
                        "A key is stored. Paste another to replace it.".into()
                    } else {
                        format!("Paste your {provider_name} key.").into()
                    },
                    control: self.ai_field(&self.ai.key, "", window, theme, cx),
                });
            }
        }
        if let Some(endpoint) = endpoint {
            rows.push(Row {
                section: Section::Ai,
                title: "Endpoint".into(),
                description: format!("Where {provider_name} listens.").into(),
                control: self.ai_field(&self.ai.endpoint, endpoint, window, theme, cx),
            });
        }
        rows.push(Row {
            section: Section::Ai,
            title: "Model".into(),
            description: format!("The {provider_name} model to ask.").into(),
            control: self.ai_field(&self.ai.model, model, window, theme, cx),
        });
        rows
    }
}
