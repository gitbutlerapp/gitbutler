use crate::projects::Project;
use anyhow::Result;
use git2::ConfigLevel;

const CFG_SIGN_COMMITS: &str = "gitbutler.signCommits";

impl Project {
    pub fn set_sign_commits(&self, val: bool) -> Result<()> {
        self.set_local_bool(CFG_SIGN_COMMITS, val)
    }
    pub fn sign_commits(&self) -> Result<Option<bool>> {
        self.get_bool(CFG_SIGN_COMMITS)
    }

    fn set_local_bool(&self, key: &str, val: bool) -> Result<()> {
        let repo = git2::Repository::open(&self.path)?;
        let config = repo.config()?;
        match config.open_level(ConfigLevel::Local) {
            Ok(mut local) => local.set_bool(key, val).map_err(Into::into),
            Err(err) => Err(err.into()),
        }
    }

    fn get_bool(&self, key: &str) -> Result<Option<bool>> {
        let repo = git2::Repository::open(&self.path)?;
        let config = repo.config()?;
        match config.get_bool(key) {
            Ok(value) => Ok(Some(value)),
            Err(err) => match err.code() {
                git2::ErrorCode::NotFound => Ok(None),
                _ => Err(err.into()),
            },
        }
    }
}
