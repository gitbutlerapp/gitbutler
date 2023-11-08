use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    gb_repository, paths::DataDir, project_repository, projects, projects::ProjectId, sessions,
    users, virtual_branches,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    projects: projects::Controller,
    local_data_dir: DataDir,
    users: users::Controller,
    vbranches_controller: virtual_branches::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            projects: projects::Controller::try_from(value)?,
            local_data_dir: DataDir::try_from(value)?,
            users: users::Controller::from(value),
            vbranches_controller: virtual_branches::Controller::try_from(value)?,
        })
    }
}

impl Handler {
    pub fn handle(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;

        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        futures::executor::block_on(async {
            self.vbranches_controller
                .flush_vbranches(project_repository.project().id)
                .await
        })?;

        let session = gb_repo
            .flush_session(&project_repository, session, user.as_ref())
            .context("failed to flush session")?;

        Ok(vec![
            events::Event::Session(*project_id, session),
            events::Event::PushGitbutlerData(*project_id),
            events::Event::PushProjectToGitbutler(*project_id),
        ])
    }
}
