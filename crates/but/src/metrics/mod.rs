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
    props: Vec<(String, Option<String>)>,
}

impl Event {
    pub fn new(event_name: EventKind, props: Vec<(String, Option<String>)>) -> Self {
        Self { event_name, props }
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
            tokio::task::spawn(async move {
                let client = client_future.await;
                while let Some(event) = receiver.recv().await {
                    let mut posthog_event =
                        posthog_rs::Event::new(event.event_name.to_string(), "user_3".to_string()); // TODO
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
