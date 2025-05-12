use crate::integrated::IsCommitIntegrated;
use crate::ui::{CommitState, PushStatus};
use crate::{
    StacksFilter, VirtualBranchesTomlMetadata, branch, head_info, id_from_name_v2_to_v3,
    state_handle, ui,
};
use anyhow::Context;
use bstr::BString;
use but_core::RefMetadata;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oxidize::{ObjectIdExt, OidExt, git2_signature_to_gix_signature};
use gitbutler_stack::{Stack, StackBranch, StackId};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Returns the list of branch information for the branches in a stack.
pub fn stack_heads_info(
    stack: &Stack,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<ui::StackHeadInfo>> {
    let branches = stack
        .branches()
        .into_iter()
        .rev()
        .filter_map(|branch| {
            let tip = branch.head_oid(repo).ok()?;
            Some(ui::StackHeadInfo {
                name: branch.name().to_owned().into(),
                tip,
            })
        })
        .collect::<Vec<_>>();

    Ok(branches)
}

/// Returns the list of stacks that are currently part of the workspace.
/// If there are no applied stacks, the returned Vec is empty.
/// If the GitButler state file in the provided path is missing or invalid, an error is returned.
///
/// - `gb_dir`: The path to the GitButler state for the project. Normally this is `.git/gitbutler` in the project's repository.
pub fn stacks(
    ctx: &CommandContext,
    gb_dir: &Path,
    repo: &gix::Repository,
    filter: StacksFilter,
) -> anyhow::Result<Vec<ui::StackEntry>> {
    let state = state_handle(gb_dir);

    let stacks = match filter {
        StacksFilter::All => state.list_all_stacks()?,
        StacksFilter::InWorkspace => state
            .list_all_stacks()?
            .into_iter()
            .filter(|s| s.in_workspace)
            .collect(),
        StacksFilter::Unapplied => state
            .list_all_stacks()?
            .into_iter()
            .filter(|s| !s.in_workspace)
            .collect(),
    };

    let stacks = stacks
        .into_iter()
        .filter_map(|mut stack| stack.migrate_change_ids(ctx).ok().map(|()| stack))
        .filter(|s| s.is_initialized());

    stacks
        .sorted_by_key(|s| s.order)
        .map(|stack| ui::StackEntry::try_new(repo, &stack))
        .collect()
}

fn try_from_stack_v3(
    repo: &gix::Repository,
    stack: branch::Stack,
    meta: &VirtualBranchesTomlMetadata,
) -> anyhow::Result<ui::StackEntry> {
    let name = stack
        .name()
        .expect("Every V2/V3 stack has a name as long as it's in a gitbutler workspace")
        .to_owned();
    let heads = stack.segments.into_iter().map(|segment| {
        let ref_name = segment
            .ref_name
            .expect("This type can't represent this state and it shouldn't have to");
        ui::StackHeadInfo {
            tip: repo
                .find_reference(ref_name.as_ref())
                .ok()
                .and_then(|r| r.try_id())
                .map(|id| id.detach())
                .unwrap_or(repo.object_hash().null()),
            name: ref_name.into(),
        }
    });
    Ok(ui::StackEntry {
        id: id_from_name_v2_to_v3(name.as_ref(), meta)?,
        heads: heads.collect(),
        tip: stack.tip.unwrap_or(repo.object_hash().null()),
    })
}

/// Returns the list of stacks that pass `filter`, in unspecified order.
///
/// Use `repo` and `meta` to read branches data
// TODO: See if the UI can migrate to `head_info()` or a variant of it so the information is only called once.
pub fn stacks_v3(
    repo: &gix::Repository,
    meta: &VirtualBranchesTomlMetadata,
    filter: StacksFilter,
) -> anyhow::Result<Vec<ui::StackEntry>> {
    // TODO: See if this works at all once VirtualBranches.toml isn't the backing anymore.
    //       Probably needs to change, maybe even alongside the notion of 'unapplied'.
    //       In future, unapplied stacks could just be stacks, either with one segment, or multiple ones - any branch with another branch
    //       found while traversing its commits to some base becomes a stack in that very sense.
    fn unapplied_stacks(
        repo: &gix::Repository,
        meta: &VirtualBranchesTomlMetadata,
        applied_stacks: &[branch::Stack],
    ) -> anyhow::Result<Vec<ui::StackEntry>> {
        let mut out = Vec::new();
        for item in meta.iter() {
            let (ref_name, ref_meta) = item?;
            if !ref_meta.is::<but_core::ref_metadata::Branch>() {
                continue;
            };
            let is_applied = applied_stacks.iter().any(|stack| {
                stack.segments.iter().any(|segment| {
                    segment
                        .ref_name
                        .as_ref()
                        .is_some_and(|name| name == &ref_name)
                })
            });
            if is_applied {
                continue;
            }

            let reference = repo.find_reference(ref_name.as_ref())?;
            let tip = reference
                .try_id()
                .with_context(|| format!("Encountered symbolic reference: {ref_name}"))?
                .detach();
            out.push(ui::StackEntry {
                id: id_from_name_v2_to_v3(ref_name.as_ref(), meta)?,
                // TODO: this is just a simulation and such a thing doesn't really exist in the V3 world, let's see how it goes.
                //       Thus, we just pass ourselves as first segment, similar to having no other segments.
                heads: vec![ui::StackHeadInfo {
                    name: ref_name.into(),
                    tip,
                }],
                tip,
            })
        }
        Ok(out)
    }

    let info = crate::head_info(
        repo,
        meta,
        head_info::Options {
            // TODO: set this to a good value for the UI to not slow down, and also a value that forces us to re-investigate this.
            stack_commit_limit: 100,
        },
    )?;

    fn into_ui_stacks(
        repo: &gix::Repository,
        stacks: Vec<branch::Stack>,
        meta: &VirtualBranchesTomlMetadata,
    ) -> Vec<ui::StackEntry> {
        stacks
            .into_iter()
            .filter_map(|stack| try_from_stack_v3(repo, stack, meta).ok())
            .collect()
    }

    let unapplied_stacks = unapplied_stacks(repo, meta, &info.stacks)?;
    Ok(match filter {
        StacksFilter::InWorkspace => into_ui_stacks(repo, info.stacks, meta),
        StacksFilter::All => {
            let mut all_stacks = unapplied_stacks;
            all_stacks.extend(into_ui_stacks(repo, info.stacks, meta));
            all_stacks
        }
        StacksFilter::Unapplied => unapplied_stacks,
    })
}

/// Returns information about the current state of a stack.
/// If the stack is not found, an error is returned.
///
/// - `gb_dir`: The path to the GitButler state for the project. Normally this is `.git/gitbutler` in the project's repository.
/// - `stack_id`: The ID of the stack to get information about.
/// - `ctx`: The command context for the project.
pub fn stack_details(
    gb_dir: &Path,
    stack_id: StackId,
    ctx: &CommandContext,
) -> anyhow::Result<ui::StackDetails> {
    /// Determines if a force push is required to push a branch to its remote.
    fn requires_force(
        ctx: &CommandContext,
        branch: &StackBranch,
        remote: &str,
    ) -> anyhow::Result<bool> {
        let upstream = branch.remote_reference(remote);

        let upstream_reference = match ctx.repo().refname_to_id(&upstream) {
            Ok(reference) => reference,
            Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(false),
            Err(other) => return Err(other).context("failed to find upstream reference"),
        };

        let upstream_commit = ctx
            .repo()
            .find_commit(upstream_reference)
            .context("failed to find upstream commit")?;

        let branch_head = branch.head_oid(&ctx.gix_repo()?)?;
        let merge_base = ctx
            .repo()
            .merge_base(upstream_commit.id(), branch_head.to_git2())?;

        Ok(merge_base != upstream_commit.id())
    }

    #[derive(Debug, Default)]
    struct BranchState {
        is_integrated: bool,
        is_dirty: bool,
        requires_force: bool,
        has_pushed_commits: bool,
    }

    impl From<BranchState> for PushStatus {
        fn from(state: BranchState) -> Self {
            match (
                state.is_integrated,
                state.is_dirty,
                state.requires_force,
                state.has_pushed_commits,
            ) {
                (true, _, _, _) => PushStatus::Integrated,
                (_, true, _, false) => PushStatus::CompletelyUnpushed,
                (_, _, true, _) => PushStatus::UnpushedCommitsRequiringForce,
                (_, true, _, _) => PushStatus::UnpushedCommits,
                (_, false, _, _) => PushStatus::NothingToPush,
            }
        }
    }

    let state = state_handle(gb_dir);
    let mut stack = state.get_stack(stack_id)?;
    let branches = stack.branches();
    let branches = branches.iter().filter(|b| !b.archived);
    let repo = ctx.gix_repo()?;
    let remote = state
        .get_default_target()
        .context("failed to get default target")?
        .push_remote_name();

    let mut stack_state = BranchState::default();
    let mut stack_is_conflicted = false;
    let mut branch_details = vec![];
    let mut current_base = stack.merge_base(ctx)?;

    for branch in branches {
        let upstream_reference = ctx
            .repo()
            .find_reference(&branch.remote_reference(remote.as_str()))
            .ok()
            .map(|_| branch.remote_reference(remote.as_str()));

        let mut branch_state = BranchState {
            requires_force: requires_force(ctx, branch, &remote)?,
            ..Default::default()
        };

        let mut is_conflicted = false;
        let mut authors = HashSet::new();
        let commits = local_and_remote_commits(ctx, &repo, branch, &stack)?;
        let upstream_commits = upstream_only_commits(ctx, &repo, branch, &stack, Some(&commits))?;

        // If there are commits in the remote, we can assume that commits have been pushed. *Like, literally*.
        branch_state.has_pushed_commits |= !upstream_commits.is_empty();

        for commit in &commits {
            is_conflicted |= commit.has_conflicts;
            branch_state.is_dirty |= matches!(commit.state, ui::CommitState::LocalOnly);
            branch_state.has_pushed_commits |=
                matches!(commit.state, CommitState::LocalAndRemote(_));
            authors.insert(commit.author.clone());
        }

        // We can assume that if the child-most commit is integrated, the whole branch is integrated
        branch_state.is_integrated = matches!(
            commits.first().map(|c| &c.state),
            Some(CommitState::Integrated)
        );

        stack_is_conflicted |= is_conflicted;
        stack_state.is_dirty |= branch_state.is_dirty;
        stack_state.requires_force |= branch_state.requires_force;
        stack_state.has_pushed_commits |= branch_state.has_pushed_commits;

        // If all branches are integrated, the stack is integrated
        stack_state.is_integrated &= branch_state.is_integrated;

        branch_details.push(ui::BranchDetails {
            name: branch.name().to_owned().into(),
            remote_tracking_branch: upstream_reference.map(Into::into),
            description: branch.description.clone(),
            pr_number: branch.pr_number,
            review_id: branch.review_id.clone(),
            tip: branch.head_oid(&repo)?,
            base_commit: current_base,
            push_status: branch_state.into(),
            last_updated_at: commits.first().map(|c| c.created_at),
            authors: authors.into_iter().collect(),
            is_conflicted,
            commits,
            upstream_commits,
            is_remote_head: false,
        });

        current_base = branch.head_oid(&repo)?;
    }

    stack.migrate_change_ids(ctx).ok(); // If it fails thats ok - best effort migration
    branch_details.reverse();

    let push_status = stack_state.into();

    Ok(ui::StackDetails {
        derived_name: stack.derived_name()?,
        push_status,
        branch_details,
        is_conflicted: stack_is_conflicted,
    })
}

/// Return the branches that belong to a particular [`gitbutler_stack::Stack`]
/// The entries are ordered from newest to oldest.
pub fn stack_branches(stack_id: StackId, ctx: &CommandContext) -> anyhow::Result<Vec<ui::Branch>> {
    let state = state_handle(&ctx.project().gb_dir());
    let remote = state
        .get_default_target()
        .context("failed to get default target")?
        .push_remote_name();

    let mut stack_branches = vec![];
    let mut stack = state.get_stack(stack_id)?;
    let mut current_base = stack.merge_base(ctx)?;
    let repo = ctx.gix_repo()?;
    for internal in stack.branches() {
        let upstream_reference = ctx
            .repo()
            .find_reference(&internal.remote_reference(remote.as_str()))
            .ok()
            .map(|_| internal.remote_reference(remote.as_str()));
        let result = ui::Branch {
            name: internal.name().to_owned().into(),
            remote_tracking_branch: upstream_reference.map(Into::into),
            description: internal.description.clone(),
            pr_number: internal.pr_number,
            review_id: internal.review_id.clone(),
            archived: internal.archived,
            tip: internal.head_oid(&repo)?,
            base_commit: current_base,
        };
        current_base = internal.head_oid(&repo)?;
        stack_branches.push(result);
    }
    stack.migrate_change_ids(ctx).ok(); // If it fails thats ok - best effort migration
    stack_branches.reverse();
    Ok(stack_branches)
}

/// Returns a list of commits beloning to this branch. Ordered from newest to oldest (child-most to parent-most).
///
/// These are the commits that are currently part of the workspace (applied).
/// Created from the local pseudo branch (head currently stored in the TOML file)
///
/// When there is only one branch in the stack, this includes the commits
/// from the tip of the stack to the merge base with the trunk / target branch (not including the merge base).
///
/// When there are multiple branches in the stack, this includes the commits from the branch head to the next branch in the stack.
///
/// In either case, this is effectively a list of commits that in the working copy which may or may not have been pushed to the remote.
pub fn stack_branch_local_and_remote_commits(
    stack_id: StackId,
    branch_name: String,
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<ui::Commit>> {
    let state = state_handle(&ctx.project().gb_dir());
    let stack = state.get_stack(stack_id)?;

    let branches = stack.branches();
    let branch = branches
        .iter()
        .find(|b| b.name() == &branch_name)
        .ok_or_else(|| anyhow::anyhow!("Could not find branch {:?}", branch_name))?;
    if branch.archived {
        return Ok(vec![]);
    }
    local_and_remote_commits(ctx, repo, branch, &stack)
}

/// Returns a fift of commits belonging to this branch. Ordered from newest to oldest (child-most to parent-most).
///
/// These are the commits that exist **only** on the upstream branch. Ordered from newest to oldest.
/// Created from the tip of the local tracking branch eg. refs/remotes/origin/my-branch -> refs/heads/my-branch
///
/// This does **not** include the commits that are in the commits list (local)
/// This is effectively the list of commits that are on the remote branch but are not in the working copy.
pub fn stack_branch_upstream_only_commits(
    stack_id: StackId,
    branch_name: String,
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<ui::UpstreamCommit>> {
    let state = state_handle(&ctx.project().gb_dir());
    let stack = state.get_stack(stack_id)?;

    let branches = stack.branches();
    let branch = branches
        .iter()
        .find(|b| b.name() == &branch_name)
        .with_context(|| format!("Could not find branch {branch_name:?}"))?;
    if branch.archived {
        return Ok(vec![]);
    }
    upstream_only_commits(ctx, repo, branch, &stack, None)
}

fn upstream_only_commits(
    ctx: &CommandContext,
    repo: &gix::Repository,
    stack_branch: &StackBranch,
    stack: &Stack,
    current_local_and_remote_commits: Option<&Vec<ui::Commit>>,
) -> anyhow::Result<Vec<ui::UpstreamCommit>> {
    let branch_commits = stack_branch.commits(ctx, stack)?;

    let local_and_remote = if let Some(current_local_and_remote) = current_local_and_remote_commits
    {
        current_local_and_remote
    } else {
        &local_and_remote_commits(ctx, repo, stack_branch, stack)?
    };

    // Upstream only
    let mut upstream_only = vec![];
    for commit in branch_commits.upstream_only.iter() {
        let matches_known_commit = local_and_remote.iter().any(|c| {
            // If the id matches verbatim or if there is a known remote_id (in the case of LocalAndRemote) that matchies
            c.id == commit.id().to_gix() || matches!(&c.state, CommitState::LocalAndRemote(remote_id) if remote_id == &commit.id().to_gix())
        });
        // Ignore commits that strictly speaking are remote only, but they match a known local commit (rebase etc)
        if !matches_known_commit {
            let created_at = i128::from(commit.time().seconds()) * 1000;
            let upstream_commit = ui::UpstreamCommit {
                id: commit.id().to_gix(),
                message: commit.message_bstr().into(),
                created_at,
                author: commit.author().into(),
            };
            upstream_only.push(upstream_commit);
        }
    }
    upstream_only.reverse();

    Ok(upstream_only)
}

fn local_and_remote_commits(
    ctx: &CommandContext,
    repo: &gix::Repository,
    stack_branch: &gitbutler_stack::StackBranch,
    stack: &Stack,
) -> anyhow::Result<Vec<ui::Commit>> {
    let state = state_handle(&ctx.project().gb_dir());
    let default_target = state
        .get_default_target()
        .context("failed to get default target")?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let mut check_commit = IsCommitIntegrated::new(ctx.repo(), &default_target, repo, &mut graph)?;

    let branch_commits = stack_branch.commits(ctx, stack)?;
    let mut local_and_remote: Vec<ui::Commit> = vec![];
    let mut is_integrated = false;

    let remote_commit_data = branch_commits
        .remote_commits
        .iter()
        .filter_map(|commit| {
            let data = CommitData::try_from(commit).ok()?;
            Some((data, commit.id()))
        })
        .collect::<HashMap<_, _>>();

    // Local and remote
    // Reverse first instead of later, so that we catch the first integrated commit
    for commit in branch_commits.clone().local_commits.iter().rev() {
        if !is_integrated {
            is_integrated = check_commit.is_integrated(commit)?;
        }
        let copied_from_remote_id = CommitData::try_from(commit)
            .ok()
            .and_then(|data| remote_commit_data.get(&data).copied());

        let state = if is_integrated {
            CommitState::Integrated
        } else {
            // Can we find this as a remote commit by any of these options:
            // - the commit is copied from a remote commit
            // - the commit has an identical sha as the remote commit (the no brainer case)
            // - the commit has a change id that matches a remote commit
            if let Some(remote_id) = copied_from_remote_id {
                CommitState::LocalAndRemote(remote_id.to_gix())
            } else if let Some(remote_id) = branch_commits
                .remote_commits
                .iter()
                .find(|c| c.id() == commit.id() || c.change_id() == commit.change_id())
                .map(|c| c.id())
            {
                CommitState::LocalAndRemote(remote_id.to_gix())
            } else {
                CommitState::LocalOnly
            }
        };

        let created_at = i128::from(commit.time().seconds()) * 1000;

        let api_commit = ui::Commit {
            id: commit.id().to_gix(),
            parent_ids: commit.parents().map(|p| p.id().to_gix()).collect(),
            message: commit.message_bstr().into(),
            has_conflicts: commit.is_conflicted(),
            state,
            created_at,
            author: commit.author().into(),
        };
        local_and_remote.push(api_commit);
    }

    Ok(local_and_remote)
}

/// The commit-data we can use for comparison to see which remote-commit was used to craete
/// a local commit from.
/// Note that trees can't be used for comparison as these are typically rebased.
#[derive(Debug, Hash, Eq, PartialEq)]
pub(crate) struct CommitData {
    message: BString,
    author: gix::actor::Signature,
}

impl TryFrom<&git2::Commit<'_>> for CommitData {
    type Error = anyhow::Error;

    fn try_from(commit: &git2::Commit<'_>) -> std::result::Result<Self, Self::Error> {
        Ok(CommitData {
            message: commit.message_raw_bytes().into(),
            author: git2_signature_to_gix_signature(commit.author()),
        })
    }
}
