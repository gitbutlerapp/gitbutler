use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDateTime;
use reqwest::{header::CONTENT_TYPE, Client as HttpClient};
use serde::Serialize;
use tracing::instrument;

const API_ENDPOINT: &str = "https://eu.posthog.com/batch/";
const TIMEOUT: &Duration = &Duration::from_millis(800);

pub struct ClientOptions {
    pub app_name: String,
    pub app_version: String,
    pub api_key: String,
}

pub struct Client {
    options: ClientOptions,
    client: HttpClient,
}

impl Client {
    pub fn new<C: Into<ClientOptions>>(options: C) -> Self {
        let client = HttpClient::builder().timeout(*TIMEOUT).build().unwrap(); // Unwrap here is as safe as `HttpClient::new`
        Client {
            options: options.into(),
            client,
        }
    }
}

#[async_trait]
impl super::Client for Client {
    #[instrument(skip(self), level = "debug")]
    async fn capture(&self, events: &[super::Event]) -> Result<(), super::Error> {
        let events = events
            .iter()
            .map(|event| {
                let event = &mut event.clone();
                event
                    .properties
                    .insert("appName", self.options.app_name.clone());
                event
                    .properties
                    .insert("appVersion", self.options.app_version.clone());
                Event::from(event)
            })
            .collect::<Vec<_>>();

        let batch = Batch {
            api_key: &self.options.api_key,
            batch: events.as_slice(),
        };

        let response = self
            .client
            .post(API_ENDPOINT)
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&batch)?)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(super::Error::BadRequest {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            })
        }
    }
}

#[derive(Serialize)]
struct Batch<'a> {
    api_key: &'a str,
    batch: &'a [Event],
}

#[derive(Serialize)]
struct Event {
    event: String,
    properties: super::Properties,
    timestamp: Option<NaiveDateTime>,
}

impl From<&mut super::Event> for Event {
    fn from(event: &mut super::Event) -> Self {
        Self {
            event: event.event.clone(),
            properties: event.properties.clone(),
            timestamp: event.timestamp,
        }
    }
}
