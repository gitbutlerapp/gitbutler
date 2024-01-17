use std::path::Path;

use crate::ConfigScope;

/// A [`crate::Repository`] implementation using the `git2` crate.
pub struct Repository {
    repo: git2::Repository,
}

impl Repository {
    /// Opens a repository at the given path.
    #[inline]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        Ok(Self {
            repo: git2::Repository::open(path)?,
        })
    }
}

impl crate::Repository for Repository {
    type Error = git2::Error;

    async fn config_get(
        &self,
        key: &str,
        scope: ConfigScope,
    ) -> Result<Option<String>, Self::Error> {
        let config = self.repo.config()?;

        let res = match scope {
            ConfigScope::Auto => config.get_string(key),
            ConfigScope::Local => config.open_level(git2::ConfigLevel::Local)?.get_string(key),
            ConfigScope::System => config
                .open_level(git2::ConfigLevel::System)?
                .get_string(key),
            ConfigScope::Global => config
                .open_level(git2::ConfigLevel::Global)?
                .get_string(key),
        };

        res.map(Some).or_else(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                Ok(None)
            } else {
                Err(e)
            }
        })
    }

    async fn config_set(
        &self,
        key: &str,
        value: &str,
        scope: ConfigScope,
    ) -> Result<(), Self::Error> {
        let mut config = self.repo.config()?;

        match scope {
            ConfigScope::Auto => config.set_str(key, value),
            ConfigScope::Local => config
                .open_level(git2::ConfigLevel::Local)?
                .set_str(key, value),
            ConfigScope::System => config
                .open_level(git2::ConfigLevel::System)?
                .set_str(key, value),
            ConfigScope::Global => config
                .open_level(git2::ConfigLevel::Global)?
                .set_str(key, value),
        }
    }
}
