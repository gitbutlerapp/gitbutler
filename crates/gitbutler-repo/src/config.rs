use anyhow::Result;

pub struct Config<'a> {
    git_repo: &'a git2::Repository,
}

impl<'a> From<&'a git2::Repository> for Config<'a> {
    fn from(value: &'a git2::Repository) -> Self {
        Self { git_repo: value }
    }
}

// TODO: Remove this in favor of gitbutler-core::config::git::GitConfig
impl Config<'_> {
    pub fn user_real_comitter(&self) -> Result<bool> {
        let gb_comitter = self
            .get_string("gitbutler.gitbutlerCommitter")
            .unwrap_or(Some("0".to_string()))
            .unwrap_or("0".to_string());
        Ok(gb_comitter == "0")
    }

    fn get_string(&self, key: &str) -> Result<Option<String>> {
        let config = self.git_repo.config()?;
        match config.get_string(key) {
            Ok(value) => Ok(Some(value)),
            Err(err) => match err.code() {
                git2::ErrorCode::NotFound => Ok(None),
                _ => Err(err.into()),
            },
        }
    }
}
