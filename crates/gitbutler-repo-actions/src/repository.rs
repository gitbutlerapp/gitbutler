use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_error::error::Code;
use gitbutler_oxidize::{ObjectIdExt, RepoExt};
use gitbutler_project::AuthKey;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_stack::{Stack, StackId};

use crate::askpass;
use gitbutler_repo::{
    credentials,
    logging::{LogUntil, RepositoryExt as _},
    RepoCommands, RepositoryExt,
};
pub trait RepoActionsExt {
    fn fetch(&self, remote_name: &str, askpass: Option<String>) -> Result<()>;
    fn push(
        &self,
        head: git2::Oid,
        branch: &RemoteRefname,
        with_force: bool,
        force_push_protection: bool,
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
            .repo()
            .maybe_find_branch_by_refname(&target_branch_refname)?
            .ok_or(anyhow!("failed to find branch {}", target_branch_refname))?;

        let commit_id: git2::Oid = branch.get().peel_to_commit()?.id();

        let now = gitbutler_time::time::now_ms();
        let branch_name = format!("test-push-{now}");

        let refname =
            RemoteRefname::from_str(&format!("refs/remotes/{remote_name}/{branch_name}",))?;

        match self.push(commit_id, &refname, false, false, None, askpass) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        let empty_refspec = Some(format!(":refs/heads/{}", branch_name));
        match self.push(commit_id, &refname, false, false, empty_refspec, askpass) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }?;

