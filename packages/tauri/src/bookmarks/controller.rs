use std::ops::Range;

use anyhow::Context;
use tauri::AppHandle;

use crate::{
    gb_repository,
    paths::DataDir,
    project_repository,
    projects::{self, ProjectId},
    users,
};

use super::{database::Database, Bookmark, Writer};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: DataDir,
    database: Database,

    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            database: Database::from(value),
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpsertError {
    #[error(transparent)]
    GetProject(#[from] projects::GetError),
    #[error(transparent)]
    GetUser(#[from] users::GetError),
    #[error(transparent)]
    OpenProjectRepository(#[from] project_repository::OpenError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Controller {
    pub fn upsert(&self, bookmark: &Bookmark) -> Result<Option<Bookmark>, UpsertError> {
        let project = self.projects.get(&bookmark.project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repository")?;
        let writer = Writer::new(&gb_repository).context("failed to open writer")?;
        writer.write(bookmark).context("failed to write bookmark")?;

        self.database
            .upsert(bookmark)
            .context("failed to upsert bookmark")
            .map_err(Into::into)
    }

    pub fn list(
        &self,
        project_id: &ProjectId,
        range: Option<Range<u128>>,
    ) -> Result<Vec<Bookmark>, ListError> {
        self.database
            .list_by_project_id(project_id, range)
            .map_err(Into::into)
    }
}
