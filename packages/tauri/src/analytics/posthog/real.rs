use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDateTime;
use reqwest::{header::CONTENT_TYPE, Client as HttpClient};

use serde::Serialize;
use serde_json;
use tracing::instrument;

const API_ENDPOINT: &str = "https://eu.posthog.com/capture/";
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
    async fn capture(&self, event: super::Event) -> Result<(), super::Error> {
        let mut event = event;
        event
            .properties
            .insert("appName", self.options.app_name.clone())?;
        event
            .properties
            .insert("appVersion", self.options.app_version.clone())?;
        let inner_event = InnerEvent::new(&event, self.options.api_key.clone());
        let _res = self
            .client
            .post(API_ENDPOINT)
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&inner_event).expect("unwrap here is safe"))
            .send()
            .await?;
        Ok(())
    }
}

// This exists so that the client doesn't have to specify the API key over and over
#[derive(Serialize)]
struct InnerEvent {
    api_key: String,
    event: String,
    properties: super::Properties,
    timestamp: Option<NaiveDateTime>,
}

impl InnerEvent {
    fn new(event: &super::Event, api_key: String) -> Self {
        Self {
            api_key,
            event: event.event.clone(),
            properties: event.properties.clone(),
            timestamp: event.timestamp,
        }
    }
}
