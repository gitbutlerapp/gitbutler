use std::path::PathBuf;

use anyhow::{Context, Result};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::RepositoryExt;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::StackId;

#[derive(Clone)]
pub struct App {
    pub app_data_dir: PathBuf,
}

/// Access to primary categories of data.
impl App {
    pub fn projects(&self) -> projects::Controller {
        projects::Controller::from_path(self.app_data_dir.clone())
    }

    pub fn users(&self) -> gitbutler_user::Controller {
        gitbutler_user::Controller::from_path(&self.app_data_dir)
    }
}

impl App {
    pub fn git_remote_branches(
        &self,
        project_id: ProjectId,
        settings: AppSettings,
    ) -> Result<Vec<RemoteRefname>> {
        let project = gitbutler_project::get(project_id)?;
        let ctx = CommandContext::open(&project, settings)?;
        ctx.repo().remote_branches()
    }

    pub fn git_test_push(
        &self,
        project_id: ProjectId,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<StackId>>,
        settings: AppSettings,
    ) -> Result<()> {
        let project = gitbutler_project::get(project_id)?;
        let ctx = CommandContext::open(&project, settings)?;
        ctx.git_test_push(remote_name, branch_name, askpass)
    }

    pub fn git_test_fetch(
        &self,
        project_id: ProjectId,
        remote_name: &str,
        askpass: Option<String>,
        settings: AppSettings,
    ) -> Result<()> {
        let project = gitbutler_project::get(project_id)?;
        let ctx = CommandContext::open(&project, settings)?;
        ctx.fetch(remote_name, askpass)
    }

    pub fn git_index_size(&self, project_id: ProjectId, settings: AppSettings) -> Result<usize> {
        let project = gitbutler_project::get(project_id)?;
        let ctx = CommandContext::open(&project, settings)?;
        let size = ctx
            .repo()
            .index()
            .context("failed to get index size")?
            .len();
        Ok(size)
    }

    pub fn git_head(&self, project_id: ProjectId, settings: AppSettings) -> Result<String> {
        let project = gitbutler_project::get(project_id)?;
        let ctx = CommandContext::open(&project, settings)?;
        let head = ctx.repo().head().context("failed to get repository head")?;
        Ok(head.name().unwrap().to_string())
    }

    pub fn git_set_global_config(key: &str, value: &str) -> Result<String> {
        let mut config = git2::Config::open_default()?;
        config.set_str(key, value)?;
        Ok(value.to_string())
    }

    pub fn git_remove_global_config(key: &str) -> Result<()> {
        let mut config = git2::Config::open_default()?;
        Ok(config.remove(key)?)
    }

    pub fn git_get_global_config(key: &str) -> Result<Option<String>> {
        let config = git2::Config::open_default()?;
        let value = config.get_string(key);
        match value {
            Ok(value) => Ok(Some(value)),
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub fn delete_all_data(&self) -> Result<()> {
        let controller = self.projects();
        for project in controller.list().context("failed to list projects")? {
            controller
                .delete(project.id)
                .map_err(|err| err.context("failed to delete project"))?;
        }
        Ok(())
    }
}
