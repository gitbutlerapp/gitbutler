use std::time;

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, paths::DataDir, project_repository, projects, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::try_from(value)?,
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &str, now: &time::SystemTime) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;

        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
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

fn should_flush(now: &time::SystemTime, session: &sessions::Session) -> Result<bool> {
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

    const ONE_MILLISECOND: time::Duration = time::Duration::from_millis(1);

    #[test]
    fn test_should_flush() {
        let now = time::SystemTime::now();
        vec![
            (now, now, false),                // just created
            (now - FIVE_MINUTES, now, false), // active
            (
                now - FIVE_MINUTES - ONE_MILLISECOND,
                now - FIVE_MINUTES,
                true,
            ), // almost not active
            (
                now - FIVE_MINUTES - ONE_MILLISECOND,
                now - FIVE_MINUTES - ONE_MILLISECOND,
                true,
            ), // not active
            (now - ONE_HOUR, now, true),      // almost too old
            (now - ONE_HOUR - ONE_MILLISECOND, now, true), // too old
        ]
        .into_iter()
        .for_each(|(start, last, expected)| {
            let session = sessions::Session {
                id: "session-id".to_string(),
                hash: None,
                meta: sessions::Meta {
                    start_timestamp_ms: start.duration_since(time::UNIX_EPOCH).unwrap().as_millis(),
                    last_timestamp_ms: last.duration_since(time::UNIX_EPOCH).unwrap().as_millis(),
                    branch: None,
                    commit: None,
                },
            };
            assert_eq!(should_flush(&now, &session).unwrap(), expected);
        });
    }
}
