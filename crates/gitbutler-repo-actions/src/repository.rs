use std::str::FromStr;

use anyhow::{Context as _, Result, anyhow, bail};
use but_ctx::Context;
use but_error::Code;
use but_oxidize::ObjectIdExt;
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_project::AuthKey;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{
    RepositoryExt, credentials,
    logging::{LogUntil, RepositoryExt as _},
};
use gitbutler_stack::{Stack, StackId};

use crate::askpass;
#[allow(clippy::too_many_arguments)]
pub trait RepoActionsExt {
    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()>;
    /// Returns the stderr output of the git executable if used.
    fn push(
        &self,
        head: git2::Oid,
        branch: &RemoteRefname,
        with_force: bool,
        force_push_protection: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
        push_opts: Vec<String>,
    ) -> Result<String>;
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

impl RepoActionsExt for Context {
    fn git_test_push(
        &self,
        remote_name: &str,
        branch_name: &str,
        askpass: Option<Option<StackId>>,
    ) -> Result<()> {
        let target_branch_refname =
            Refname::from_str(&format!("refs/remotes/{remote_name}/{branch_name}"))?;
        let git2_repo = self.git2_repo.get()?;
        let branch = git2_repo
            .maybe_find_branch_by_refname(&target_branch_refname)?
            .ok_or(anyhow!("failed to find branch {}", target_branch_refname))?;

        let commit_id: git2::Oid = branch.get().peel_to_commit()?.id();

        let now = gitbutler_time::time::now_ms();
        let branch_name = format!("test-push-{now}");

        let refname =
            RemoteRefname::from_str(&format!("refs/remotes/{remote_name}/{branch_name}",))?;

        match self.push(commit_id, &refname, false, false, None, askpass, vec![]) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        let empty_refspec = Some(format!(":refs/heads/{branch_name}"));
        match self.push(
            commit_id,
            &refname,
            false,
            false,
            empty_refspec,
            askpass,
            vec![],
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        Ok(())
    }

    fn add_branch_reference(&self, stack: &Stack) -> Result<()> {
        let gix_repo = self.repo.get()?;
        let repo = self.git2_repo.get()?;
        let (should_write, with_force) = match repo.find_reference(&stack.refname()?.to_string()) {
            Ok(reference) => match reference.target() {
                Some(head_oid) => Ok((head_oid != stack.head_oid(&gix_repo)?.to_git2(), true)),
                None => Ok((true, true)),
            },
            Err(err) => match err.code() {
                git2::ErrorCode::NotFound => Ok((true, false)),
                _ => Err(err),
            },
        }
        .context("failed to lookup reference")?;

        if should_write {
            repo.reference(
                &stack.refname()?.to_string(),
                stack.head_oid(&gix_repo)?.to_git2(),
                with_force,
                "new vbranch",
            )
            .context("failed to create branch reference")?;
        }

        Ok(())
    }

    fn delete_branch_reference(&self, stack: &Stack) -> Result<()> {
        match self
            .git2_repo
            .get()?
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
        let oids = self.git2_repo.get()?.l(from, LogUntil::Commit(to), false)?;
        Ok(oids.len().try_into()?)
    }

    fn commit(
        &self,
        message: &str,
        tree: &git2::Tree,
        parents: &[&git2::Commit],
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid> {
        let git2_repo = self.git2_repo.get()?;
        let (author, committer) = git2_repo.signatures().context("failed to get signatures")?;
        git2_repo
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
        force_push_protection: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
        push_opts: Vec<String>,
    ) -> Result<String> {
        let use_git_executable = self.legacy_project.preferred_key == AuthKey::SystemExecutable;
        if !use_git_executable && force_push_protection {
            bail!("Force push protection is only supported when 'Using the Git executable'");
        }
        let refspec = refspec.unwrap_or_else(|| {
            // The Git executable has flags set related to force, and these flags don't play well
            // with the refspec force-format which seems to override them, leading to incorrect results
            // in conjunction with `force_push_protection`.
            let prefix = if with_force && !use_git_executable {
                "+"
            } else {
                Default::default()
            };
            format!("{prefix}{}:refs/heads/{}", head, branch.branch())
        });

        // NOTE(qix-): This is a nasty hack, however the codebase isn't structured
        // NOTE(qix-): in a way that allows us to really incorporate new backends
        // NOTE(qix-): without a lot of work. This is a temporary measure to
        // NOTE(qix-): work around a time-sensitive change that was necessary
        // NOTE(qix-): without having to refactor a large portion of the codebase.
        if use_git_executable {
            let path = self.legacy_project.git_dir().to_owned();
            let remote = branch.remote().to_string();
            match std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(gitbutler_git::push(
                        path,
                        gitbutler_git::tokio::TokioExecutor,
                        &remote,
                        gitbutler_git::RefSpec::parse(refspec).unwrap(),
                        with_force,
                        force_push_protection,
                        handle_git_prompt_push,
                        askpass_broker,
                        push_opts,
                    ))
            })
            .join()
            .unwrap() {
                Ok(result) => Ok(result),
                Err(err) => match err {
                    gitbutler_git::Error::ForcePushProtection(_) => {
                        Err(anyhow!("The force push was blocked because the remote branch contains commits that would be overwritten")
                            .context(Code::GitForcePushProtection))
                    },
                    gitbutler_git::Error::GerritNoNewChanges(_) => {
                        // Treat "no new changes" as success for Gerrit
                        Ok("".to_string())
                    },
                    _ => Err(err.into())
                }
            }
        } else {
            let git2_repo = self.git2_repo.get()?;
            let auth_flows = credentials::help(&git2_repo, &self.legacy_project, branch.remote())?;
            for (mut remote, callbacks) in auth_flows {
                let mut update_refs_error: Option<git2::Error> = None;
                for callback in callbacks {
                    let mut cbs: git2::RemoteCallbacks = callback.into();
                    if self.legacy_project.omit_certificate_check.unwrap_or(false) {
                        cbs.certificate_check(|_, _| {
                            Ok(git2::CertificateCheckStatus::CertificateOk)
                        });
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
                                project_id = %self.legacy_project.id,
                                remote = %branch.remote(),
                                %head,
                                branch = branch.branch(),
                                "pushed git branch"
                            );
                            return Ok("".to_string());
                        }
                        Err(err) => match err.class() {
                            git2::ErrorClass::Net | git2::ErrorClass::Http => {
                                tracing::warn!(project_id = %self.legacy_project.id, ?err, "push failed due to network");
                                continue;
                            }
                            _ => match err.code() {
                                git2::ErrorCode::Auth => {
                                    tracing::warn!(project_id = %self.legacy_project.id, ?err, "push failed due to auth");
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
    }

    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()> {
        let refspec = format!("+refs/heads/*:refs/remotes/{remote_name}/*");

        // NOTE(qix-): This is a nasty hack, however the codebase isn't structured
        // NOTE(qix-): in a way that allows us to really incorporate new backends
        // NOTE(qix-): without a lot of work. This is a temporary measure to
        // NOTE(qix-): work around a time-sensitive change that was necessary
        // NOTE(qix-): without having to refactor a large portion of the codebase.
        if self.legacy_project.preferred_key == AuthKey::SystemExecutable {
            let path = self.legacy_project.git_dir().to_owned();
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

        let git2_repo = self.git2_repo.get()?;
        let auth_flows = credentials::help(&git2_repo, &self.legacy_project, remote_name)?;
        for (mut remote, callbacks) in auth_flows {
            for callback in callbacks {
                let mut fetch_opts = git2::FetchOptions::new();
                let mut cbs: git2::RemoteCallbacks = callback.into();
                if self.legacy_project.omit_certificate_check.unwrap_or(false) {
                    cbs.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
                }
                fetch_opts.remote_callbacks(cbs);
                fetch_opts.prune(git2::FetchPrune::On);

                match remote.fetch(&[&refspec], Some(&mut fetch_opts), None) {
                    Ok(()) => {
                        tracing::info!(project_id = %self.legacy_project.id, %refspec, "git fetched");
                        return Ok(());
                    }
                    Err(err) => match err.class() {
                        git2::ErrorClass::Net | git2::ErrorClass::Http => {
                            tracing::warn!(project_id = %self.legacy_project.id, ?err, "fetch failed due to network");
                            continue;
                        }
                        _ => match err.code() {
                            git2::ErrorCode::Auth => {
                                tracing::warn!(project_id = %self.legacy_project.id, ?err, "fetch failed due to auth");
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
