use std::{collections::HashMap, path, sync::Arc};

use anyhow::Context;
use futures::future::join_all;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::{
    assets, gb_repository, git, keys,
    project_repository::{self, conflicts},
    projects, users, watcher,
};

use super::{branch::Ownership, RemoteBranchFile};

pub struct Controller {
    local_data_dir: path::PathBuf,
    semaphores: Arc<tokio::sync::Mutex<HashMap<String, Semaphore>>>,

    watchers: watcher::Watchers,
    assets_proxy: assets::Proxy,
    projects_storage: projects::Storage,
    users_storage: users::Storage,
    keys_storage: keys::Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("detached head detected, go back to gitbutler/integration to continue")]
    DetachedHead,
    #[error("unexpected head {0}, go back to gitbutler/integration to continue")]
    InvalidHead(String),
    #[error("integration commit not found")]
    NoIntegrationCommit,
    #[error("failed to open project repository")]
    PushError(#[from] project_repository::Error),
    #[error("project is in a conflicted state")]
    Conflicting,
    #[error(transparent)]
    LockError(#[from] tokio::sync::AcquireError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl TryFrom<&AppHandle> for Controller {
    type Error = Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("Failed to get local data dir")?;
        Ok(Self {
            local_data_dir,
            watchers: value.state::<watcher::Watchers>().inner().clone(),
            semaphores: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            assets_proxy: assets::Proxy::try_from(value)?,
            projects_storage: projects::Storage::from(value),
            users_storage: users::Storage::from(value),
            keys_storage: keys::Storage::from(value),
        })
    }
}

