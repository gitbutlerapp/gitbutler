use std::path;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    analytics, events as app_events, gb_repository, git, project_repository,
    projects::{self, ProjectId},
    users,
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

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let projects = value.state::<projects::Controller>().inner().clone();
            let users = value.state::<users::Controller>().inner().clone();
            let handler = Handler::new(app_data_dir, projects, users);
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

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

    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;

        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository for project")?;

        match path.as_ref().to_str().unwrap() {
            "FETCH_HEAD" => Ok(vec![
                events::Event::Emit(app_events::Event::git_fetch(&project.id)),
                events::Event::CalculateVirtualBranches(*project_id),
            ]),
            "logs/HEAD" => Ok(vec![events::Event::Emit(app_events::Event::git_activity(
                &project.id,
            ))]),
            "GB_FLUSH" => {
                let user = self.users.get_user()?;
                let gb_repo = gb_repository::Repository::open(
                    &self.local_data_dir,
                    &project_repository,
                    user.as_ref(),
                )
                .context("failed to open repository")?;

                let file_path = project.path.join(".git/GB_FLUSH");

                if file_path.exists() {
                    if let Err(e) = std::fs::remove_file(&file_path) {
                        tracing::error!(%project_id, path = %file_path.display(), "GB_FLUSH file delete error: {}", e);
                    }

                    if let Some(current_session) = gb_repo
                        .get_current_session()
                        .context("failed to get current session")?
                    {
                        return Ok(vec![events::Event::Flush(project.id, current_session)]);
                    }
                }

                Ok(vec![])
            }
            "HEAD" => {
                let head_ref = project_repository
                    .get_head()
                    .context("failed to get head")?;
                let head_ref_name = head_ref.name().context("failed to get head name")?;
                if head_ref_name.to_string() != "refs/heads/gitbutler/integration" {
                    let mut integration_reference = project_repository
                        .git_repository
                        .find_reference(&git::Refname::from(git::LocalRefname::new(
                            "gitbutler/integration",
                            None,
                        )))?;
                    integration_reference.delete()?;
                }
                if let Some(head) = head_ref.name() {
                    Ok(vec![
                        events::Event::Analytics(analytics::Event::HeadChange {
                            project_id: project.id,
                            reference_name: head_ref_name.to_string(),
                        }),
                        events::Event::Emit(app_events::Event::git_head(
                            &project.id,
                            &head.to_string(),
                        )),
                    ])
                } else {
                    Ok(vec![])
                }
            }
            "index" => Ok(vec![events::Event::Emit(app_events::Event::git_index(
                &project.id,
            ))]),
            _ => Ok(vec![]),
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use events::Event;
    use pretty_assertions::assert_eq;

    use crate::{
        tests::{Case, Suite},
        watcher::handlers,
    };

    use super::*;

    #[test]
    fn test_flush_session() -> Result<()> {
        let suite = Suite::default();
        let Case {
            project,
            gb_repository,
            ..
        } = suite.new_case();

        assert!(gb_repository.get_current_session()?.is_none());
        create_new_session_via_new_file(&project, &suite);
        assert!(gb_repository.get_current_session()?.is_some());

        let listener = Handler {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
        };

        let flush_file_path = project.path.join(".git/GB_FLUSH");
        fs::write(flush_file_path.as_path(), "")?;

        let result = listener.handle("GB_FLUSH", &project.id)?;

        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Event::Flush(_, _)));

        assert!(!flush_file_path.exists(), "flush file deleted");

        Ok(())
    }

    #[test]
    fn test_do_not_flush_session_if_file_is_missing() -> Result<()> {
        let suite = Suite::default();
        let Case {
            project,
            gb_repository,
            ..
        } = suite.new_case();

        assert!(gb_repository.get_current_session()?.is_none());
        create_new_session_via_new_file(&project, &suite);
        assert!(gb_repository.get_current_session()?.is_some());

        let listener = Handler {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
        };

        let result = listener.handle("GB_FLUSH", &project.id)?;

        assert_eq!(result.len(), 0);

        Ok(())
    }

    fn create_new_session_via_new_file(project: &projects::Project, suite: &Suite) {
        fs::write(project.path.join("test.txt"), "test").unwrap();

        let file_change_listener =
            handlers::calculate_deltas_handler::Handler::from_path(&suite.local_app_data);
        file_change_listener
            .handle("test.txt", &project.id)
            .unwrap();
    }

    #[test]
    fn test_flush_deletes_flush_file_without_session_to_flush() -> Result<()> {
        let suite = Suite::default();
        let Case { project, .. } = suite.new_case();

        let listener = Handler {
            local_data_dir: suite.local_app_data,
            projects: suite.projects,
            users: suite.users,
        };

        let flush_file_path = project.path.join(".git/GB_FLUSH");
        fs::write(flush_file_path.as_path(), "")?;

        let result = listener.handle("GB_FLUSH", &project.id)?;

        assert_eq!(result.len(), 0);

        assert!(!flush_file_path.exists(), "flush file deleted");

        Ok(())
    }
}
