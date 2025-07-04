use std::{collections::HashMap, env};

use but_settings::AppSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Metrics {
    sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Mcp,
}
#[derive(Debug, Clone)]
pub struct Event {
    event_name: EventKind,
    props: HashMap<String, serde_json::Value>,
}

impl Event {
    pub fn new(event_name: EventKind) -> Self {
        let event = &mut Event {
            event_name,
            props: HashMap::new(),
        };
        event.insert_prop("appVersion", option_env!("VERSION").unwrap_or_default());
        event.insert_prop("releaseChannel", option_env!("CHANNEL").unwrap_or_default());
        event.insert_prop("appName", option_env!("CARGO_BIN_NAME").unwrap_or_default());
        event.insert_prop("OS", Event::normalize_os(env::consts::OS));
        event.insert_prop("Arch", env::consts::ARCH);
        event.clone()
    }

    pub fn insert_prop<K: Into<String>, P: Serialize>(&mut self, key: K, prop: P) {
        if let Ok(value) = serde_json::to_value(prop) {
            let _ = self.props.insert(key.into(), value);
        }
    }

    fn normalize_os(os: &str) -> String {
        match os {
            "macos" => "Mac OS X".to_string(),
            "windows" => "Windows".to_string(),
            "linux" => "Linux".to_string(),
            "android" => "Android".to_string(),
            _ => os.to_string(),
        }
    }
}

impl Metrics {
    pub fn new_with_background_handling(app_settings: &AppSettings) -> Self {
        let metrics_permitted = app_settings.telemetry.app_metrics_enabled;
        // Only create client and sender if metrics are permitted
        let client = if metrics_permitted {
            option_env!("POSTHOG_API_KEY").and_then(|api_key| {
                let options = posthog_rs::ClientOptionsBuilder::default()
                    .api_key(api_key.to_string())
                    .api_endpoint("https://eu.i.posthog.com/i/v0/e/".to_string())
                    .build()
                    .ok()?;
                Some(posthog_rs::client(options))
            })
        } else {
            None
        };
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let sender = if metrics_permitted {
            Some(sender)
        } else {
            None
        };
        let metrics = Metrics { sender };

        if let Some(client_future) = client {
            let mut receiver = receiver;
            let distinct_id = app_settings.telemetry.app_distinct_id.clone();
            tokio::task::spawn(async move {
                let client = client_future.await;
                while let Some(event) = receiver.recv().await {
                    let mut posthog_event = if let Some(id) = &distinct_id {
                        posthog_rs::Event::new(event.event_name.to_string(), id.clone())
                    } else {
                        posthog_rs::Event::new_anon(event.event_name.to_string())
                    };
                    for (key, prop) in event.props {
                        let _ = posthog_event.insert_prop(key, prop);
                    }
                    let _ = client.capture(posthog_event).await;
                }
            });
        }

        metrics
    }

    pub fn capture(&self, event: &Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event.clone());
        }
    }
}
