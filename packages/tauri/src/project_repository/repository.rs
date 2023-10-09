use std::path;

use anyhow::{Context, Result};

use crate::{git, keys, projects, reader, users};

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

impl TryFrom<projects::Project> for Repository {
    type Error = OpenError;

    fn try_from(project: projects::Project) -> Result<Self, Self::Error> {
        let git_repository = git::Repository::open(&project.path).map_err(|error| match error {
            git::Error::NotFound(_) => OpenError::NotFound(project.path.clone()),
            other => OpenError::Other(other.into()),
        })?;
        Ok(Self {
            git_repository,
            project,
        })
    }
}

impl TryFrom<&projects::Project> for Repository {
    type Error = OpenError;

    fn try_from(project: &projects::Project) -> Result<Self, Self::Error> {
        Self::try_from(project.clone())
    }
}

impl Repository {
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

    pub fn get_wd_reader(&self) -> reader::DirReader {
        reader::DirReader::open(self.root().to_path_buf())
    }

    pub fn root(&self) -> &std::path::Path {
        self.git_repository.path().parent().unwrap()
    }

    pub fn git_remote_branches(&self) -> Result<Vec<git::RemoteBranchName>> {
        self.git_repository
            .branches(Some(git2::BranchType::Remote))?
            .flatten()
            .map(|(branch, _)| branch)
            .map(|branch| {
                git::RemoteBranchName::try_from(&branch)
                    .context("failed to convert branch to remote name")
            })
            .collect::<Result<Vec<_>>>()
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
                    .context(format!("failed to push {}", oid))?;
                revwalk
                    .map(|oid| oid.map(|oid| oid.into()))
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
                    .map(|oid| oid.map(|oid| oid.into()))
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
            LogUntil::End => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from.into())
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .map(|oid| oid.map(|oid| oid.into()))
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

    // returns a remote and makes sure that the push url is an ssh url
    // if url is already ssh, or not set at all, then it returns the remote as is.
    fn get_remote(&self, name: &str) -> Result<git::Remote, RemoteError> {
        let remote = self
            .git_repository
            .find_remote(name)
            .context("failed to find remote")
            .map_err(RemoteError::Other)?;

        if let Ok(Some(url)) = remote.url() {
            match url.as_ssh() {
                Ok(ssh_url) => Ok(self
                    .git_repository
                    .remote_anonymous(ssh_url)
                    .context("failed to get anonymous")
                    .map_err(RemoteError::Other)?),
                Err(_) => Err(RemoteError::NonSSHUrl(url.to_string())),
            }
        } else {
            Err(RemoteError::NoUrl)
        }
    }

    pub fn push(
        &self,
        head: &git::Oid,
        branch: &git::RemoteBranchName,
        key: &keys::Key,
    ) -> Result<(), RemoteError> {
        let mut remote = self.get_remote(branch.remote())?;

        for credential_callback in git::credentials::for_key(key) {
            let mut remote_callbacks = git2::RemoteCallbacks::new();
            remote_callbacks.credentials(credential_callback);

            match remote.push(
                &[&format!("{}:refs/heads/{}", head, branch.branch())],
                Some(&mut git2::PushOptions::new().remote_callbacks(remote_callbacks)),
            ) {
                Ok(()) => {
                    tracing::info!(
                        project_id = self.project.id,
                        remote = %branch.remote(),
                        %head,
                        branch = branch.branch(),
                        "pushed git branch"
                    );
                    return Ok(());
                }
                Err(error) => {
                    tracing::error!(project_id = self.project.id, ?error, "git push failed",);
                    continue;
                }
            }
        }

        Err(RemoteError::AuthError)
    }

    pub fn fetch(&self, remote_name: &str, key: &keys::Key) -> Result<(), RemoteError> {
        let mut remote = self.get_remote(remote_name)?;

        for credential_callback in git::credentials::for_key(key) {
            let mut remote_callbacks = git2::RemoteCallbacks::new();
            remote_callbacks.credentials(credential_callback);
            remote_callbacks.push_update_reference(|refname, message| {
                if let Some(msg) = message {
                    tracing::debug!(
                        project_id = self.project.id,
                        refname,
                        msg,
                        "push update reference",
                    );
                }
                Ok(())
            });
            remote_callbacks.push_negotiation(|proposals| {
                tracing::debug!(
                    project_id = self.project.id,
                    proposals = proposals
                        .iter()
                        .map(|p| format!(
                            "src_refname: {}, dst_refname: {}",
                            p.src_refname().unwrap_or(&p.src().to_string()),
                            p.dst_refname().unwrap_or(&p.dst().to_string())
                        ))
                        .collect::<Vec<_>>()
                        .join(", "),
                    "push negotiation"
                );
                Ok(())
            });
            remote_callbacks.push_transfer_progress(|one, two, three| {
                tracing::debug!(
                    project_id = self.project.id,
                    "push transfer progress: {}/{}/{}",
                    one,
                    two,
                    three
                );
            });

            let mut fetch_opts = git2::FetchOptions::new();
            fetch_opts.remote_callbacks(remote_callbacks);
            fetch_opts.prune(git2::FetchPrune::On);

            let refspec = &format!("+refs/heads/*:refs/remotes/{}/*", remote_name);

            match remote.fetch(&[refspec], Some(&mut fetch_opts)) {
                Ok(()) => {
                    tracing::info!(project_id = self.project.id, %refspec, "git fetched");
                    return Ok(());
                }
                Err(error) => {
                    tracing::error!(project_id = self.project.id, ?error, "fetch failed");
                    continue;
                }
            }
        }

        Err(RemoteError::AuthError)
    }

    pub fn git_commit(&self, message: &str) -> Result<()> {
        let config = self
            .git_repository
            .config()
            .with_context(|| "failed to get config")?;
        let name = config
            .get_string("user.name")
            .context("failed to get user.name")?
            .context("name is not set")?;
        let email = config
            .get_string("user.email")
            .context("failed to get user.email")?
            .context("email is not set")?;

        // Get the default signature for the repository
        let signature =
            git::Signature::now(&name, &email).with_context(|| "failed to get signature")?;

        // Create the commit with current index
        let tree_id = self.git_repository.index()?.write_tree()?;
        let tree = self.git_repository.find_tree(tree_id)?;
        let parent_commit = self.git_repository.head()?.peel_to_commit()?;
        let commit_oid = self.git_repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;

        tracing::info!(
            project_id = self.project.id,
            %commit_oid,
            message,
            "created commit"
        );

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("git url is empty")]
    NoUrl,
    #[error("git url is not ssh: {0}")]
    NonSSHUrl(String),
    #[error("authentication failed")]
    AuthError,
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<RemoteError> for crate::error::Error {
    fn from(value: RemoteError) -> Self {
        match value {
            RemoteError::AuthError => crate::error::Error::UserError {
                code: crate::error::Code::ProjectGitAuth,
                message: "Project remote authentication error".to_string(),
            },
            RemoteError::NonSSHUrl(url) => crate::error::Error::UserError {
                code: crate::error::Code::ProjectGitRemote,
                message: format!("Project has non-ssh remote url: {}", url),
            },
            RemoteError::NoUrl => crate::error::Error::UserError {
                code: crate::error::Code::ProjectGitRemote,
                message: "Project has no remote url".to_string(),
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
    End,
}
