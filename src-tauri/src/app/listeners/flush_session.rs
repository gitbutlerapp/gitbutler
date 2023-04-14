use std::{sync, time};

use anyhow::{anyhow, Context, Result};

use crate::{app::gb_repository, events, projects, sessions, users};

pub struct Listener<'listener> {
    project_id: String,
    user_store: &'listener users::Storage,
    gb_repository: &'listener gb_repository::Repository,
    sender: sync::mpsc::Sender<events::Event>,
}

impl<'listener> Listener<'listener> {
    pub fn new(
        project: &projects::Project,
        user_store: &'listener users::Storage,
        gb_repository: &'listener gb_repository::Repository,
        sender: sync::mpsc::Sender<events::Event>,
    ) -> Self {
        Self {
            project_id: project.id.clone(),
            user_store,
            gb_repository,
            sender,
        }
    }

    pub fn register(&self, session: &sessions::Session) -> Result<()> {
        let session = sessions::Session {
            id: session.id.clone(),
            hash: session.hash.clone(),
            activity: session.activity.clone(),
            meta: sessions::Meta {
                last_timestamp_ms: time::SystemTime::now()
                    .duration_since(time::SystemTime::UNIX_EPOCH)?
                    .as_millis(),
                ..session.meta.clone()
            },
        };

        let user = self.user_store.get().context("failed to get user")?;

        self.flush(user, &session)
            .context("failed to flush session")?;

        Ok(())
    }

    fn flush(&self, user: Option<users::User>, session: &sessions::Session) -> Result<()> {
        Err(anyhow!("not implemented"))
    }
}
