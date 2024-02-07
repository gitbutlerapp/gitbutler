use super::{ThreadedResource, ThreadedResourceHandle};
use crate::{Authorization, ConfigScope, RefSpec};
use std::path::{Path, PathBuf};

/// A [`crate::Repository`] implementation using the `git2` crate.
pub struct Repository<R: ThreadedResource> {
    repo: R::Handle<git2::Repository>,
}

impl<R: ThreadedResource> Repository<R> {
    /// Initializes a repository at the given path.
    ///
    /// Errors if the repository is already initialized.
    #[inline]
    pub async fn init<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            repo: R::new(|| {
                git2::Repository::init_opts(
                    path,
                    git2::RepositoryInitOptions::new().no_reinit(true),
                )
            })
            .await?,
        })
    }

    /// Opens a repository at the given path, or initializes it if it doesn't exist.
    #[inline]
    pub async fn open_or_init<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            repo: R::new(|| {
                git2::Repository::init_opts(
                    path,
                    git2::RepositoryInitOptions::new().no_reinit(false),
                )
            })
            .await?,
        })
    }

    /// Initializes a bare repository at the given path.
    ///
    /// Errors if the repository is already initialized.
    #[inline]
    pub async fn init_bare<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            repo: R::new(|| {
                git2::Repository::init_opts(
                    path,
                    git2::RepositoryInitOptions::new()
                        .no_reinit(true)
                        .bare(true),
                )
            })
            .await?,
        })
    }

    /// Opens a repository at the given path, or initializes a new bare repository
    /// if it doesn't exist.
    #[inline]
    pub async fn open_or_init_bare<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            repo: R::new(|| {
                git2::Repository::init_opts(
                    path,
                    git2::RepositoryInitOptions::new()
                        .no_reinit(false)
                        .bare(true),
                )
            })
            .await?,
        })
    }

    /// Opens a repository at the given path.
    /// Will error if there's no existing repository at the given path.
    #[inline]
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<Self, git2::Error> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            repo: R::new(|| git2::Repository::open(path)).await?,
        })
    }
}

impl<R: ThreadedResource> crate::Repository for Repository<R> {
    type Error = git2::Error;

    async fn config_get(
        &self,
        key: &str,
        #[cfg_attr(test, allow(unused_variables))] scope: ConfigScope,
    ) -> Result<Option<String>, crate::Error<Self::Error>> {
        let key = key.to_owned();
        self.repo
            .with(move |repo| {
                let config = repo.config()?;

                #[cfg(test)]
                let scope = ConfigScope::Local;

                // NOTE(qix-): See source comments for ConfigScope to explain
                // NOTE(qix-): the `#[cfg(not(test))]` attributes.
                let res = match scope {
                    #[cfg(not(test))]
                    ConfigScope::Auto => config.get_string(&key),
                    ConfigScope::Local => config
                        .open_level(git2::ConfigLevel::Local)?
                        .get_string(&key),
                    #[cfg(not(test))]
                    ConfigScope::System => config
                        .open_level(git2::ConfigLevel::System)?
                        .get_string(&key),
                    #[cfg(not(test))]
                    ConfigScope::Global => config
                        .open_level(git2::ConfigLevel::Global)?
                        .get_string(&key),
                };

                Ok(res.map(Some).or_else(|e| {
                    if e.code() == git2::ErrorCode::NotFound {
                        Ok(None)
                    } else {
                        Err(e)
                    }
                })?)
            })
            .await
            .await
    }

    async fn config_set(
        &self,
        key: &str,
        value: &str,
        #[cfg_attr(test, allow(unused_variables))] scope: ConfigScope,
    ) -> Result<(), crate::Error<Self::Error>> {
        let key = key.to_owned();
        let value = value.to_owned();

        self.repo
            .with(move |repo| {
                #[cfg_attr(test, allow(unused_mut))]
                let mut config = repo.config()?;

                #[cfg(test)]
                let scope = ConfigScope::Local;

                // NOTE(qix-): See source comments for ConfigScope to explain
                // NOTE(qix-): the `#[cfg(not(test))]` attributes.
                match scope {
                    #[cfg(not(test))]
                    ConfigScope::Auto => Ok(config.set_str(&key, &value)?),
                    ConfigScope::Local => Ok(config
                        .open_level(git2::ConfigLevel::Local)?
                        .set_str(&key, &value)?),
                    #[cfg(not(test))]
                    ConfigScope::System => Ok(config
                        .open_level(git2::ConfigLevel::System)?
                        .set_str(&key, &value)?),
                    #[cfg(not(test))]
                    ConfigScope::Global => Ok(config
                        .open_level(git2::ConfigLevel::Global)?
                        .set_str(&key, &value)?),
                }
            })
            .await
            .await
    }

