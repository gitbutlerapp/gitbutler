use std::env;

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
pub struct Event {
    event_name: EventKind,
    props: Vec<(String, String)>,
}

impl Event {
    pub fn new(event_name: EventKind, mut props: Vec<(String, String)>) -> Self {
        props.push((
            "appVersion".to_string(),
            option_env!("CARGO_PKG_VERSION")
                .unwrap_or_default()
                .to_string(),
        ));
        props.push((
            "appName".to_string(),
            option_env!("CARGO_BIN_NAME")
                .unwrap_or_default()
                .to_string(),
        ));
        props.push(("OS".to_string(), Event::normalize_os(env::consts::OS)));
        props.push(("Arch".to_string(), env::consts::ARCH.to_string()));
        Self { event_name, props }
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

    pub fn capture(&self, event: Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }
}
