//! A client to provide analytics.
use std::{fmt, str, sync::Arc};

use gitbutler_core::{projects::ProjectId, users::User};

mod posthog;

pub struct Config<'c> {
    pub posthog_token: Option<&'c str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    HeadChange {
        project_id: ProjectId,
        reference_name: String,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::HeadChange {
                project_id,
                reference_name,
            } => write!(
                f,
                "HeadChange(project_id: {}, reference_name: {})",
                project_id, reference_name
            ),
        }
    }
}

impl Event {
    pub fn project_id(&self) -> ProjectId {
        match self {
            Event::HeadChange { project_id, .. } => *project_id,
        }
    }

    fn into_posthog_event(self, user: &User) -> posthog::Event {
        match self {
            Event::HeadChange {
                project_id,
                reference_name: reference,
            } => {
                let mut event =
                    posthog::Event::new("git::head_changed", &format!("user_{}", user.id));
                event.insert_prop("project_id", format!("project_{}", project_id));
                event.insert_prop("reference", reference);
                event
            }
        }
    }
}

/// NOTE: Needs to be `Clone` only because the watcher wants to obtain it from `tauri`.
/// It's just for dependency injection.
#[derive(Clone)]
pub struct Client {
    client: Arc<dyn posthog::Client + Sync + Send>,
}

impl Client {
    pub fn new(app_name: String, app_version: String, config: &Config) -> Self {
        let client: Arc<dyn posthog::Client + Sync + Send> =
            if let Some(posthog_token) = config.posthog_token {
                let real = posthog::real::Client::new(posthog::real::ClientOptions {
                    api_key: posthog_token.to_string(),
                    app_name,
                    app_version,
                });
                let real_with_retry = posthog::retry::Client::new(real);
                Arc::new(real_with_retry)
            } else {
                Arc::<posthog::mock::Client>::default()
            };
        Client { client }
    }

    /// Send `event` to analytics and associate it with `user` without blocking.
    pub fn send_non_anonymous_event_nonblocking(&self, user: &User, event: &Event) {
        let client = self.client.clone();
        let event = event.clone().into_posthog_event(user);
        tokio::spawn(async move {
            if let Err(error) = client.capture(&[event]).await {
                tracing::warn!(?error, "failed to send analytics");
            }
        });
    }
}

impl Default for Client {
    fn default() -> Self {
        Self {
            client: Arc::new(posthog::mock::Client),
        }
    }
}
