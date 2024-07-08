use std::str::FromStr;

use anyhow::{anyhow, Context, Result};

use gitbutler_branch::branch::{Branch, BranchId};
use gitbutler_command_context::ProjectRepo;
use gitbutler_core::{
    error::Code,
    git::{self, CommitHeadersV2},
    ssh,
};

use crate::{askpass, Config};
use gitbutler_project::AuthKey;

use crate::{credentials::Helper, RepositoryExt};
pub trait RepoActions {
    fn fetch(&self, remote_name: &str, credentials: &Helper, askpass: Option<String>)
        -> Result<()>;
    fn push(
        &self,
        head: &git2::Oid,
        branch: &git::RemoteRefname,
        with_force: bool,
        credentials: &Helper,
        refspec: Option<String>,
        askpass_broker: Option<Option<BranchId>>,
    ) -> Result<()>;
    fn commit(
        &self,
        message: &str,
        tree: &git2::Tree,
        parents: &[&git2::Commit],
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid>;
    fn distance(&self, from: git2::Oid, to: git2::Oid) -> Result<u32>;
    fn log(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Commit>>;
    fn list_commits(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Commit>>;
    fn list(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Oid>>;
    fn l(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Oid>>;
    fn delete_branch_reference(&self, branch: &Branch) -> Result<()>;
    fn add_branch_reference(&self, branch: &Branch) -> Result<()>;
    fn git_test_push(
        &self,
        credentials: &Helper,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<BranchId>>,
    ) -> Result<()>;
}

impl RepoActions for ProjectRepo {
    fn git_test_push(
        &self,
        credentials: &Helper,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<BranchId>>,
    ) -> Result<()> {
        let target_branch_refname =
            git::Refname::from_str(&format!("refs/remotes/{}/{}", remote_name, branch_name))?;
        let branch = self
            .repo()
            .find_branch_by_refname(&target_branch_refname)?
            .ok_or(anyhow!("failed to find branch {}", target_branch_refname))?;

        let commit_id: git2::Oid = branch.get().peel_to_commit()?.id();

        let now = gitbutler_core::time::now_ms();
        let branch_name = format!("test-push-{now}");

        let refname =
            git::RemoteRefname::from_str(&format!("refs/remotes/{remote_name}/{branch_name}",))?;

        match self.push(&commit_id, &refname, false, credentials, None, askpass) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        let empty_refspec = Some(format!(":refs/heads/{}", branch_name));
        match self.push(
            &commit_id,
            &refname,
            false,
            credentials,
            empty_refspec,
            askpass,
        ) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        Ok(())
    }

    fn add_branch_reference(&self, branch: &Branch) -> Result<()> {
        let (should_write, with_force) =
            match self.repo().find_reference(&branch.refname().to_string()) {
                Ok(reference) => match reference.target() {
                    Some(head_oid) => Ok((head_oid != branch.head, true)),
                    None => Ok((true, true)),
                },
                Err(err) => match err.code() {
                    git2::ErrorCode::NotFound => Ok((true, false)),
                    _ => Err(err),
                },
            }
            .context("failed to lookup reference")?;

        if should_write {
            self.repo()
                .reference(
                    &branch.refname().to_string(),
                    branch.head,
                    with_force,
                    "new vbranch",
                )
                .context("failed to create branch reference")?;
        }

        Ok(())
    }

    fn delete_branch_reference(&self, branch: &Branch) -> Result<()> {
        match self.repo().find_reference(&branch.refname().to_string()) {
            Ok(mut reference) => {
                reference
                    .delete()
                    .context("failed to delete branch reference")?;
                Ok(())
            }
            Err(err) => match err.code() {
                git2::ErrorCode::NotFound => Ok(()),
                _ => Err(err),
            },
        }
        .context("failed to lookup reference")
    }

    // returns a list of commit oids from the first oid to the second oid
    fn l(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Oid>> {
        match to {
            LogUntil::Commit(oid) => {
                let mut revwalk = self.repo().revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .hide(oid)
                    .context(format!("failed to hide {}", oid))?;
                revwalk
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
            LogUntil::Take(n) => {
                let mut revwalk = self.repo().revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .take(n)
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
            LogUntil::When(cond) => {
                let mut revwalk = self.repo().revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                let mut oids: Vec<git2::Oid> = vec![];
                for oid in revwalk {
                    let oid = oid.context("failed to get oid")?;
                    oids.push(oid);

                    let commit = self
                        .repo()
                        .find_commit(oid)
                        .context("failed to find commit")?;

                    if cond(&commit).context("failed to check condition")? {
                        break;
                    }
                }
                Ok(oids)
            }
            LogUntil::End => {
                let mut revwalk = self.repo().revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
        }
        .context("failed to collect oids")
    }

    // returns a list of oids from the first oid to the second oid
    fn list(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Oid>> {
        self.l(from, LogUntil::Commit(to))
    }

    fn list_commits(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Commit>> {
        Ok(self
            .list(from, to)?
            .into_iter()
            .map(|oid| self.repo().find_commit(oid))
            .collect::<Result<Vec<_>, _>>()?)
    }

    // returns a list of commits from the first oid to the second oid
    fn log(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Commit>> {
        self.l(from, to)?
            .into_iter()
            .map(|oid| self.repo().find_commit(oid))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to collect commits")
    }

    // returns the number of commits between the first oid to the second oid
    fn distance(&self, from: git2::Oid, to: git2::Oid) -> Result<u32> {
        let oids = self.l(from, LogUntil::Commit(to))?;
        Ok(oids.len().try_into()?)
    }

    fn commit(
        &self,
        message: &str,
        tree: &git2::Tree,
        parents: &[&git2::Commit],
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid> {
        let (author, committer) = signatures(self).context("failed to get signatures")?;
        self.repo()
            .commit_with_signature(
                None,
                &author,
                &committer,
                message,
                tree,
                parents,
                commit_headers,
            )
            .context("failed to commit")
    }

    fn push(
        &self,
        head: &git2::Oid,
        branch: &git::RemoteRefname,
        with_force: bool,
        credentials: &Helper,
        refspec: Option<String>,
        askpass_broker: Option<Option<BranchId>>,
    ) -> Result<()> {
        let refspec = refspec.unwrap_or_else(|| {
            if with_force {
                format!("+{}:refs/heads/{}", head, branch.branch())
            } else {
                format!("{}:refs/heads/{}", head, branch.branch())
            }
        });

        // NOTE(qix-): This is a nasty hack, however the codebase isn't structured
        // NOTE(qix-): in a way that allows us to really incorporate new backends
        // NOTE(qix-): without a lot of work. This is a temporary measure to
        // NOTE(qix-): work around a time-sensitive change that was necessary
        // NOTE(qix-): without having to refactor a large portion of the codebase.
        if self.project().preferred_key == AuthKey::SystemExecutable {
            let path = self.project().worktree_path();
            let remote = branch.remote().to_string();
            return std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(gitbutler_git::push(
                        path,
                        gitbutler_git::tokio::TokioExecutor,
                        &remote,
                        gitbutler_git::RefSpec::parse(refspec).unwrap(),
                        with_force,
                        handle_git_prompt_push,
                        askpass_broker,
                    ))
            })
            .join()
            .unwrap()
            .map_err(Into::into);
        }

        let auth_flows = credentials.help(self, branch.remote())?;
        for (mut remote, callbacks) in auth_flows {
            if let Some(url) = remote.url() {
                if !self.project().omit_certificate_check.unwrap_or(false) {
                    let git_url = git::Url::from_str(url)?;
                    ssh::check_known_host(&git_url).context("failed to check known host")?;
                }
            }
            let mut update_refs_error: Option<git2::Error> = None;
            for callback in callbacks {
                let mut cbs: git2::RemoteCallbacks = callback.into();
                if self.project().omit_certificate_check.unwrap_or(false) {
                    cbs.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
                }
                cbs.push_update_reference(|_reference: &str, status: Option<&str>| {
                    if let Some(status) = status {
                        update_refs_error = Some(git2::Error::from_str(status));
                        return Err(git2::Error::from_str(status));
                    };
                    Ok(())
                });

                let push_result = remote.push(
                    &[refspec.as_str()],
                    Some(&mut git2::PushOptions::new().remote_callbacks(cbs)),
                );
                match push_result {
                    Ok(()) => {
                        tracing::info!(
                            project_id = %self.project().id,
                            remote = %branch.remote(),
                            %head,
                            branch = branch.branch(),
                            "pushed git branch"
                        );
                        return Ok(());
                    }
                    Err(err) => match err.class() {
                        git2::ErrorClass::Net | git2::ErrorClass::Http => {
                            tracing::warn!(project_id = %self.project().id, ?err, "push failed due to network");
                            continue;
                        }
                        _ => match err.code() {
                            git2::ErrorCode::Auth => {
                                tracing::warn!(project_id = %self.project().id, ?err, "push failed due to auth");
                                continue;
                            }
                            _ => {
                                if let Some(update_refs_err) = update_refs_error {
                                    return Err(update_refs_err).context(err);
                                }
                                return Err(err.into());
                            }
                        },
                    },
                }
            }
        }

        Err(anyhow!("authentication failed").context(Code::ProjectGitAuth))
    }

    fn fetch(
        &self,
        remote_name: &str,
        credentials: &Helper,
        askpass: Option<String>,
    ) -> Result<()> {
        let refspec = format!("+refs/heads/*:refs/remotes/{}/*", remote_name);

        // NOTE(qix-): This is a nasty hack, however the codebase isn't structured
        // NOTE(qix-): in a way that allows us to really incorporate new backends
        // NOTE(qix-): without a lot of work. This is a temporary measure to
        // NOTE(qix-): work around a time-sensitive change that was necessary
        // NOTE(qix-): without having to refactor a large portion of the codebase.
        if self.project().preferred_key == AuthKey::SystemExecutable {
            let path = self.project().worktree_path();
            let remote = remote_name.to_string();
            return std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(gitbutler_git::fetch(
                        path,
                        gitbutler_git::tokio::TokioExecutor,
                        &remote,
                        gitbutler_git::RefSpec::parse(refspec).unwrap(),
                        handle_git_prompt_fetch,
                        askpass,
                    ))
            })
            .join()
            .unwrap()
            .map_err(Into::into);
        }

        let auth_flows = credentials.help(self, remote_name)?;
        for (mut remote, callbacks) in auth_flows {
            if let Some(url) = remote.url() {
                if !self.project().omit_certificate_check.unwrap_or(false) {
                    let git_url = git::Url::from_str(url)?;
                    ssh::check_known_host(&git_url).context("failed to check known host")?;
                }
            }
            for callback in callbacks {
                let mut fetch_opts = git2::FetchOptions::new();
                let mut cbs: git2::RemoteCallbacks = callback.into();
                if self.project().omit_certificate_check.unwrap_or(false) {
                    cbs.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
                }
                fetch_opts.remote_callbacks(cbs);
                fetch_opts.prune(git2::FetchPrune::On);

                match remote.fetch(&[&refspec], Some(&mut fetch_opts), None) {
                    Ok(()) => {
                        tracing::info!(project_id = %self.project().id, %refspec, "git fetched");
                        return Ok(());
                    }
                    Err(err) => match err.class() {
                        git2::ErrorClass::Net | git2::ErrorClass::Http => {
                            tracing::warn!(project_id = %self.project().id, ?err, "fetch failed due to network");
                            continue;
                        }
                        _ => match err.code() {
                            git2::ErrorCode::Auth => {
                                tracing::warn!(project_id = %self.project().id, ?err, "fetch failed due to auth");
                                continue;
                            }
                            _ => {
                                return Err(err.into());
                            }
                        },
                    },
                }
            }
        }

        Err(anyhow!("authentication failed")).context(Code::ProjectGitAuth)
    }
}

fn signatures(project_repo: &ProjectRepo) -> Result<(git2::Signature, git2::Signature)> {
    let config: Config = project_repo.repo().into();

    let author = match (config.user_name()?, config.user_email()?) {
        (None, Some(email)) => git2::Signature::now(&email, &email)?,
        (Some(name), None) => git2::Signature::now(&name, &format!("{}@example.com", &name))?,
        (Some(name), Some(email)) => git2::Signature::now(&name, &email)?,
        _ => git2::Signature::now("GitButler", "gitbutler@gitbutler.com")?,
    };

    let comitter = if config.user_real_comitter()? {
        author.clone()
    } else {
        git2::Signature::now("GitButler", "gitbutler@gitbutler.com")?
    };

    Ok((author, comitter))
}

type OidFilter = dyn Fn(&git2::Commit) -> Result<bool>;

pub enum LogUntil {
    Commit(git2::Oid),
    Take(usize),
    When(Box<OidFilter>),
    End,
}

async fn handle_git_prompt_push(
    prompt: String,
    askpass: Option<Option<BranchId>>,
) -> Option<String> {
    if let Some(branch_id) = askpass {
        tracing::info!("received prompt for branch push {branch_id:?}: {prompt:?}");
        askpass::get_broker()
            .submit_prompt(prompt, askpass::Context::Push { branch_id })
            .await
    } else {
        tracing::warn!("received askpass push prompt but no broker was supplied; returning None");
        None
    }
}

async fn handle_git_prompt_fetch(prompt: String, askpass: Option<String>) -> Option<String> {
    if let Some(action) = askpass {
        tracing::info!("received prompt for fetch with action {action:?}: {prompt:?}");
        askpass::get_broker()
            .submit_prompt(prompt, askpass::Context::Fetch { action })
            .await
    } else {
        tracing::warn!("received askpass fetch prompt but no broker was supplied; returning None");
        None
    }
}
