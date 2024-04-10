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
    pub fn from_app(app: &AppHandle) -> Self {
        let client = app
            .try_state::<analytics::Client>()
            .map_or(analytics::Client::default(), |client| {
                client.inner().clone()
            });
        let users = app.state::<users::Controller>().inner().clone();
        Handler { users, client }
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
