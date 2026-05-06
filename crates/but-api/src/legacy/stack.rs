use std::borrow::Cow;

use anyhow::{Context as _, Result, anyhow};
use but_api_macros::but_api;
use but_core::{RepositoryExt, branch, ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_workspace::legacy::push::{PushBranch, PushStackPlan, PushTarget};
use gitbutler_branch_actions::stack::CreateSeriesRequest;
use gitbutler_git::{GitContextExt as _, PushResult};
use gitbutler_oplog::SnapshotExt;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::{first_parent_commit_ids_until, hooks};
use gix::refs::Category;
use tracing::instrument;

pub mod create_reference {
    use serde::{Deserialize, Serialize};

    use crate::json::HexHash;

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        /// The short name of the new branch, i.e. `foo` or `features/bar`
        pub new_name: String,
        /// If `None`, it's a new stack.
        pub anchor: Option<Anchor>,
    }

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(tag = "type", content = "subject", rename_all = "camelCase")]
    pub enum Anchor {
        AtCommit {
            commit_id: HexHash,
            position: but_workspace::branch::create_reference::Position,
        },
        AtReference {
            short_name: String,
            position: but_workspace::branch::create_reference::Position,
        },
    }
}

/// Create the local branch reference described by `request`.
///
/// This acquires exclusive worktree access from `ctx` before normalizing and
/// creating the reference.
///
/// See [`create_reference_with_perm()`] for how `request` is normalized and
/// applied through [`but_workspace::branch::create_reference()`].
#[but_api]
#[instrument(err(Debug))]
pub fn create_reference(
    ctx: &mut Context,
    request: create_reference::Request,
) -> Result<(Option<StackId>, gix::refs::FullName)> {
    let mut guard = ctx.exclusive_worktree_access();
    create_reference_with_perm(ctx, request, guard.write_permission())
}

/// Create a local branch reference using an existing `perm` for exclusive access.
///
/// It normalizes the requested branch name in `request` into a full local refname,
/// translates the legacy anchor payload into the `but_workspace` representation,
/// and updates the workspace state in place without acquiring its own repository lock.
///
/// Returns the stack id owning the created or attached reference when one exists, together with
/// the full refname that was created.
///
/// The underlying implementation is [`but_workspace::branch::create_reference()`],
#[but_api]
#[instrument(skip(ctx, perm), err(Debug))]
pub fn create_reference_with_perm(
    ctx: &mut Context,
    request: create_reference::Request,
    perm: &mut RepoExclusive,
) -> Result<(Option<StackId>, gix::refs::FullName)> {
    use bstr::ByteSlice;
    let create_reference::Request { new_name, anchor } = request;
    let new_ref = Category::LocalBranch
        .to_full_name(branch::normalize_short_name(new_name.as_str())?.as_bstr())
        .map_err(anyhow::Error::from)?;
    let anchor = anchor
        .map(|anchor| -> Result<_> {
            Ok(match anchor {
                create_reference::Anchor::AtCommit {
                    commit_id,
                    position,
                } => but_workspace::branch::create_reference::Anchor::AtCommit {
                    commit_id: commit_id.into(),
                    position,
                },
                create_reference::Anchor::AtReference {
                    short_name,
                    position,
                } => but_workspace::branch::create_reference::Anchor::AtSegment {
                    ref_name: Cow::Owned(
                        Category::LocalBranch
                            .to_full_name(short_name.as_str())
                            .map_err(anyhow::Error::from)?,
                    ),
                    position,
                },
            })
        })
        .transpose()?;

    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let new_ws = but_workspace::branch::create_reference(
        new_ref.clone(),
        anchor,
        &repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None,
    )?;

    let stack_id = new_ws
        .find_segment_and_stack_by_refname(new_ref.as_ref())
        .and_then(|(stack, _)| stack.id);

    *ws = new_ws.into_owned();
    Ok((stack_id, new_ref))
}

/// Create a dependent branch named by `request.name` in the stack identified by
/// `stack_id`.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// dependent-branch snapshot and mutating the workspace.
#[but_api]
#[instrument(err(Debug))]
pub fn create_branch(
    ctx: &mut Context,
    stack_id: StackId,
    request: CreateSeriesRequest,
) -> Result<()> {
    let normalized_name = branch::normalize_short_name(request.name.as_str())?.to_string();
    let new_ref = Category::LocalBranch
        .to_full_name(normalized_name.as_str())
        .map_err(anyhow::Error::from)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    ctx.snapshot_create_dependent_branch(&normalized_name, guard.write_permission())
        .ok();

    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let stack = ws.try_find_stack_by_id(stack_id)?;
    if request.preceding_head.is_some() {
        return Err(anyhow!(
            "BUG: cannot have preceding head name set - let's use the new API instead"
        ));
    }

    let new_ws = but_workspace::branch::create_reference(
        new_ref.as_ref(),
        {
            use but_workspace::branch::create_reference::Position::Above;
            let segment = stack.segments.first().context("BUG: no empty stacks")?;
            segment
                .ref_info
                .as_ref()
                .map(
                    |ri| but_workspace::branch::create_reference::Anchor::AtSegment {
                        ref_name: Cow::Borrowed(ri.ref_name.as_ref()),
                        position: Above,
                    },
                )
                .or_else(|| {
                    Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                        commit_id: ws.graph.tip_skip_empty(segment.id)?.id,
                        position: Above,
                    })
                })
                .with_context(|| {
                    format!(
                        "TODO: UI should migrate to new version of `create_branch()` instead,\
                            couldn't handle stack_id={stack_id:?}, request={request:?}"
                    )
                })?
        },
        &repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None, // order - not used for dependent branches
    )?;

    *ws = new_ws.into_owned();
    Ok(())
}

