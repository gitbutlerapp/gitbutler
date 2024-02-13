use std::{path, time};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    gb_repository, project_repository,
    projects::{self, FetchResult, ProjectId},
    sessions, users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let handler = Handler::new(
                app_data_dir,
                projects::Controller::try_from(value)?,
                users::Controller::try_from(value)?,
            );
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

const GB_FETCH_INTERVAL: time::Duration = time::Duration::new(15 * 60, 0);
const PROJECT_FETCH_INTERVAL: time::Duration = time::Duration::new(15 * 60, 0);
const PROJECT_PUSH_INTERVAL: time::Duration = time::Duration::new(15 * 60, 0);

impl Handler {
    fn new(
        local_data_dir: path::PathBuf,
        projects: projects::Controller,
        users: users::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            projects,
            users,
        }
    }

    pub fn handle(
        &self,
        project_id: &ProjectId,
        now: &time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        let user = self.users.get_user()?;

        let project = self.projects.get(project_id)?;
        let project_repository = match project_repository::Repository::open(&project) {
            Ok(project_repository) => Ok(project_repository),
            Err(project_repository::OpenError::NotFound(_)) => return Ok(vec![]),
            Err(error) => Err(error),
        }
        .context("failed to open project repository")?;

        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let mut events = vec![];

        let project_data_last_fetch = project
            .project_data_last_fetch
            .as_ref()
            .map(FetchResult::timestamp)
            .copied()
            .unwrap_or(time::UNIX_EPOCH);

        if now.duration_since(project_data_last_fetch)? > PROJECT_FETCH_INTERVAL {
            events.push(events::Event::FetchProjectData(*project_id));
        }

        if project.is_sync_enabled() {
            let gb_data_last_fetch = project
                .gitbutler_data_last_fetch
                .as_ref()
                .map(FetchResult::timestamp)
                .copied()
                .unwrap_or(time::UNIX_EPOCH);

            if now.duration_since(gb_data_last_fetch)? > GB_FETCH_INTERVAL {
                events.push(events::Event::FetchGitbutlerData(*project_id));
            }
        }

        if let Some(current_session) = gb_repo
            .get_current_session()
            .context("failed to get current session")?
        {
            if should_flush(now, &current_session)? {
                events.push(events::Event::Flush(*project_id, current_session));
            }
        }

        let should_push_code = project_repository.project().is_sync_enabled()
            && project_repository.project().has_code_url();

        if should_push_code {
            let project_code_last_push = project
                .gitbutler_code_push_state
                .as_ref()
                .map(|state| &state.timestamp)
                .copied()
                .unwrap_or(time::UNIX_EPOCH);

            if now.duration_since(project_code_last_push)? > PROJECT_PUSH_INTERVAL {
                events.push(events::Event::PushProjectToGitbutler(*project_id));
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
    use crate::sessions::SessionId;

    use super::*;

    const ONE_MILLISECOND: time::Duration = time::Duration::from_millis(1);

    #[test]
    fn test_should_flush() {
        let now = time::SystemTime::now();
        for (start, last, expected) in vec![
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
        ] {
            let session = sessions::Session {
                id: SessionId::generate(),
                hash: None,
                meta: sessions::Meta {
                    start_timestamp_ms: start.duration_since(time::UNIX_EPOCH).unwrap().as_millis(),
                    last_timestamp_ms: last.duration_since(time::UNIX_EPOCH).unwrap().as_millis(),
                    branch: None,
                    commit: None,
                },
            };
            assert_eq!(should_flush(&now, &session).unwrap(), expected);
        }
    }
}

#[cfg(test)]
mod test_handler {
    use std::time::SystemTime;

    use crate::test_utils::{Case, Suite};

    use super::super::test_remote_repository;
    use super::*;

    #[tokio::test]
    async fn test_fetch_triggered() -> Result<()> {
        let suite = Suite::default();
        let Case { project, .. } = suite.new_case();

        let cloud = test_remote_repository()?;

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: cloud.path().to_str().unwrap().to_string(),
            code_git_url: None,
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        let listener = Handler {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
        };

        let result = listener.handle(&project.id, &SystemTime::now()).unwrap();

        assert!(result
            .iter()
            .any(|ev| matches!(ev, events::Event::FetchGitbutlerData(_))));

        Ok(())
    }

    #[test]
    fn test_no_fetch_triggered() {
        let suite = Suite::default();
        let Case { project, .. } = suite.new_case();

        let listener = Handler {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
        };

        let result = listener.handle(&project.id, &SystemTime::now()).unwrap();

        assert!(!result
            .iter()
            .any(|ev| matches!(ev, events::Event::FetchGitbutlerData(_))));
    }
}
