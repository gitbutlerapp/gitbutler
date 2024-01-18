use std::path::Path;

use crate::ConfigScope;

/// A [`crate::Repository`] implementation using the `git2` crate.
pub struct Repository {
    repo: git2::Repository,
}

impl Repository {
    /// Initializes a repository at the given path.
    ///
    /// Errors if the repository is already initialized.
    #[inline]
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        Ok(Self {
            repo: git2::Repository::init_opts(
                path,
                git2::RepositoryInitOptions::new().no_reinit(true),
            )?,
        })
    }

    /// Opens a repository at the given path, or initializes it if it doesn't exist.
    #[inline]
    pub fn open_or_init<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        Ok(Self {
            repo: git2::Repository::init_opts(
                path,
                git2::RepositoryInitOptions::new().no_reinit(false),
            )?,
        })
    }

    /// Initializes a bare repository at the given path.
    ///
    /// Errors if the repository is already initialized.
    #[inline]
    pub fn init_bare<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        Ok(Self {
            repo: git2::Repository::init_opts(
                path,
                git2::RepositoryInitOptions::new()
                    .no_reinit(true)
                    .bare(true),
            )?,
        })
    }

    /// Opens a repository at the given path, or initializes a new bare repository
    /// if it doesn't exist.
    #[inline]
    pub fn open_or_init_bare<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        Ok(Self {
            repo: git2::Repository::init_opts(
                path,
                git2::RepositoryInitOptions::new()
                    .no_reinit(false)
                    .bare(true),
            )?,
        })
    }

    /// Opens a repository at the given path.
    /// Will error if there's no existing repository at the given path.
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
        #[cfg_attr(test, allow(unused_variables))] scope: ConfigScope,
    ) -> Result<Option<String>, Self::Error> {
        let config = self.repo.config()?;

        #[cfg(test)]
        let scope = ConfigScope::Local;

        // NOTE(qix-): See source comments for ConfigScope to explain
        // NOTE(qix-): the `#[cfg(not(test))]` attributes.
        let res = match scope {
            #[cfg(not(test))]
            ConfigScope::Auto => config.get_string(key),
            ConfigScope::Local => config.open_level(git2::ConfigLevel::Local)?.get_string(key),
            #[cfg(not(test))]
            ConfigScope::System => config
                .open_level(git2::ConfigLevel::System)?
                .get_string(key),
            #[cfg(not(test))]
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
        #[cfg_attr(test, allow(unused_variables))] scope: ConfigScope,
    ) -> Result<(), Self::Error> {
        #[cfg_attr(test, allow(unused_mut))]
        let mut config = self.repo.config()?;

        #[cfg(test)]
        let scope = ConfigScope::Local;

        // NOTE(qix-): See source comments for ConfigScope to explain
        // NOTE(qix-): the `#[cfg(not(test))]` attributes.
        match scope {
            #[cfg(not(test))]
            ConfigScope::Auto => config.set_str(key, value),
            ConfigScope::Local => config
                .open_level(git2::ConfigLevel::Local)?
                .set_str(key, value),
            #[cfg(not(test))]
            ConfigScope::System => config
                .open_level(git2::ConfigLevel::System)?
                .set_str(key, value),
            #[cfg(not(test))]
            ConfigScope::Global => config
                .open_level(git2::ConfigLevel::Global)?
                .set_str(key, value),
        }
    }
}