        Ok(())
    }

    fn add_branch_reference(&self, stack: &Stack) -> Result<()> {
        let gix_repo = self.repo().to_gix()?;
        let (should_write, with_force) =
            match self.repo().find_reference(&stack.refname()?.to_string()) {
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
            self.repo()
                .reference(
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
        match self.repo().find_reference(&stack.refname()?.to_string()) {
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
        let oids = self.repo().l(from, LogUntil::Commit(to), false)?;
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
            .repo()
            .signatures()
            .context("failed to get signatures")?;
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
        head: git2::Oid,
        branch: &RemoteRefname,
        with_force: bool,
        force_push_protection: bool,
        refspec: Option<String>,
        askpass_broker: Option<Option<StackId>>,
    ) -> Result<()> {
        let refspec = refspec.unwrap_or_else(|| {
            format!("{}:refs/heads/{}", head, branch.branch()) // for force pushing we previously had "+{}:refs/heads/{}" which was removed because it bypasses the force push protection flags as it is equivalent to --force
        });

        // NOTE(qix-): This is a nasty hack, however the codebase isn't structured
        // NOTE(qix-): in a way that allows us to really incorporate new backends
        // NOTE(qix-): without a lot of work. This is a temporary measure to
        // NOTE(qix-): work around a time-sensitive change that was necessary
        // NOTE(qix-): without having to refactor a large portion of the codebase.
        if self.project().preferred_key == AuthKey::SystemExecutable {
            let path = self.project().worktree_path();
            let remote = branch.remote().to_string();
            let push_result = std::thread::spawn(move || {
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
                    ))
            })
            .join()
            .unwrap()
            .map_err(|err| {
                match err {
                    gitbutler_git::Error::ForcePushProtection(_) => {
                        anyhow!("The force push was blocked because the remote branch contains commits that would be overwritten")
                            .context(Code::GitForcePushProtection)
                    },
                    _ => err.into()
                }
            });

            // Set up branch tracking after successful push
            if push_result.is_ok() {
                if let Err(err) = setup_branch_tracking(self, branch) {
                    tracing::warn!(
                        project_id = %self.project().id,
                        branch = branch.branch(),
                        remote = branch.remote(),
                        %err,
                        "Failed to set up tracking for {}. Push succeeded but force push detection may not work properly. Manual setup: git branch --set-upstream-to={}/{}",
                        branch.branch(), branch.remote(), branch.branch()
                    );
                }

                // Fetch the pushed branch to create proper remote-tracking reference
                if let Err(err) = fetch_pushed_branch(self, branch) {
                    tracing::warn!(
                        project_id = %self.project().id,
                        branch = branch.branch(),
                        remote = branch.remote(),
                        %err,
                        "Failed to fetch {} after push. Push succeeded but remote-tracking ref may be outdated. Manual refresh: git fetch {}",
                        branch.branch(), branch.remote()
                    );
                }
            }

            return push_result;
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

                        // Set up branch tracking after successful push
                        if let Err(err) = setup_branch_tracking(self, branch) {
                            tracing::warn!(
                                project_id = %self.project().id,
                                branch = branch.branch(),
                                remote = branch.remote(),
                                %err,
                                "Failed to set up tracking for {}. Push succeeded but force push detection may not work properly. Manual setup: git branch --set-upstream-to={}/{}",
                                branch.branch(), branch.remote(), branch.branch()
                            );
                        }

                        // Fetch the pushed branch to create proper remote-tracking reference
                        if let Err(err) = fetch_pushed_branch(self, branch) {
                            tracing::warn!(
                                project_id = %self.project().id,
                                branch = branch.branch(),
                                remote = branch.remote(),
                                %err,
                                "Failed to fetch {} after push. Push succeeded but remote-tracking ref may be outdated. Manual refresh: git fetch {}",
                                branch.branch(), branch.remote()
                            );
                        }

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

/// Sets up branch tracking configuration after a successful push.
/// This allows Git to properly track the relationship between local and remote branches,
/// enabling force push detection and other Git operations to work correctly.
///
/// Equivalent to: git branch --set-upstream-to=origin/branch-name branch-name
fn setup_branch_tracking(ctx: &CommandContext, branch: &RemoteRefname) -> Result<()> {
    let local_branch_name = branch.branch();
    let remote_name = branch.remote();

    tracing::info!(
        project_id = %ctx.project().id,
        branch = local_branch_name,
        remote = remote_name,
        "setup_branch_tracking called"
    );

    // Check if local branch exists before setting up tracking
    let local_branch_exists = ctx
        .repo()
        .find_branch(local_branch_name, git2::BranchType::Local)
        .is_ok();

    if !local_branch_exists {
        tracing::info!(
            project_id = %ctx.project().id,
            branch = local_branch_name,
            "skipping tracking setup: local branch does not exist"
        );
        return Ok(());
    }

    let project = ctx.project();

    // Set branch.{branch_name}.remote = {remote_name}
    let remote_config_key = format!("branch.{local_branch_name}.remote");
    project.set_local_config(&remote_config_key, remote_name)?;

    // Set branch.{branch_name}.merge = refs/heads/{branch_name}
    let merge_config_key = format!("branch.{local_branch_name}.merge");
    let merge_ref = format!("refs/heads/{}", local_branch_name);
    project.set_local_config(&merge_config_key, &merge_ref)?;

    tracing::info!(
        project_id = %ctx.project().id,
        branch = local_branch_name,
        remote = remote_name,
        "successfully set up branch tracking"
    );

    Ok(())
}

fn fetch_pushed_branch(ctx: &CommandContext, branch: &RemoteRefname) -> Result<()> {
    let remote_name = branch.remote();

    tracing::info!(
        project_id = %ctx.project().id,
        branch = branch.branch(),
        remote = remote_name,
        "fetching from remote to ensure pushed branch remote-tracking reference is up to date"
    );

    match ctx.fetch(remote_name, None) {
        Ok(()) => {
            tracing::info!(
                project_id = %ctx.project().id,
                branch = branch.branch(),
                remote = remote_name,
                "successfully fetched from remote after push"
            );
        }
        Err(err) => {
            tracing::warn!(
                project_id = %ctx.project().id,
                branch = branch.branch(),
                remote = remote_name,
                error = %err,
                "Failed to fetch {} from {} after push. Push succeeded but remote-tracking ref may be stale. Manual refresh: git fetch {}",
                branch.branch(), remote_name, remote_name
            );
            // Don't fail the entire operation - the push succeeded and tracking is set up
        }
    }

    Ok(())
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
