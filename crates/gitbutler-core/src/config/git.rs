use crate::projects::Project;
use anyhow::Result;
use git2::ConfigLevel;

use super::CFG_SIGN_COMMITS;

impl Project {
    pub fn set_sign_commits_config(&self, val: bool) -> Result<()> {
        self.set_bool_to_repo_config(CFG_SIGN_COMMITS, val)
    }
    pub fn should_sign_commits(&self) -> Result<Option<bool>> {
        self.bool_from_git_config(CFG_SIGN_COMMITS)
    }

    fn set_bool_to_repo_config(&self, key: &str, val: bool) -> Result<()> {
        // TODO(ST): make a nice API to actually write changes back. Right now one would have to use plumbing.
        let repo = git2::Repository::open(&self.path)?;
        let config = repo.config()?;
        match config.open_level(ConfigLevel::Local) {
            Ok(mut local) => local.set_bool(key, val).map_err(Into::into),
            Err(err) => Err(err.into()),
        }
    }

    fn bool_from_git_config(&self, key: &str) -> Result<Option<bool>> {
        let repo = gix::open(&self.path)?;
        repo.config_snapshot()
            .try_boolean(key)
            .transpose()
            .map_err(Into::into)
    }
}
