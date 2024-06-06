use anyhow::Result;

pub struct Config<'a> {
    git_repository: &'a git2::Repository,
}

impl<'a> From<&'a git2::Repository> for Config<'a> {
    fn from(value: &'a git2::Repository) -> Self {
        Self {
            git_repository: value,
        }
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

    pub fn user_name(&self) -> Result<Option<String>> {
        self.get_string("user.name").map_err(Into::into)
    }

    pub fn user_email(&self) -> Result<Option<String>> {
        self.get_string("user.email").map_err(Into::into)
    }

    pub fn set_local(&self, key: &str, val: &str) -> Result<()> {
        let config = self.git_repository.config()?;
        match config.open_level(git2::ConfigLevel::Local) {
            Ok(mut local) => local.set_str(key, val).map_err(Into::into),
            Err(err) => Err(err.into()),
        }
    }

    pub fn get_local(&self, key: &str) -> Result<Option<String>> {
        let config = self.git_repository.config()?;
        match config
            .open_level(git2::ConfigLevel::Local)
            .and_then(|local| local.get_string(key))
        {
            Ok(value) => Ok(Some(value)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_string(&self, key: &str) -> Result<Option<String>> {
        let config = self.git_repository.config()?;
        match config.get_string(key) {
            Ok(value) => Ok(Some(value)),
            Err(err) => match err.code() {
                git2::ErrorCode::NotFound => Ok(None),
                _ => Err(err.into()),
            },
        }
    }
}
