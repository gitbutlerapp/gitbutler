use std::{
    path,
    sync::{atomic::AtomicUsize, Arc},
};

use anyhow::{Context, Result};

use crate::{
    git::{self, credentials::HelpError, Url},
    keys, projects, ssh, users,
    virtual_branches::Branch,
};

use super::conflicts;

pub struct Repository {
    pub git_repository: git::Repository,
    project: projects::Project,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("repository not found at {0}")]
    NotFound(path::PathBuf),
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<OpenError> for crate::error::Error {
    fn from(value: OpenError) -> Self {
        match value {
            OpenError::NotFound(path) => crate::error::Error::UserError {
                code: crate::error::Code::Projects,
                message: format!("{} not found", path.display()),
            },
            OpenError::Other(error) => {
                tracing::error!(?error);
                crate::error::Error::Unknown
            }
        }
    }
}

impl Repository {
    pub fn open(project: &projects::Project) -> Result<Self, OpenError> {
        git::Repository::open(&project.path)
            .map_err(|error| match error {
                git::Error::NotFound(_) => OpenError::NotFound(project.path.clone()),
                other => OpenError::Other(other.into()),
            })
            .map(|git_repository| Self {
                git_repository,
                project: project.clone(),
            })
    }

    pub fn is_resolving(&self) -> bool {
        conflicts::is_resolving(self)
    }

    pub fn path(&self) -> &path::Path {
        path::Path::new(&self.project.path)
    }

    pub fn config(&self) -> super::Config {
        super::Config::from(&self.git_repository)
    }