impl Controller {
    pub async fn create_commit(
        &self,
        project_id: &str,
        branch: &str,
        message: &str,
        ownership: Option<&Ownership>,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let signing_key = if project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                {
                    Some(
                        self.keys_storage
                            .get_or_create()
                            .context("failed to get private key")?,
                    )
                } else {
                    None
                };
                super::commit(
                    gb_repository,
                    project_repository,
                    branch,
                    message,
                    ownership,
                    signing_key.as_ref(),
                    user,
                )?;
                Ok(())
            })
        })
        .await
    }

    pub async fn can_apply_remote_branch(
        &self,
        project_id: &str,
        branch_name: &git::BranchName,
    ) -> Result<bool, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let user = self.users_storage.get().context("failed to get user")?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gitbutler repository")
                .map_err(Error::Other)?;
        super::is_remote_branch_mergeable(&gb_repository, &project_repository, branch_name)
            .map_err(Error::Other)
    }

    pub async fn can_apply_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<bool, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let user = self.users_storage.get().context("failed to get user")?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gitbutler repository")
                .map_err(Error::Other)?;
        super::is_virtual_branch_mergeable(&gb_repository, &project_repository, branch_id)
            .map_err(Error::Other)
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &str,
    ) -> Result<Vec<super::VirtualBranch>, Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::list_virtual_branches(gb_repository, project_repository)
                    .map_err(Error::Other)
            })
        })
        .await
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &str,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                if conflicts::is_resolving(project_repository) {
                    return Err(Error::Conflicting);
                }
                super::create_virtual_branch(gb_repository, create).map_err(Error::Other)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &str,
        branch: &git::BranchName,
    ) -> Result<String, Error> {
        self.with_lock::<Result<String, Error>>(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let branch = super::create_virtual_branch_from_branch(
                    gb_repository,
                    project_repository,
                    branch,
                    None,
                    user,
                )
                .map_err(Error::Other)?;

                let signing_key = if project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                {
                    Some(
                        self.keys_storage
                            .get_or_create()
                            .context("failed to get private key")?,
                    )
                } else {
                    None
                };

                // also apply the branch
                super::apply_branch(
                    gb_repository,
                    project_repository,
                    &branch.id,
                    signing_key.as_ref(),
                    user,
                )
                .map_err(Error::Other)?;
                Ok(branch.id)
            })
        })
        .await
    }

    pub async fn get_base_branch_data(
        &self,
        project_id: &str,
    ) -> Result<Option<super::BaseBranch>, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let user = self.users_storage.get().context("failed to get user")?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gitbutler repository")
                .map_err(Error::Other)?;
        let base_branch = super::get_base_branch_data(&gb_repository, &project_repository)?;
        if let Some(branch) = base_branch {
            Ok(Some(self.proxy_base_branch(branch).await))
        } else {
            Ok(None)
        }
    }

    pub async fn list_remote_commit_files(
        &self,
        project_id: &str,
        commit_oid: git::Oid,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let commit = project_repository
            .git_repository
            .find_commit(commit_oid)
            .context("failed to find commit")?;
        super::list_remote_commit_files(&project_repository.git_repository, &commit)
            .map_err(Error::Other)
    }

    pub async fn set_base_branch(
        &self,
        project_id: &str,
        target_branch: &git::RemoteBranchName,
    ) -> Result<super::BaseBranch, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        let user = self.users_storage.get().context("failed to get user")?;

        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;

        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gitbutler repository")
                .map_err(Error::Other)?;
        let target = super::set_base_branch(
            &gb_repository,
            &project_repository,
            user.as_ref(),
            target_branch,
        )
        .map_err(Error::Other)?;
        let current_session = gb_repository.get_current_session()?;

        if let Some(session) = current_session {
            self.watchers
                .post(watcher::Event::Session(project_id.to_string(), session))
                .await?;
        }

        let target = self.proxy_base_branch(target).await;

        Ok(target)
    }

    pub async fn merge_virtual_branch_upstream(
        &self,
        project_id: &str,
        branch: &str,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let signing_key = if project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                {
                    Some(
                        self.keys_storage
                            .get_or_create()
                            .context("failed to get private key")?,
                    )
                } else {
                    None
                };
                super::merge_virtual_branch_upstream(
                    gb_repository,
                    project_repository,
                    branch,
                    signing_key.as_ref(),
                    user,
                )
                .map_err(Error::Other)
            })
        })
        .await
    }

    pub async fn update_base_branch(&self, project_id: &str) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                super::update_base_branch(gb_repository, project_repository, user)
                    .map_err(Error::Other)
            })
        })
        .await
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &str,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::update_branch(gb_repository, project_repository, branch_update)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::delete_branch(gb_repository, project_repository, branch_id)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let signing_key = if project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                {
                    Some(
                        self.keys_storage
                            .get_or_create()
                            .context("failed to get private key")?,
                    )
                } else {
                    None
                };
                super::apply_branch(
                    gb_repository,
                    project_repository,
                    branch_id,
                    signing_key.as_ref(),
                    user,
                )
                .map_err(Error::Other)
            })
        })
        .await
    }

    pub async fn unapply_ownership(
        &self,
        project_id: &str,
        ownership: &Ownership,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::unapply_ownership(gb_repository, project_repository, ownership)
                    .map_err(Error::Other)
            })
        })
        .await
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::unapply_branch(gb_repository, project_repository, branch_id)
                    .map_err(Error::Other)
            })
        })
        .await
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                let private_key = match &project_repository.project().preferred_key {
                    projects::AuthKey::Local {
                        private_key_path,
                        passphrase,
                    } => keys::Key::Local {
                        private_key_path: private_key_path.clone(),
                        passphrase: passphrase.clone(),
                    },
                    projects::AuthKey::Generated => {
                        let private_key = self
                            .keys_storage
                            .get_or_create()
                            .context("failed to get or create private key")?;
                        keys::Key::Generated(Box::new(private_key))
                    }
                };

                super::push(project_repository, gb_repository, branch_id, &private_key).map_err(
                    |e| match e {
                        super::PushError::Repository(e) => Error::PushError(e),
                        super::PushError::Other(e) => Error::Other(e),
                    },
                )
            })
        })
        .await
    }

    fn with_verify_branch<T>(
        &self,
        project_id: &str,
        action: impl FnOnce(
            &gb_repository::Repository,
            &project_repository::Repository,
            Option<&users::User>,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let user = self.users_storage.get().context("failed to get user")?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gitbutler repository")
                .map_err(Error::Other)?;
        super::integration::verify_branch(&gb_repository, &project_repository).map_err(
            |e| match e {
                super::integration::VerifyError::DetachedHead => Error::DetachedHead,
                super::integration::VerifyError::InvalidHead(head) => Error::InvalidHead(head),
                super::integration::VerifyError::NoIntegrationCommit => Error::NoIntegrationCommit,
                e => Error::Other(anyhow::Error::from(e)),
            },
        )?;
        action(&gb_repository, &project_repository, user.as_ref())
    }

    async fn with_lock<T>(&self, project_id: &str, action: impl FnOnce() -> T) -> T {
        let mut semaphores = self.semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await;
        action()
    }

    async fn proxy_base_branch(&self, target: super::BaseBranch) -> super::BaseBranch {
        super::BaseBranch {
            recent_commits: join_all(
                target
                    .clone()
                    .recent_commits
                    .into_iter()
                    .map(|commit| async move {
                        super::RemoteCommit {
                            author: super::Author {
                                gravatar_url: self
                                    .assets_proxy
                                    .proxy(&commit.author.gravatar_url)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(gravatar_url = %commit.author.gravatar_url, ?error, "failed to proxy gravatar url");
                                        commit.author.gravatar_url
                                    }),
                                ..commit.author
                            },
                            ..commit
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .await,
            upstream_commits: join_all(
                target
                    .clone()
                    .upstream_commits
                    .into_iter()
                    .map(|commit| async move {
                        super::RemoteCommit {
                            author: super::Author {
                                gravatar_url: self
                                    .assets_proxy
                                    .proxy(&commit.author.gravatar_url)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(gravatar_url = %commit.author.gravatar_url, ?error, "failed to proxy gravatar url");
                                        commit.author.gravatar_url
                                    }),
                                ..commit.author
                            },
                            ..commit
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .await,
            ..target
        }
    }
}
