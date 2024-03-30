use std::{fmt, str, sync::Arc};

use gitbutler::{projects::ProjectId, users::User};
use tauri::AppHandle;

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
    pub fn project_id(&self) -> &ProjectId {
        match self {
            Event::HeadChange { project_id, .. } => project_id,
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

#[derive(Clone)]
pub struct Client {
    client: Arc<Box<dyn posthog::Client + Sync + Send>>,
}

impl Client {
    pub fn new(app_handle: &AppHandle, config: &Config) -> Self {
        let client: Box<dyn posthog::Client + Sync + Send> =
            if let Some(posthog_token) = config.posthog_token {
                let real = posthog::real::Client::new(posthog::real::ClientOptions {
                    api_key: posthog_token.to_string(),
                    app_name: app_handle.package_info().name.clone(),
                    app_version: app_handle.package_info().version.to_string(),
                });
                let real_with_retry = posthog::retry::Client::new(real);
                Box::new(real_with_retry)
            } else {
                Box::<posthog::mock::Client>::default()
            };
        Client {
            client: Arc::new(client),
        }
    }

    pub async fn send(&self, user: &User, event: &Event) {
        if let Err(error) = self
            .client
            .capture(&[event.clone().into_posthog_event(user)])
            .await
        {
            tracing::warn!(?error, "failed to send analytics");
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self {
            client: Arc::new(Box::<posthog::mock::Client>::default()),
        }
    }
}