/// Remove a branch without creating an oplog snapshot.
///
/// This is the core implementation used by both [`remove_branch`] (which creates its own snapshot)
/// and batch operations like `but clean` (which create a single snapshot for multiple removals).
pub fn remove_branch_only(
    ctx: &mut Context,
    branch_name: &str,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let ref_name = Category::LocalBranch
        .to_full_name(branch_name)
        .map_err(anyhow::Error::from)?;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let new_ws = but_workspace::branch::remove_reference(
        ref_name.as_ref(),
        &repo,
        &ws,
        &mut meta,
        but_workspace::branch::remove_reference::Options {
            avoid_anonymous_stacks: true,
            keep_metadata: false,
        },
    )?;

    if let Some(new_ws) = new_ws {
        *ws = new_ws;
    }
    Ok(())
}

/// Remove a branch from a stack.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// removal snapshot and detaching the branch.
///
/// This can only be called on a branch that's inside of a stack of multiple branches and is not the top branch,
/// or on a branch that's empty.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn remove_branch(ctx: &mut Context, stack_id: StackId, branch_name: String) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    remove_branch_with_perm(ctx, stack_id, branch_name, guard.write_permission())
}

/// Remove a branch from a stack while reusing caller-held exclusive access.
///
/// This records the dependent-branch removal snapshot and then delegates to
/// [`remove_branch_only()`] for the actual workspace mutation.
pub fn remove_branch_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    perm: &mut RepoExclusive,
) -> Result<()> {
    let _ = stack_id;
    ctx.snapshot_remove_dependent_branch(&branch_name, perm)
        .ok();
    remove_branch_only(ctx, &branch_name, perm)
}

/// Change the branch name from `branch_name` to `new_name` in the stack
/// identified by `stack_id`.
///
/// This acquires exclusive worktree access from `ctx` before applying the
/// rename.
///
/// See [`update_branch_name_with_perm()`] for the underlying mutation.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn update_branch_name(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    update_branch_name_with_perm(
        ctx,
        stack_id,
        branch_name,
        new_name,
        guard.write_permission(),
    )?;
    Ok(())
}

