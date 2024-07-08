use anyhow::Result;
use bstr::BString;
use gitbutler_core::{
    git::RepositoryExt,
    project_repository::{self, Config},
    projects::Project,
};

pub trait RepoCommands {
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;
}

impl RepoCommands for Project {
    fn get_local_config(&self, key: &str) -> Result<Option<String>> {
        let project_repo = project_repository::ProjectRepo::open(self)?;
        let config: Config = project_repo.repo().into();
        config.get_local(key)
    }

    fn set_local_config(&self, key: &str, value: &str) -> Result<()> {
        let project_repo = project_repository::ProjectRepo::open(self)?;
        let config: Config = project_repo.repo().into();
        config.set_local(key, value)
    }

    fn check_signing_settings(&self) -> Result<bool> {
        let repo = project_repository::ProjectRepo::open(self)?;
        let signed = repo.repo().sign_buffer(&BString::new("test".into()).into());
        match signed {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }
}