    async fn fetch(
        &self,
        remote: &str,
        refspec: RefSpec,
        authorization: &Authorization,
    ) -> Result<(), crate::Error<Self::Error>> {
        let remote = remote.to_owned();
        let authorization = authorization.clone();

        self.repo
            .with(move |repo| {
                let mut remote = repo.find_remote(&remote)?;

                let mut callbacks = git2::RemoteCallbacks::new();

                callbacks.credentials(|_url, username, _allowed| {
                    let auth = match &authorization {
                        Authorization::Auto => {
                            let cred = git2::Cred::default()?;
                            Ok(cred)
                        }
                        Authorization::Basic { username, password } => {
                            let username = username.as_deref().unwrap_or_default();
                            let password = password.as_deref().unwrap_or_default();

                            git2::Cred::userpass_plaintext(username, password)
                        }
                        Authorization::Ssh {
                            passphrase,
                            private_key,
                        } => {
                            let private_key =
                                private_key.as_ref().map(PathBuf::from).unwrap_or_else(|| {
                                    let mut path = dirs::home_dir().unwrap();
                                    path.push(".ssh");
                                    path.push("id_rsa");
                                    path
                                });

                            let username = username
                                .map(ToOwned::to_owned)
                                .unwrap_or_else(|| std::env::var("USER").unwrap_or_default());

                            git2::Cred::ssh_key(
                                &username,
                                None,
                                &private_key,
                                passphrase.clone().as_deref(),
                            )
                        }
                    };

                    auth
                });

                let mut fetch_options = git2::FetchOptions::new();
                fetch_options.remote_callbacks(callbacks);

                let refspec = refspec.to_string();

                let r = remote.fetch(&[&refspec], Some(&mut fetch_options), None);

                r.map_err(|e| {
                    if e.code() == git2::ErrorCode::NotFound {
                        crate::Error::RefNotFound(refspec)
                    } else {
                        e.into()
                    }
                })
            })
            .await
            .await
    }

    async fn create_remote(
        &self,
        remote: &str,
        uri: &str,
    ) -> Result<(), crate::Error<Self::Error>> {
        let remote = remote.to_owned();
        let uri = uri.to_owned();

        self.repo
            .with(move |repo| {
                repo.remote(&remote, &uri).map_err(|e| {
                    if e.code() == git2::ErrorCode::Exists {
                        crate::Error::RemoteExists(remote.to_owned(), e)
                    } else {
                        e.into()
                    }
                })?;

                Ok(())
            })
            .await
            .await
    }

    async fn create_or_update_remote(
        &self,
        remote: &str,
        uri: &str,
    ) -> Result<(), crate::Error<Self::Error>> {
        let remote = remote.to_owned();
        let uri = uri.to_owned();

        self.repo
            .with(move |repo| {
                let r = repo
                    .find_remote(&remote)
                    .and_then(|_| repo.remote_set_url(&remote, &uri));

                if let Err(e) = r {
                    if e.code() == git2::ErrorCode::NotFound {
                        repo.remote(&remote, &uri)?;
                    } else {
                        Err(e)?
                    }
                }

                Ok(())
            })
            .await
            .await
    }

    async fn remote(&self, remote: &str) -> Result<String, crate::Error<Self::Error>> {
        let remote = remote.to_owned();

        self.repo
            .with(move |repo| {
                let r = repo.find_remote(&remote);

                let r = match r {
                    Err(e) if e.code() == git2::ErrorCode::NotFound => {
                        return Err(crate::Error::NoSuchRemote(remote, e))?;
                    }
                    Err(e) => {
                        return Err(e)?;
                    }
                    Ok(r) => r,
                };

                let url = r.url().ok_or_else(|| {
                    crate::Error::NoSuchRemote(remote, git2::Error::from_str("remote has no URL"))
                })?;

                Ok(url.to_string())
            })
            .await
            .await
    }
}