    pub fn git_signatures<'a>(
        &self,
        user: Option<&users::User>,
    ) -> Result<(git::Signature<'a>, git::Signature<'a>)> {
        super::signatures::signatures(self, user).context("failed to get signatures")
    }

    pub fn project(&self) -> &projects::Project {
        &self.project
    }

    pub fn set_project(&mut self, project: &projects::Project) {
        self.project = project.clone();
    }

    pub fn get_head(&self) -> Result<git::Reference, git::Error> {
        let head = self.git_repository.head()?;
        Ok(head)
    }

    pub fn get_wd_tree(&self) -> Result<git::Tree> {
        let tree = self.git_repository.get_wd_tree()?;
        Ok(tree)
    }

    pub fn is_path_ignored<P: AsRef<std::path::Path>>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        let ignored = self.git_repository.is_path_ignored(path)?;
        Ok(ignored)
    }

    pub fn root(&self) -> &std::path::Path {
        self.git_repository.path().parent().unwrap()
    }

    pub fn git_remote_branches(&self) -> Result<Vec<git::RemoteRefname>> {
        self.git_repository
            .branches(Some(git2::BranchType::Remote))?
            .flatten()
            .map(|(branch, _)| branch)
            .map(|branch| {
                git::RemoteRefname::try_from(&branch)
                    .context("failed to convert branch to remote name")
            })
            .collect::<Result<Vec<_>>>()
    }

    pub fn add_branch_reference(&self, branch: &Branch) -> Result<()> {
        let (should_write, with_force) =
            match self.git_repository.find_reference(&branch.refname().into()) {
                Ok(reference) => match reference.target() {
                    Some(head_oid) => Ok((head_oid != branch.head, true)),
                    None => Ok((true, true)),
                },
                Err(git::Error::NotFound(_)) => Ok((true, false)),
                Err(error) => Err(error),
            }
            .context("failed to lookup reference")?;

        if should_write {
            self.git_repository
                .reference(
                    &branch.refname().into(),
                    branch.head,
                    with_force,
                    "new vbranch",
                )
                .context("failed to create branch reference")?;
        }

        Ok(())
    }

    pub fn delete_branch_reference(&self, branch: &Branch) -> Result<()> {
        match self.git_repository.find_reference(&branch.refname().into()) {
            Ok(mut reference) => {
                reference
                    .delete()
                    .context("failed to delete branch reference")?;
                Ok(())
            }
            Err(git::Error::NotFound(_)) => Ok(()),
            Err(error) => Err(error),
        }
        .context("failed to lookup reference")
    }

    // returns a list of commit oids from the first oid to the second oid
    pub fn l(&self, from: git::Oid, to: LogUntil) -> Result<Vec<git::Oid>> {
        match to {
            LogUntil::Commit(oid) => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from.into())
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .hide(oid.into())
                    .context(format!("failed to hide {}", oid))?;
                revwalk
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
            LogUntil::Take(n) => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from.into())
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .take(n)
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
            LogUntil::When(cond) => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from.into())
                    .context(format!("failed to push {}", from))?;
                let mut oids: Vec<git::Oid> = vec![];
                for oid in revwalk {
                    let oid = oid.context("failed to get oid")?;
                    oids.push(oid.into());

                    let commit = self
                        .git_repository
                        .find_commit(oid.into())
                        .context("failed to find commit")?;

                    if cond(&commit).context("failed to check condition")? {
                        break;
                    }
                }
                Ok(oids)
            }
            #[cfg(test)]
            LogUntil::End => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from.into())
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
        }
        .context("failed to collect oids")
    }

    // returns a list of commits from the first oid to the second oid
    pub fn log(&self, from: git::Oid, to: LogUntil) -> Result<Vec<git::Commit>> {
        self.l(from, to)?
            .into_iter()
            .map(|oid| self.git_repository.find_commit(oid))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to collect commits")
    }

    // returns the number of commits between the first oid to the second oid
    pub fn distance(&self, from: git::Oid, to: git::Oid) -> Result<u32> {
        let oids = self.l(from, LogUntil::Commit(to))?;
        Ok(oids.len().try_into()?)
    }

    pub fn commit(
        &self,
        user: Option<&users::User>,
        message: &str,
        tree: &git::Tree,
        parents: &[&git::Commit],
        signing_key: Option<&keys::PrivateKey>,
    ) -> Result<git::Oid> {
        let (author, committer) = self.git_signatures(user)?;
        if let Some(key) = signing_key {
            self.git_repository
                .commit_signed(&author, message, tree, parents, key)
                .context("failed to commit signed")
        } else {
            self.git_repository
                .commit(None, &author, &committer, message, tree, parents)
                .context("failed to commit")
        }
    }

    pub fn push_to_gitbutler_server(
        &self,
        user: Option<&users::User>,
        ref_specs: &[&str],
    ) -> Result<bool, RemoteError> {
        let url = self
            .project
            .api
            .as_ref()
            .ok_or(RemoteError::Other(anyhow::anyhow!("api not set")))?
            .code_git_url
            .as_ref()
            .ok_or(RemoteError::Other(anyhow::anyhow!("code_git_url not set")))?
            .as_str()
            .parse::<Url>()
            .map_err(|e| RemoteError::Other(e.into()))?;

        tracing::debug!(
            project_id = %self.project.id,
            %url,
            "pushing code to gb repo",
        );

        let access_token = user
            .map(|user| user.access_token.clone())
            .ok_or(RemoteError::Auth)?;

        let mut callbacks = git2::RemoteCallbacks::new();
        if self.project.omit_certificate_check.unwrap_or(false) {
            callbacks.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
        }
        let bytes_pushed = Arc::new(AtomicUsize::new(0));
        let total_objects = Arc::new(AtomicUsize::new(0));
        {
            let byte_counter = Arc::<AtomicUsize>::clone(&bytes_pushed);
            let total_counter = Arc::<AtomicUsize>::clone(&total_objects);
            callbacks.push_transfer_progress(move |_current, total, bytes| {
                byte_counter.store(bytes, std::sync::atomic::Ordering::Relaxed);
                total_counter.store(total, std::sync::atomic::Ordering::Relaxed);
            });
        }

        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);
        let auth_header = format!("Authorization: {}", access_token);
        let headers = &[auth_header.as_str()];
        push_options.custom_headers(headers);

        let mut remote = self
            .git_repository
            .remote_anonymous(&url)
            .map_err(|e| RemoteError::Other(e.into()))?;

        remote
            .push(ref_specs, Some(&mut push_options))
            .map_err(|error| match error {
                git::Error::Network(error) => {
                    tracing::warn!(project_id = %self.project.id, ?error, "git push failed",);
                    RemoteError::Network
                }
                git::Error::Auth(error) => {
                    tracing::warn!(project_id = %self.project.id, ?error, "git push failed",);
                    RemoteError::Auth
                }
                error => RemoteError::Other(error.into()),
            })?;

        let bytes_pushed = bytes_pushed.load(std::sync::atomic::Ordering::Relaxed);
        let total_objects_pushed = total_objects.load(std::sync::atomic::Ordering::Relaxed);

        tracing::debug!(
            project_id = %self.project.id,
            ref_spec = ref_specs.join(" "),
            bytes = bytes_pushed,
            objects = total_objects_pushed,
            "pushed to gb repo tmp ref",
        );

        Ok(total_objects_pushed > 0)
    }

    pub fn push(
        &self,
        head: &git::Oid,
        branch: &git::RemoteRefname,
        with_force: bool,
        credentials: &git::credentials::Helper,
    ) -> Result<(), RemoteError> {
        let refspec = if with_force {
            format!("+{}:refs/heads/{}", head, branch.branch())
        } else {
            format!("{}:refs/heads/{}", head, branch.branch())
        };

        let auth_flows = credentials.help(self, branch.remote())?;
        for (mut remote, callbacks) in auth_flows {
            if let Some(url) = remote.url().context("failed to get remote url")? {
                if !self.project.omit_certificate_check.unwrap_or(false) {
                    ssh::check_known_host(&url).context("failed to check known host")?;
                }
            }
            for callback in callbacks {
                let mut cbs: git2::RemoteCallbacks = callback.into();
                if self.project.omit_certificate_check.unwrap_or(false) {
                    cbs.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
                }
                cbs.push_update_reference(|_reference: &str, status: Option<&str>| {
                    if let Some(status) = status {
                        return Err(git2::Error::from_str(status));
                    };
                    Ok(())
                });

                match remote.push(
                    &[refspec.as_str()],
                    Some(&mut git2::PushOptions::new().remote_callbacks(cbs)),
                ) {
                    Ok(()) => {
                        tracing::info!(
                            project_id = %self.project.id,
                            remote = %branch.remote(),
                            %head,
                            branch = branch.branch(),
                            "pushed git branch"
                        );
                        return Ok(());
                    }
                    Err(git::Error::Auth(error) | git::Error::Http(error)) => {
                        tracing::warn!(project_id = %self.project.id, ?error, "git push failed");
                        continue;
                    }
                    Err(git::Error::Network(error)) => {
                        tracing::warn!(project_id = %self.project.id, ?error, "git push failed");
                        return Err(RemoteError::Network);
                    }
                    Err(error) => return Err(RemoteError::Other(error.into())),
                }
            }
        }

        Err(RemoteError::Auth)
    }

    pub fn fetch(
        &self,
        remote_name: &str,
        credentials: &git::credentials::Helper,
    ) -> Result<(), RemoteError> {
        let refspec = &format!("+refs/heads/*:refs/remotes/{}/*", remote_name);
        let auth_flows = credentials.help(self, remote_name)?;
        for (mut remote, callbacks) in auth_flows {
            if let Some(url) = remote.url().context("failed to get remote url")? {
                if !self.project.omit_certificate_check.unwrap_or(false) {
                    ssh::check_known_host(&url).context("failed to check known host")?;
                }
            }
            for callback in callbacks {
                let mut fetch_opts = git2::FetchOptions::new();
                let mut cbs: git2::RemoteCallbacks = callback.into();
                if self.project.omit_certificate_check.unwrap_or(false) {
                    cbs.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
                }
                fetch_opts.remote_callbacks(cbs);
                fetch_opts.prune(git2::FetchPrune::On);

                match remote.fetch(&[refspec], Some(&mut fetch_opts)) {
                    Ok(()) => {
                        tracing::info!(project_id = %self.project.id, %refspec, "git fetched");
                        return Ok(());
                    }
                    Err(git::Error::Auth(error) | git::Error::Http(error)) => {
                        tracing::warn!(project_id = %self.project.id, ?error, "fetch failed");
                        continue;
                    }
                    Err(git::Error::Network(error)) => {
                        tracing::warn!(project_id = %self.project.id, ?error, "fetch failed");
                        return Err(RemoteError::Network);
                    }
                    Err(error) => return Err(RemoteError::Other(error.into())),
                }
            }
        }

        Err(RemoteError::Auth)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error(transparent)]
    Help(#[from] HelpError),
    #[error("network failed")]
    Network,
    #[error("authentication failed")]
    Auth,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<RemoteError> for crate::error::Error {
    fn from(value: RemoteError) -> Self {
        match value {
            RemoteError::Help(error) => error.into(),
            RemoteError::Network => crate::error::Error::UserError {
                code: crate::error::Code::ProjectGitRemote,
                message: "Network erorr occured".to_string(),
            },
            RemoteError::Auth => crate::error::Error::UserError {
                code: crate::error::Code::ProjectGitAuth,
                message: "Project remote authentication error".to_string(),
            },
            RemoteError::Other(error) => {
                tracing::error!(?error);
                crate::error::Error::Unknown
            }
        }
    }
}

type OidFilter = dyn Fn(&git::Commit) -> Result<bool>;

pub enum LogUntil {
    Commit(git::Oid),
    Take(usize),
    When(Box<OidFilter>),
    #[cfg(test)]
    End,
}
