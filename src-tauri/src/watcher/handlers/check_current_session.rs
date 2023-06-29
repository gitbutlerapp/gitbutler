use std::{path, time};

use anyhow::{Context, Result};

use crate::{gb_repository, projects, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_id: String,
    project_store: projects::Storage,
    local_data_dir: path::PathBuf,
    user_store: users::Storage,
}

impl Handler {
    pub fn new(
        local_data_dir: path::PathBuf,
        project_id: String,
        project_store: projects::Storage,
        user_store: users::Storage,
    ) -> Self {
        Self {
            project_id,
            project_store,
            local_data_dir,
            user_store,
        }
    }

    pub fn handle(&self, now: time::SystemTime) -> Result<Vec<events::Event>> {
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            self.project_id.clone(),
            self.project_store.clone(),
            self.user_store.clone(),
        )
        .context("failed to open repository")?;
        match gb_repo
            .get_current_session()
            .context("failed to get current session")?
        {
            None => Ok(vec![]),
            Some(current_session) => {
                if should_flush(now, &current_session)? {
                    Ok(vec![events::Event::Flush(current_session)])
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

pub(super) fn should_flush(now: time::SystemTime, session: &sessions::Session) -> Result<bool> {
    Ok(!is_session_active(now, session)? || is_session_too_old(now, session)?)
}

const ONE_HOUR: time::Duration = time::Duration::new(60 * 60, 0);

fn is_session_too_old(now: time::SystemTime, session: &sessions::Session) -> Result<bool> {
    let session_start =
        time::UNIX_EPOCH + time::Duration::from_millis(session.meta.start_timestamp_ms.try_into()?);
    Ok(session_start + ONE_HOUR < now)
}

const FIVE_MINUTES: time::Duration = time::Duration::new(5 * 60, 0);

fn is_session_active(now: time::SystemTime, session: &sessions::Session) -> Result<bool> {
    let session_last_update =
        time::UNIX_EPOCH + time::Duration::from_millis(session.meta.last_timestamp_ms.try_into()?);
    Ok(session_last_update + FIVE_MINUTES > now)
}