/// Apply the rename from `branch_name` to `new_name` in the stack identified by
/// `stack_id` while reusing caller-held exclusive access.
///
/// This delegates to
/// [`gitbutler_branch_actions::stack::update_branch_name_with_perm()`].
pub fn update_branch_name_with_perm(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
    perm: &mut RepoExclusive,
) -> Result<()> {
    gitbutler_branch_actions::stack::update_branch_name_with_perm(
        ctx,
        stack_id,
        branch_name,
        new_name,
        perm,
    )?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_branch_pr_number(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    pr_number: Option<usize>,
) -> Result<()> {
    gitbutler_branch_actions::stack::update_branch_pr_number(
        ctx,
        stack_id,
        branch_name,
        pr_number,
    )?;
    Ok(())
}

pub mod json {
    use serde::Serialize;

    /// JSON-friendly version of [`gitbutler_git::PushResult`].
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct PushResult {
        /// The name of the remote to which the branches were pushed.
        pub remote: String,
        /// The list of pushed branches and their corresponding remote refnames.
        pub branch_to_remote: Vec<(String, String)>,
        /// The list of branches with their before/after commit SHAs.
        /// Format: (branch_name, before_sha, after_sha)
        pub branch_sha_updates: Vec<(String, String, String)>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(PushResult);

    impl From<gitbutler_git::PushResult> for PushResult {
        fn from(value: gitbutler_git::PushResult) -> Self {
            Self {
                remote: value.remote,
                branch_to_remote: value
                    .branch_to_remote
                    .into_iter()
                    .map(|(name, refname)| (name, refname.to_string()))
                    .collect(),
                branch_sha_updates: value.branch_sha_updates,
            }
        }
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn push_stack(
    ctx: &mut Context,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: String,
    run_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<PushResult> {
    let guard = ctx.exclusive_worktree_access();
    let target = PushTarget::from_context(ctx)?;
    let options = PushStackOptions {
        with_force,
        force_push_protection: !skip_force_push_protection
            && ctx.legacy_project.force_push_protection,
        run_hooks,
        push_flags: push_opts,
    };

    // First fetch, because we don't want to push integrated series.
    for remote_name in target.remote_names_to_fetch() {
        ctx.fetch(remote_name, Some("push_stack".into()))?;
    }
    let plan =
        PushStackPlan::from_workspace(ctx, stack_id, &target, &branch, guard.read_permission())?;

    PushStackExecutor::new(ctx, &plan, &target, options)?.execute(ctx)
}

#[but_api(napi, json::PushResult)]
#[instrument(err(Debug))]
pub fn push_stack_legacy(
    ctx: &mut Context,
    stack_id: StackId,
    with_force: bool,
    skip_force_push_protection: bool,
    branch: String,
    run_hooks: bool,
) -> Result<PushResult> {
    push_stack(
        ctx,
        stack_id,
        with_force,
        skip_force_push_protection,
        branch,
        run_hooks,
        Vec::new(),
    )
}

struct PushStackOptions {
    with_force: bool,
    force_push_protection: bool,
    run_hooks: bool,
    push_flags: Vec<but_gerrit::PushFlag>,
}

struct PushStackExecutor<'plan> {
    plan: &'plan PushStackPlan,
    remote_name: String,
    target_branch_name: String,
    gix_repo: gix::Repository,
    gerrit_mode: bool,
    run_husky_hooks: bool,
    options: PushStackOptions,
}

impl<'plan> PushStackExecutor<'plan> {
    fn new(
        ctx: &Context,
        plan: &'plan PushStackPlan,
        target: &PushTarget,
        options: PushStackOptions,
    ) -> Result<Self> {
        let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;
        let gerrit_mode = gix_repo
            .git_settings()?
            .gitbutler_gerrit_mode
            .unwrap_or(false);
        let run_husky_hooks = ctx.legacy_project.husky_hooks_enabled;

        Ok(Self {
            plan,
            remote_name: target.push_remote_name().to_owned(),
            target_branch_name: target.target_branch_name().to_owned(),
            gix_repo,
            gerrit_mode,
            run_husky_hooks,
            options,
        })
    }

    fn execute(self, ctx: &mut Context) -> Result<PushResult> {
        let mut result = PushResult {
            remote: self.remote_name.clone(),
            branch_to_remote: vec![],
            branch_sha_updates: vec![],
        };

        for branch in self.plan.branches() {
            self.push_branch(ctx, branch)?;
            append_push_result(&mut result, branch);
        }

        Ok(result)
    }

    fn push_branch(&self, ctx: &mut Context, branch: &PushBranch) -> Result<()> {
        if self.options.run_hooks {
            self.run_pre_push_hook(branch)?;
        }

        let gerrit_push_args = self.gerrit_push_args(branch.local_sha());
        let push_output = ctx.push(
            branch.local_sha(),
            branch.remote_refname().to_owned(),
            self.options.with_force,
            self.options.force_push_protection,
            gerrit_push_args.refspec,
            Some(Some(self.plan.id())),
            gerrit_push_args.push_opts,
        )?;

        self.record_gerrit_push_metadata(ctx, &push_output)?;

        Ok(())
    }

    fn run_pre_push_hook(&self, branch: &PushBranch) -> Result<()> {
        let remote = self.gix_repo.find_remote(self.remote_name.as_str())?;
        let url = remote
            .url(gix::remote::Direction::Push)
            .or_else(|| remote.url(gix::remote::Direction::Fetch))
            .map(|url| url.to_bstring().to_string())
            .with_context(|| format!("Remote named {} didn't have a URL", self.remote_name))?;
        let remote_refname = RemoteRefname::new(&self.remote_name, branch.name());

        match hooks::pre_push(
            &self.gix_repo,
            &self.remote_name,
            &url,
            branch.local_sha(),
            &remote_refname,
            self.run_husky_hooks,
        )? {
            hooks::HookResult::Success | hooks::HookResult::NotConfigured => Ok(()),
            hooks::HookResult::Failure(error_data) => Err(anyhow::anyhow!(
                "pre-push hook failed: {}",
                error_data.error
            )),
        }
    }

    fn gerrit_push_args(&self, head: gix::ObjectId) -> GerritPushArgs {
        if self.gerrit_mode {
            GerritPushArgs {
                refspec: Some(format!("{head}:refs/for/{}", self.target_branch_name)),
                push_opts: self
                    .options
                    .push_flags
                    .iter()
                    .map(|flag| flag.to_string())
                    .collect(),
            }
        } else {
            GerritPushArgs {
                refspec: None,
                push_opts: vec![],
            }
        }
    }

    fn record_gerrit_push_metadata(&self, ctx: &Context, push_output: &str) -> Result<()> {
        if !self.gerrit_mode {
            return Ok(());
        }

        let push_output = but_gerrit::parse::push_output(push_output)?;
        let range = self.plan.gerrit_metadata_range();
        let candidate_ids = first_parent_commit_ids_until(&self.gix_repo, range.head, range.base)?;
        but_gerrit::record_push_metadata(ctx, candidate_ids, push_output)
    }
}

struct GerritPushArgs {
    refspec: Option<String>,
    push_opts: Vec<String>,
}

fn append_push_result(result: &mut PushResult, branch: &PushBranch) {
    result
        .branch_to_remote
        .push((branch.name().to_owned(), branch.remote_refname().to_owned()));
    result.branch_sha_updates.push((
        branch.name().to_owned(),
        branch.before_sha().to_string(),
        branch.local_sha().to_string(),
    ));
}
