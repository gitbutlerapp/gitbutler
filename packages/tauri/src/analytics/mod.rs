use std::{str, sync::Arc};

use tauri::AppHandle;

use crate::users::User;

mod posthog;

pub struct Config<'c> {
    pub posthog_token: Option<&'c str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    HeadChange {
        project_id: String,
        reference_name: String,
    },
}

impl Event {
    pub fn project_id(&self) -> &str {
        match self {
            Event::HeadChange { project_id, .. } => project_id,
        }
    }

    fn into_posthog_event(self, user: &User) -> Result<posthog::Event, posthog::Error> {
        match self {
            Event::HeadChange {
                project_id,
                reference_name: reference,
            } => {
                let mut event =
                    posthog::Event::new("git::head_changed", &format!("user_{}", user.id));
                event.insert_prop("project_id", format!("project_{}", project_id))?;
                event.insert_prop("reference", reference)?;
                Ok(event)
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
                Box::new(posthog::real::Client::new(posthog::real::ClientOptions {
                    api_key: posthog_token.to_string(),
                    app_name: app_handle.package_info().name.to_string(),
                    app_version: app_handle.package_info().version.to_string(),
                }))
            } else {
                Box::<posthog::mock::Client>::default()
            };
        Client {
            client: Arc::new(client),
        }
    }

    pub async fn send(&self, user: &User, event: &Event) -> Result<(), posthog::Error> {
        self.client
            .capture(event.clone().into_posthog_event(user)?)
            .await?;
        Ok(())
    }
}
