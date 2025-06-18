use crate::integrated::IsCommitIntegrated;
use crate::ui::{CommitState, PushStatus, StackDetails};
use crate::{
    RefInfo, StacksFilter, branch, head_info, id_from_name_v2_to_v3, ref_info, state_handle, ui,
};
use anyhow::Context;
use bstr::BString;
use but_core::RefMetadata;
use but_graph::{
    Commit, LocalCommit, LocalCommitRelation, RemoteCommit, Segment, SegmentMetadata,
    VirtualBranchesTomlMetadata,
};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oxidize::{ObjectIdExt, OidExt, git2_signature_to_gix_signature};
use gitbutler_stack::{Stack, StackBranch, StackId};
use gix::date::parse::TimeBuf;
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
        .context("Every V2/V3 stack has a name as long as it's in a gitbutler workspace")?
        .to_owned();
    let heads: Vec<_> = stack
        .segments
        .into_iter()
        .map(|segment| -> anyhow::Result<_> {
            let ref_name = segment
                .ref_name
                .context("This type can't represent this state and it shouldn't have to")?;
            Ok(ui::StackHeadInfo {
                tip: repo
                    .find_reference(ref_name.as_ref())
                    .ok()
                    .and_then(|r| r.try_id())
                    .map(|id| id.detach())
                    .unwrap_or(repo.object_hash().null()),
                name: ref_name.shorten().into(),
            })
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(ui::StackEntry {
        id: id_from_name_v2_to_v3(name.as_ref(), meta)?,
        tip: heads
            .first()
            .map(|h| h.tip)
            .unwrap_or(repo.object_hash().null()),
        heads,
        order: None,
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

            let Some(reference) = repo.try_find_reference(ref_name.as_ref())? else {
                continue;
            };
            let tip = reference
                .try_id()
                .with_context(|| format!("Encountered symbolic reference: {ref_name}"))?
                .detach();
            out.push(ui::StackEntry {
                id: id_from_name_v2_to_v3(ref_name.as_ref(), meta)?,
                // TODO: this is just a simulation and such a thing doesn't really exist in the V3 world, let's see how it goes.
                //       Thus, we just pass ourselves as first segment, similar to having no other segments.
                heads: vec![ui::StackHeadInfo {
                    name: ref_name.shorten().into(),
                    tip,
                }],
                tip,
                order: None,
            })
        }
        Ok(out)
    }

    let info = head_info(
        repo,
        meta,
        ref_info::Options {
            // TODO: set this to a good value for the UI to not slow down, and also a value that forces us to re-investigate this.
            stack_commit_limit: 100,
            expensive_commit_info: false,
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

/// Get additional information for the stack identified by `stack_id`.
// TODO: StackId shouldn't be used, instead use the ref-name or stack index as universal tip identifier.
//       It's notable that there isn't always a ref-name available right now in case the ref advanced, but maybe this is something
//       we can pull out of the metadata information.
pub fn stack_details_v3(
    stack_id: StackId,
    repo: &gix::Repository,
    meta: &VirtualBranchesTomlMetadata,
) -> anyhow::Result<ui::StackDetails> {
    fn stack_by_id(
        head_info: RefInfo,
        stack_id: StackId,
        meta: &VirtualBranchesTomlMetadata,
    ) -> anyhow::Result<Option<branch::Stack>> {
        let stacks_with_id: Vec<_> = head_info
            .stacks
            .into_iter()
            .filter_map(|stack| {
                let name = stack.name()?.to_owned();
                Some(id_from_name_v2_to_v3(name.as_ref(), meta).map(|stack_id| (stack_id, stack)))
            })
            .collect::<Result<_, _>>()?;

        Ok(stacks_with_id
            .into_iter()
            .find_map(|(id, stack)| (id == stack_id).then_some(stack)))
    }
    let ref_info_options = ref_info::Options {
        stack_commit_limit: 0,
        expensive_commit_info: true,
    };
    let stack = meta.data().branches.get(&stack_id).with_context(|| {
        format!("Couldn't find {stack_id} even when looking at virtual_branches.toml directly")
    })?;
    let full_name = gix::refs::FullName::try_from(format!(
        "refs/heads/{shortname}",
        shortname = stack.derived_name()?
    ))?;
    let existing_ref = repo.find_reference(&full_name)?;
    let stack = stack_by_id(ref_info(existing_ref, meta, ref_info_options)?, stack_id, meta)?
        .with_context(|| format!("Really couldn't find {stack_id} in current HEAD or when searching virtual_branches.toml plainly"))?;

    let branch_details = stack
        .segments
        .iter()
        .enumerate()
        .map(|(idx, segment)| {
            ui::BranchDetails::from_segment(
                segment,
                stack
                    .segments
                    .get(idx + 1)
                    .and_then(|below| below.commits.first().map(|commit| commit.id))
                    .or(stack.base),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let topmost_branch = branch_details
        .first()
        .context("Stacks should never be empty")?;
    Ok(StackDetails {
        derived_name: topmost_branch.name.to_string(),
        push_status: topmost_branch.push_status,
        is_conflicted: topmost_branch.is_conflicted,
        branch_details,
    })
}

impl ui::BranchDetails {
    fn from_segment(
        Segment {
            id: _,
            ref_name,
            commits: commits_unique_from_tip,
            commits_unique_in_remote_tracking_branch,
            remote_tracking_ref_name,
            metadata,
        }: &Segment,
        previous_tip_or_stack_base: Option<gix::ObjectId>,
    ) -> anyhow::Result<Self> {
        let ref_name = ref_name
            .clone()
            .context("Can't handle a stack yet whose tip isn't pointed to by a ref")?;
        let (description, updated_at, review_id, pr_number) = metadata
            .clone()
            .and_then(|meta| match meta {
                SegmentMetadata::Branch(meta) => Some((
                    meta.description,
                    meta.ref_info.updated_at,
                    meta.review.review_id,
                    meta.review.pull_request,
                )),
                SegmentMetadata::Workspace(_) => None,
            })
            .unwrap_or_default();
        let base_commit = previous_tip_or_stack_base
            // This case is for unborn branches only where there is a segment with nothing
            .unwrap_or_else(|| gix::ObjectId::null(gix::hash::Kind::Sha1));
        Ok(ui::BranchDetails {
            is_remote_head: ref_name
                .category()
                .is_some_and(|c| matches!(c, gix::refs::Category::RemoteBranch)),
            name: ref_name.shorten().into(),
            remote_tracking_branch: remote_tracking_ref_name
                .as_ref()
                .map(|full_name| full_name.as_bstr().into()),
            description,
            pr_number,
            review_id,
            tip: commits_unique_from_tip
                .first()
                .map(|commit| commit.id)
                .unwrap_or(base_commit),
            base_commit,
            push_status: PushStatus::derive_from_commits(
                remote_tracking_ref_name.is_some(),
                commits_unique_from_tip,
                commits_unique_in_remote_tracking_branch,
            ),
            last_updated_at: updated_at.map(|time| time.seconds as i128 * 1_000),
            authors: {
                let mut authors = HashSet::<ui::Author>::new();
                let all_commits = commits_unique_from_tip.iter().map(|c| &c.inner).chain(
                    commits_unique_in_remote_tracking_branch
                        .iter()
                        .map(|c| &c.inner),
                );
                for commit in all_commits {
                    authors.insert((commit.author.to_ref(&mut TimeBuf::default())).into());
                }
                let mut authors: Vec<_> = authors.into_iter().collect();
                authors.sort_by(|a, b| a.name.cmp(&b.name));
                authors
            },
            commits: commits_unique_from_tip.iter().map(Into::into).collect(),
            is_conflicted: commits_unique_from_tip.iter().any(|c| c.has_conflicts),
            upstream_commits: commits_unique_in_remote_tracking_branch
                .iter()
                .map(Into::into)
                .collect(),
        })
    }
}

impl PushStatus {
    /// Derive the push-status by looking at commits in the local and remote tracking branches.
    /// TODO: tests
    ///       * generally this doesn't currently handle advanced (and possibly fast-forwardable)
    ///         remotes very well. It doesn't feel like it can be expressed.
    ///       * It doesn't deal with diverged local/remote branches.
    ///       * Special cases of remote is merged, and remote tracking branch is deleted after fetch
    ///         if it was deleted on the remote?
    fn derive_from_commits(
        has_remote_tracking_ref: bool,
        commits_unique_from_tip: &[LocalCommit],
        commits_unique_in_remote_tracking_branch: &[RemoteCommit],
    ) -> Self {
        if !has_remote_tracking_ref {
            // Generally, don't do anything if no remote relationship is set up (anymore).
            // There may be better ways to deal with this.
            return PushStatus::CompletelyUnpushed;
        }

        let everything_local = commits_unique_from_tip
            .last()
            .is_some_and(|c| matches!(c.relation, LocalCommitRelation::LocalOnly));
        let everything_integrated_locally = commits_unique_from_tip
            .last()
            .is_some_and(|c| matches!(c.relation, LocalCommitRelation::Integrated));
        if everything_integrated_locally {
            PushStatus::Integrated
        } else if everything_local {
            if commits_unique_in_remote_tracking_branch.is_empty() {
                PushStatus::UnpushedCommits
            } else {
                PushStatus::UnpushedCommitsRequiringForce
            }
        } else {
            // Local commits intersect with remote, either by similarity or identity.
            if commits_unique_from_tip
                .last()
                .is_some_and(|c| matches!(c.relation, LocalCommitRelation::LocalAndRemote(remote_id) if remote_id != c.id)) {
                PushStatus::UnpushedCommitsRequiringForce
            } else {
                // TODO: There could be remote commits that can be forwarded to, but that can't be expressed.
                //       Probably an update would fix it automatically.
                PushStatus::NothingToPush
            }
        }
    }
}

impl From<&RemoteCommit> for ui::UpstreamCommit {
    fn from(
        RemoteCommit {
            inner:
                Commit {
                    id,
                    parent_ids: _,
                    message,
                    author,
                    // TODO: also pass refs for the frontend.
                    refs: _,
                    // TODO: also pass flags for the frontend.
                    flags: _,
                },
            // TODO: Represent this in the UI (maybe) and/or deal with divergence of the local and remote tracking branch.
            has_conflicts: _,
        }: &RemoteCommit,
    ) -> Self {
        ui::UpstreamCommit {
            id: *id,
            message: message.clone(),
            created_at: author.time.seconds as i128 * 1000,
            author: author
                .to_ref(&mut gix::date::parse::TimeBuf::default())
                .into(),
        }
    }
}

impl From<&LocalCommit> for ui::Commit {
    fn from(
        LocalCommit {
            inner:
                Commit {
                    id,
                    parent_ids,
                    message,
                    author,
                    // TODO: also pass refs
                    refs: _,
                    // TODO: also flags refs
                    flags: _,
                },
            relation,
            has_conflicts,
        }: &LocalCommit,
    ) -> Self {
        ui::Commit {
            id: *id,
            parent_ids: parent_ids.clone(),
            message: message.clone(),
            has_conflicts: *has_conflicts,
            state: (*relation).into(),
            created_at: author.time.seconds as i128 * 1000,
            author: author
                .to_ref(&mut gix::date::parse::TimeBuf::default())
                .into(),
        }
    }
}

impl From<LocalCommitRelation> for ui::CommitState {
    fn from(value: LocalCommitRelation) -> Self {
        use ui::CommitState as E;
        match value {
            LocalCommitRelation::LocalOnly => E::LocalOnly,
            LocalCommitRelation::LocalAndRemote(id) => E::LocalAndRemote(id),
            LocalCommitRelation::Integrated => E::Integrated,
        }
    }
}

/// Return the branches that belong to a particular [`Stack`]
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
    let mut check_commit = IsCommitIntegrated::new(repo, ctx.repo(), &default_target, &mut graph)?;

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
