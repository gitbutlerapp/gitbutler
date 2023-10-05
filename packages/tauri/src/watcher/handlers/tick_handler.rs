use std::{path, time};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, projects, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    project_store: projects::Storage,
    user_store: users::Storage,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("failed to get local data dir")?;
        let project_store = projects::Storage::try_from(value)?;
        let user_store = users::Storage::try_from(value)?;
        Ok(Self {
            project_store,
            local_data_dir,
            user_store,
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &str, now: &time::SystemTime) -> Result<Vec<events::Event>> {
        let user = self.user_store.get()?;

        let project = self.project_store.get(project_id)?;

        let gb_repo =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open repository")?;

        let mut events = vec![];

        if project
            .gitbutler_data_last_fetched
            .as_ref()
            .map_or(Ok(true), |f| f.should_fetch(now))
            .context("failed to check if gitbutler data should be fetched")?
        {
            events.push(events::Event::FetchGitbutlerData(
                project_id.to_string(),
                *now,
            ));
        }

        if project
            .project_data_last_fetched
            .as_ref()
            .map_or(Ok(true), |f| f.should_fetch(now))
            .context("failed to check if project data should be fetched")?
        {
            events.push(events::Event::FetchProjectData(
                project_id.to_string(),
                *now,
            ));
        }

        if let Some(current_session) = gb_repo
            .get_current_session()
            .context("failed to get current session")?
        {
            if should_flush(now, &current_session)? {
                events.push(events::Event::Flush(
                    project_id.to_string(),
                    current_session,
                ));
            }
        }

        Ok(events)
    }
}

pub(super) fn should_flush(now: &time::SystemTime, session: &sessions::Session) -> Result<bool> {
    Ok(!is_session_active(now, session)? || is_session_too_old(now, session)?)
}

const ONE_HOUR: time::Duration = time::Duration::new(60 * 60, 0);

fn is_session_too_old(now: &time::SystemTime, session: &sessions::Session) -> Result<bool> {
    let session_start =
        time::UNIX_EPOCH + time::Duration::from_millis(session.meta.start_timestamp_ms.try_into()?);
    Ok(session_start + ONE_HOUR < *now)
}

const FIVE_MINUTES: time::Duration = time::Duration::new(5 * 60, 0);

fn is_session_active(now: &time::SystemTime, session: &sessions::Session) -> Result<bool> {
    let session_last_update =
        time::UNIX_EPOCH + time::Duration::from_millis(session.meta.last_timestamp_ms.try_into()?);
    Ok(session_last_update + FIVE_MINUTES > *now)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIVE_MINUTES: time::Duration = time::Duration::new(5 * 60, 0);
    const ONE_HOUR: time::Duration = time::Duration::new(60 * 60, 0);

    #[test]
    fn test_should_flush() -> Result<()> {
        let now = time::SystemTime::now();
        let start = now;
        let last = now;

        let session = sessions::Session {
            id: "session-id".to_string(),
            hash: None,
            meta: sessions::Meta {
                start_timestamp_ms: start.duration_since(time::UNIX_EPOCH)?.as_millis(),
                last_timestamp_ms: last.duration_since(time::UNIX_EPOCH)?.as_millis(),
                branch: None,
                commit: None,
            },
        };

        assert!(!should_flush(&now, &session)?);

        assert!(should_flush(&(start + FIVE_MINUTES), &session)?);
        assert!(should_flush(&(last + ONE_HOUR), &session)?);

        Ok(())
    }
}
