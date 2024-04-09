use anyhow::{Context, Result};
use gitbutler_core::users;
use tauri::{AppHandle, Manager};

use super::events;
use crate::analytics;

#[derive(Clone)]
pub struct Handler {
    users: users::Controller,
    client: analytics::Client,
}

impl Handler {
    pub fn from_app(value: &AppHandle) -> Result<Self, anyhow::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else {
            let client = value
                .try_state::<analytics::Client>()
                .map_or(analytics::Client::default(), |client| {
                    client.inner().clone()
                });
            let users = value.state::<users::Controller>().inner().clone();
            let handler = Handler { users, client };
            value.manage(handler.clone());
            Ok(handler)
        }
    }
}

impl Handler {
    pub async fn handle(&self, event: &analytics::Event) -> Result<Vec<events::Event>> {
        if let Some(user) = self.users.get_user().context("failed to get user")? {
            self.client.send(&user, event).await;
        }
        Ok(vec![])
    }
}
