use crate::git;

pub struct Config<'a> {
    git_repository: &'a git::Repository,
}

impl<'a> From<&'a git::Repository> for Config<'a> {
    fn from(value: &'a git::Repository) -> Self {
        Self {
            git_repository: value,
        }
    }
}

impl Config<'_> {
    pub fn sign_commits(&self) -> Result<bool, git::Error> {
        let sign_commits = self
            .git_repository
            .config()?
            .get_bool("gitbutler.signCommits")
            .unwrap_or(Some(false))
            .unwrap_or(false);
        Ok(sign_commits)
    }

    pub fn user_real_comitter(&self) -> Result<bool, git::Error> {
        let gb_comitter = self
            .git_repository
            .config()?
            .get_string("gitbutler.gitbutlerCommitter")
            .unwrap_or(Some("0".to_string()))
            .unwrap_or("0".to_string());
        Ok(gb_comitter == "0")
    }

    pub fn user_name(&self) -> Result<Option<String>, git::Error> {
        self.git_repository.config()?.get_string("user.name")
    }

    pub fn user_email(&self) -> Result<Option<String>, git::Error> {
        self.git_repository.config()?.get_string("user.email")
    }
}
