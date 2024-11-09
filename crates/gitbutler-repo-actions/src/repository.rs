use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_error::error::Code;
use gitbutler_project::AuthKey;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_stack::{Stack, StackId};

use crate::askpass;
use gitbutler_repo::{credentials, LogUntil, RepositoryExt};
pub trait RepoActionsExt {
    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()>;
    fn push(
        &self,
        head: git2::Oid,
        branch: &RemoteRefname,
        with_force: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
    ) -> Result<()>;
    fn commit(
        &self,
        message: &str,
        tree: &git2::Tree,
        parents: &[&git2::Commit],
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid>;
    fn distance(&self, from: git2::Oid, to: git2::Oid) -> Result<u32>;
    fn delete_branch_reference(&self, stack: &Stack) -> Result<()>;
    fn add_branch_reference(&self, stack: &Stack) -> Result<()>;
    fn git_test_push(
        &self,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<StackId>>,
    ) -> Result<()>;
}

impl RepoActionsExt for CommandContext {
    fn git_test_push(
        &self,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<StackId>>,
    ) -> Result<()> {
        let target_branch_refname =
            Refname::from_str(&format!("refs/remotes/{}/{}", remote_name, branch_name))?;
        let branch = self
            .repository()
            .maybe_find_branch_by_refname(&target_branch_refname)?
            .ok_or(anyhow!("failed to find branch {}", target_branch_refname))?;

        let commit_id: git2::Oid = branch.get().peel_to_commit()?.id();

        let now = gitbutler_time::time::now_ms();
        let branch_name = format!("test-push-{now}");

        let refname =
            RemoteRefname::from_str(&format!("refs/remotes/{remote_name}/{branch_name}",))?;

        match self.push(commit_id, &refname, false, None, askpass) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        let empty_refspec = Some(format!(":refs/heads/{}", branch_name));
        match self.push(commit_id, &refname, false, empty_refspec, askpass) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        Ok(())
    }

    fn add_branch_reference(&self, stack: &Stack) -> Result<()> {
        let (should_write, with_force) = match self
            .repository()
            .find_reference(&stack.refname()?.to_string())
        {
            Ok(reference) => match reference.target() {
                Some(head_oid) => Ok((head_oid != stack.head(), true)),
                None => Ok((true, true)),
            },
            Err(err) => match err.code() {
                git2::ErrorCode::NotFound => Ok((true, false)),
                _ => Err(err),
            },
        }
        .context("failed to lookup reference")?;

        if should_write {
            self.repository()
                .reference(
                    &stack.refname()?.to_string(),
                    stack.head(),
                    with_force,
                    "new vbranch",
                )
                .context("failed to create branch reference")?;
        }

        Ok(())
    }

    fn delete_branch_reference(&self, stack: &Stack) -> Result<()> {
        match self
            .repository()
            .find_reference(&stack.refname()?.to_string())
        {
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

    // returns the number of commits between the first oid to the second oid
    fn distance(&self, from: git2::Oid, to: git2::Oid) -> Result<u32> {
        let oids = self.repository().l(from, LogUntil::Commit(to), false)?;
        Ok(oids.len().try_into()?)
    }

    fn commit(
        &self,
        message: &str,
        tree: &git2::Tree,
        parents: &[&git2::Commit],
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid> {
        let (author, committer) = self
            .repository()
            .signatures()
            .context("failed to get signatures")?;
        self.repository()
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
        head: git2::Oid,
        branch: &RemoteRefname,
        with_force: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
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

        let auth_flows = credentials::help(self, branch.remote())?;
        for (mut remote, callbacks) in auth_flows {
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

    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()> {
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

        let auth_flows = credentials::help(self, remote_name)?;
        for (mut remote, callbacks) in auth_flows {
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

async fn handle_git_prompt_push(
    prompt: String,
    askpass: Option<Option<StackId>>,
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
