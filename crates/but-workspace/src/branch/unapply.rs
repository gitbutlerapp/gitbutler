use crate::branch::OnWorkspaceMergeConflict;
use but_core::ref_metadata::StackId;
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use std::borrow::Cow;

/// Returned by [unapply()](function::unapply()).
pub struct Outcome<'workspace> {
    /// The newly created workspace, if owned, or the one that was passed in if borrowed, to show how the workspace looks like now.
    ///
    /// If borrowed, the graph already didn't contain the desired branch and nothing had to be unapplied. Note that metadata changes
    /// might not be included in this case, as they aren't the source of truth.
    pub workspace: Cow<'workspace, but_graph::projection::Workspace>,
    /// The unapply operation ended in checking out the last remaining stack in the workspace, whose tip name is listed here.
    /// This will only happen if the commit to check out is named.
    ///
    /// If a remote tracking branch is given to apply, it will actually apply its local tracking branch, which is created on demand as well.
    /// Further, if there is no target or if the current branch isn't the target branch, then the current branch and the given one
    /// will be applied.
    pub checked_out: Option<gix::refs::FullName>,
    /// `true` if we created the given workspace ref as it didn't exist yet.
    pub workspace_ref_created: bool,
    /// If not `None`, an actual merge was attempted, but depending on [the settings](OnWorkspaceMergeConflict),
    /// this was persisted or not.
    pub workspace_merge: Option<crate::commit::merge::Outcome>,
    /// The ids of all stacks that were conflicting and thus didn't get applied, and tip ref names can be derived from that.
    pub conflicting_stack_ids: Vec<StackId>,
}

impl Outcome<'_> {
    /// Return `true` if a new graph traversal was performed, which always is a sign for an operation which changed the workspace.
    /// This is `false` if the to be unapplied branch was already outside the workspace.
    pub fn workspace_changed(&self) -> bool {
        matches!(self.workspace, Cow::Owned(_))
    }
}

impl<'a> Outcome<'a> {
    /// Convert this instance into a fully-owned one.
    pub fn into_owned(self) -> Outcome<'static> {
        let Outcome {
            workspace,
            checked_out,
            workspace_ref_created,
            workspace_merge,
            conflicting_stack_ids,
        } = self;

        Outcome {
            workspace: Cow::Owned(workspace.into_owned()),
            checked_out,
            workspace_ref_created,
            workspace_merge,
            conflicting_stack_ids,
        }
    }
}

impl std::fmt::Debug for Outcome<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Outcome {
            workspace: _,
            checked_out,
            workspace_ref_created,
            workspace_merge: _,
            conflicting_stack_ids,
        } = self;
        let mut f = f.debug_struct("Outcome");
        f.field("workspace_changed", &self.workspace_changed())
            .field("workspace_ref_created", workspace_ref_created);
        if let Some(checked_out) = checked_out {
            f.field("checked_out", &checked_out.to_string());
        }
        if !conflicting_stack_ids.is_empty() {
            f.field("conflicting_stack_ids", conflicting_stack_ids);
        }
        f.finish()
    }
}

/// How to treat the workspace merge commit when [unapplying](function::unapply()) it is not technically required anymore.
///
/// This happens when the amount of stacks goes from `2` to `1`.
/// It also happens when a stack is unapplied and the workspace commit only has a single commit merged into it.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkspaceMergeCommit {
    /// Never remove the workspace merge commit. This allows workspaces with 1 stack, or empty workspaces.
    /// can connect directly with the *one* workspace base.
    /// This also ensures that there is a workspace merge commit, even if it is none-sensical.
    #[default]
    Keep,
    /// Remove a workspace merge commit by pointing the workspace reference elsewhere, or
    /// by [deleting](WorkspaceReference) the workspace reference.
    /// Removal happens if the workspace merge commit would only connect to a commit owned by a single stack,
    /// or if there is no stack left at all and the workspace is empty.
    /// Note that workspace commits also don't even have to be present if there are one or more virtual stacks,
    /// as these don't have commits on their own.
    // TODO: make this the default when this is the default in apply()
    RemoveIfPossible,
}

/// Decide what to do with the workspace reference (typically `gitbutler/workspace`) when it's not needed anymore.
///
/// It's not needed anymore when switching away from it to another branch, which may happen only if the workspace commit
/// isn't needed anymore and is [configured](WorkspaceMergeCommit) to be forgotten in that case.
/// *Additionally*, it may only happen if it's a managed reference, i.e. in `refs/heads/gitbutler/`, and if there is only
/// one *named* stack left which then as well may be checked out directly.
#[derive(Default, Debug, Copy, Clone)]
pub enum WorkspaceReference {
    /// No matter what, keep the reference for its metadata, *and* keep it checked out.
    #[default]
    KeepCheckedOut,
    /// Keep the reference for its metadata, but allow switching to the last remaining stack.
    KeepButAllowSwitchingToRemainingStack,
    /// Delete the workspace metadata and the workspace reference after switching to the last remaining stack.
    DeleteAfterSwitchingToRemainingStack,
}

/// Options for [branch::unapply()](function::unapply()).
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How the branch should be brought into the workspace.
    pub workspace_merge_commit: WorkspaceMergeCommit,
    /// Decide how to deal with conflicts when updating the workspace merge commit after removing a stack.
    ///
    /// Note that it should be incredibly unlikely, but can we prove it's impossible?
    pub on_workspace_conflict: OnWorkspaceMergeConflict,
    /// What to do with the workspace reference after unapplying.
    pub workspace_reference: WorkspaceReference,
    /// How the worktree checkout should behave when uncommitted changes are present in the worktree that it would
    /// want to modify to accommodate the new workspace commit, with the unapplied stack removed.
    pub uncommitted_changes: UncommitedWorktreeChanges,
}

pub(crate) mod function {
    use std::borrow::Cow;

    use super::{Options, Outcome, WorkspaceMergeCommit, WorkspaceReference};
    use crate::ref_info::WorkspaceExt;
    use anyhow::{Context as _, bail};
    use but_core::{
        ObjectStorageExt, RefMetadata, RepositoryExt,
        branch::SafeDelete,
        ref_metadata::{
            StackId, StackKind::AppliedAndUnapplied, Workspace, WorkspaceCommitRelation,
        },
    };
    use but_graph::projection::WorkspaceKind;
    use gix::{
        reference::Category,
        refs::{
            FullNameRef, Target,
            transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
        },
    };

    use crate::{WorkspaceCommit, commit::merge::Tip};

    /// Alter the `workspace` so that `branch` can't be found in it anymore as visible Stack.
    /// This requires a managed workspace, as ad-hoc workspaces, i.e. single-branch mode, doesn't support
    /// parallel branches.
    ///
    /// If the workspace tree changed, but the workspace isn't checked out, we will not check out the new workspace.
    /// Otherwise, we will do that though.
    pub fn unapply<'ws>(
        branch: &FullNameRef,
        workspace: &'ws but_graph::projection::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            workspace_merge_commit,
            on_workspace_conflict,
            workspace_reference,
            uncommitted_changes,
        }: Options,
    ) -> anyhow::Result<Outcome<'ws>> {
        if workspace.has_workspace_commit_in_ancestry(repo) {
            bail!("Refusing to work on workspace whose workspace commit isn't at the top");
        }
        if matches!(workspace.kind, WorkspaceKind::AdHoc) {
            bail!("Cannot unapply anything on a checked-out branch");
        }

        let mut branch = branch.to_owned();
        let mut branch_ref = try_find_validated_ref(repo, branch.as_ref())?;
        if branch
            .category()
            .is_some_and(|category| category == Category::RemoteBranch)
        {
            let Some((local_tracking_branch_name, _remote_name)) =
                repo.upstream_branch_and_remote_for_tracking_branch(branch.as_ref())?
            else {
                bail!("Couldn't find remote refspecs that would match {branch}");
            };
            branch = local_tracking_branch_name;
            branch_ref = try_find_validated_ref(repo, branch.as_ref())?;
        }

        if meta.workspace_opt(branch.as_ref())?.is_some() {
            bail!(
                "Refusing to unapply a reference that already is a workspace: '{}'",
                branch.shorten()
            );
        }

        let ws_ref_name = workspace
            .ref_name()
            .context("BUG: managed workspaces must be named to support unapply")?;
        let current_workspace_target_id = repo
            .try_find_reference(ws_ref_name)?
            .with_context(|| {
                format!(
                    "Couldn't find workspace reference '{}' to update it during unapply",
                    ws_ref_name.shorten()
                )
            })?
            .peel_to_id()?
            .detach();

        let mut ws_md = meta.workspace(ws_ref_name)?;
        let Some((stack_idx, branch_idx)) =
            ws_md.find_owner_indexes_by_name(branch.as_ref(), AppliedAndUnapplied)
        else {
            let _ = branch_ref;
            bail!(
                "Branch '{}' not found in any applied stack",
                branch.shorten()
            );
        };

        {
            let stack = ws_md
                .stacks
                .get_mut(stack_idx)
                .expect("BUG: stack index must still point to the owning stack");
            if !stack.workspacecommit_relation.is_in_workspace() {
                return Ok(Outcome {
                    workspace: Cow::Borrowed(workspace),
                    checked_out: None,
                    workspace_ref_created: false,
                    workspace_merge: None,
                    conflicting_stack_ids: Vec::new(),
                });
            }

            let next_visible_commit_id = stack
                .branches
                .get(branch_idx + 1)
                .map(|next_branch| resolve_commit_id(repo, workspace, next_branch))
                .transpose()?;
            let new_relation =
                next_visible_commit_id.map_or(WorkspaceCommitRelation::Outside, |commit_id| {
                    WorkspaceCommitRelation::MergeFrom {
                        commit_id: Some(commit_id),
                    }
                });
            if stack.workspacecommit_relation == new_relation {
                return Ok(Outcome {
                    workspace: Cow::Borrowed(workspace),
                    checked_out: None,
                    workspace_ref_created: false,
                    workspace_merge: None,
                    conflicting_stack_ids: Vec::new(),
                });
            }
            stack.workspacecommit_relation = new_relation;
        }

        let prev_head_id = workspace
            .graph
            .entrypoint_commit()
            .context("BUG: managed workspaces are expected to have an entrypoint commit")?
            .id;

        let (new_workspace_target_id, workspace_merge, conflicting_stack_ids) =
            compute_workspace_target_after_unapply(
                workspace,
                repo,
                &mut ws_md,
                workspace_merge_commit,
                on_workspace_conflict,
                ws_ref_name,
            )?;

        let switch_to_remaining_ref = maybe_switch_to_remaining_stack(
            workspace,
            repo,
            &ws_md,
            workspace_merge_commit,
            workspace_reference,
            RemainingStackSwitchTargets {
                prev_head_id,
                current_workspace_target_id,
                new_workspace_target_id,
            },
        )?;
        let delete_workspace_ref_after_switch = switch_to_remaining_ref.is_some()
            && matches!(
                workspace_reference,
                WorkspaceReference::DeleteAfterSwitchingToRemainingStack
            );
        let checked_out = switch_to_remaining_ref.clone();

        if workspace.is_entrypoint() && prev_head_id != new_workspace_target_id {
            but_core::worktree::safe_checkout(
                prev_head_id,
                new_workspace_target_id,
                repo,
                but_core::worktree::checkout::Options {
                    uncommitted_changes,
                    skip_head_update: true,
                },
            )?;
        }

        if let Some(ref remaining_ref) = switch_to_remaining_ref {
            if !delete_workspace_ref_after_switch
                && current_workspace_target_id != new_workspace_target_id
            {
                update_workspace_ref(repo, ws_ref_name, new_workspace_target_id)?;
            }
            if workspace.is_entrypoint() {
                set_head_to_named_reference(repo, remaining_ref.as_ref())?;
            }
            if delete_workspace_ref_after_switch {
                remove_workspace_reference_and_metadata(repo, meta, ws_ref_name)?;
            } else {
                meta.set_workspace(&ws_md)?;
            }
        } else {
            if current_workspace_target_id != new_workspace_target_id {
                update_workspace_ref(repo, ws_ref_name, new_workspace_target_id)?;
            }
            meta.set_workspace(&ws_md)?;
            if workspace.is_entrypoint() {
                set_head_to_workspace_ref(repo, ws_ref_name, new_workspace_target_id)?;
            }
        }

        let ws = but_graph::Graph::from_head(repo, meta, workspace.graph.options.clone())?
            .into_workspace()?;
        Ok(Outcome {
            workspace: Cow::Owned(ws),
            checked_out,
            workspace_ref_created: false,
            workspace_merge,
            conflicting_stack_ids,
        })
    }

    fn compute_workspace_target_after_unapply(
        workspace: &but_graph::projection::Workspace,
        repo: &gix::Repository,
        ws_md: &mut Workspace,
        workspace_merge_commit: WorkspaceMergeCommit,
        on_workspace_conflict: crate::branch::OnWorkspaceMergeConflict,
        ws_ref_name: &FullNameRef,
    ) -> anyhow::Result<(
        gix::ObjectId,
        Option<crate::commit::merge::Outcome>,
        Vec<StackId>,
    )> {
        let mut in_workspace_stack_count = ws_md
            .stacks
            .iter()
            .filter(|stack| stack.is_in_workspace())
            .count();
        if in_workspace_stack_count == 0 {
            let fallback = workspace
                .lower_bound
                .or_else(|| workspace.tip_commit().map(|commit| commit.id))
                .context("BUG: managed workspace without applied stacks should still have a visible base")?;
            return Ok((fallback, None, Vec::new()));
        }

        if in_workspace_stack_count == 1
            && matches!(
                workspace_merge_commit,
                WorkspaceMergeCommit::RemoveIfPossible
            )
        {
            let remaining_stack = ws_md
                .stacks
                .iter()
                .find(|stack| stack.is_in_workspace())
                .context("BUG: counted exactly one remaining applied stack")?;
            let target_id = visible_commit_id_for_stack(remaining_stack, repo, workspace)?;
            return Ok((target_id, None, Vec::new()));
        }

        let mut in_memory_repo = repo.clone().for_tree_diffing()?.with_object_memory();
        let mut merge_result = WorkspaceCommit::from_new_merge_with_metadata(
            ws_md.stacks.iter().filter(|stack| stack.is_in_workspace()),
            anon_stacks(&workspace.stacks),
            &workspace.graph,
            &in_memory_repo,
            None,
        )?;
        ensure_no_missing_stacks(&merge_result)?;

        if merge_result.has_conflicts() && on_workspace_conflict.should_abort() {
            let conflicting_stack_ids =
                correlate_conflicting_stack_ids(ws_md, &merge_result.conflicting_stacks);
            return Ok((
                repo.try_find_reference(ws_ref_name)?
                    .context("workspace ref must exist while aborting unapply")?
                    .peel_to_id()?
                    .detach(),
                Some(merge_result),
                conflicting_stack_ids,
            ));
        }

        let conflicting_stack_ids = correlate_conflicting_stack_ids_and_remove_from_workspace(
            ws_md,
            &merge_result.conflicting_stacks,
        );
        if !conflicting_stack_ids.is_empty() {
            in_workspace_stack_count = ws_md
                .stacks
                .iter()
                .filter(|stack| stack.is_in_workspace())
                .count();
            if in_workspace_stack_count == 0 {
                let fallback = workspace
                    .lower_bound
                    .or_else(|| workspace.tip_commit().map(|commit| commit.id))
                    .context("BUG: removing conflicting stacks left no visible workspace base")?;
                return Ok((fallback, Some(merge_result), conflicting_stack_ids));
            }
            if in_workspace_stack_count == 1
                && matches!(
                    workspace_merge_commit,
                    WorkspaceMergeCommit::RemoveIfPossible
                )
            {
                let remaining_stack = ws_md
                    .stacks
                    .iter()
                    .find(|stack| stack.is_in_workspace())
                    .context("BUG: counted exactly one remaining applied stack after conflicts")?;
                let target_id = visible_commit_id_for_stack(remaining_stack, repo, workspace)?;
                return Ok((target_id, Some(merge_result), conflicting_stack_ids));
            }
            merge_result = WorkspaceCommit::from_new_merge_with_metadata(
                ws_md.stacks.iter().filter(|stack| stack.is_in_workspace()),
                anon_stacks(&workspace.stacks),
                &workspace.graph,
                &in_memory_repo,
                None,
            )?;
            ensure_no_missing_stacks(&merge_result)?;
        }

        if let Some(storage) = in_memory_repo.objects.take_object_memory() {
            storage.persist(repo)?;
        }
        Ok((
            merge_result.workspace_commit_id,
            Some(merge_result),
            conflicting_stack_ids,
        ))
    }

    fn visible_commit_id_for_stack(
        stack: &but_core::ref_metadata::WorkspaceStack,
        repo: &gix::Repository,
        workspace: &but_graph::projection::Workspace,
    ) -> anyhow::Result<gix::ObjectId> {
        match stack.workspacecommit_relation {
            WorkspaceCommitRelation::Merged => {
                let top_branch = stack
                    .branches
                    .first()
                    .context("BUG: applied stacks must have at least one branch")?;
                resolve_commit_id(repo, workspace, top_branch)
            }
            WorkspaceCommitRelation::MergeFrom {
                commit_id: Some(commit_id),
            } => Ok(commit_id),
            WorkspaceCommitRelation::MergeFrom { commit_id: None } => workspace
                .lower_bound
                .or_else(|| workspace.tip_commit().map(|commit| commit.id))
                .context("BUG: stack without visible tree requires a workspace base"),
            WorkspaceCommitRelation::Outside => {
                bail!("BUG: visible_commit_id_for_stack() called for unapplied stack")
            }
        }
    }

    fn resolve_commit_id(
        repo: &gix::Repository,
        workspace: &but_graph::projection::Workspace,
        branch: &but_core::ref_metadata::WorkspaceStackBranch,
    ) -> anyhow::Result<gix::ObjectId> {
        let ref_name = branch.ref_name.as_ref();
        if let Some((_segment, commit)) = workspace.graph.segment_and_commit_by_ref_name(ref_name) {
            return Ok(commit.id);
        }
        if let Some(head_commit_id) = branch.head_commit_id.filter(|id| !id.is_null()) {
            return Ok(head_commit_id);
        }
        repo.try_find_reference(ref_name)?
            .with_context(|| {
                format!(
                    "Couldn't find branch '{}' to resolve its commit",
                    ref_name.shorten()
                )
            })?
            .peel_to_id()
            .map(|id| id.detach())
            .map_err(Into::into)
    }

    fn try_find_validated_ref<'repo>(
        repo: &'repo gix::Repository,
        branch: &FullNameRef,
    ) -> anyhow::Result<Option<gix::Reference<'repo>>> {
        let branch_ref = repo.try_find_reference(branch)?;
        if branch_ref.as_ref().is_some_and(|reference| {
            matches!(reference.target(), gix::refs::TargetRef::Symbolic(_))
        }) {
            bail!(
                "Refusing to unapply symbolic ref '{}' due to potential ambiguity",
                branch.shorten()
            );
        }
        Ok(branch_ref)
    }

    fn anon_stacks(
        stacks: &[but_graph::projection::Stack],
    ) -> impl Iterator<Item = (usize, Tip)> + '_ {
        stacks.iter().enumerate().filter_map(|(idx, stack)| {
            if stack.ref_name().is_none() {
                stack.tip_skip_empty().and_then(|commit_id| {
                    stack.segments.first().map(|segment| {
                        (
                            idx,
                            Tip {
                                name: None,
                                commit_id,
                                merge_commit_id: Some(commit_id),
                                segment_idx: segment.id,
                            },
                        )
                    })
                })
            } else {
                None
            }
        })
    }

    fn correlate_conflicting_stack_ids(
        ws: &Workspace,
        conflicts: &[crate::commit::merge::ConflictingStack],
    ) -> Vec<StackId> {
        conflicts
            .iter()
            .filter_map(|conflict| conflict.ref_name.as_ref())
            .filter_map(|ref_name| {
                ws.find_stack_with_branch(ref_name.as_ref(), AppliedAndUnapplied)
                    .map(|stack| stack.id)
            })
            .collect()
    }

    fn correlate_conflicting_stack_ids_and_remove_from_workspace(
        ws: &mut Workspace,
        conflicts: &[crate::commit::merge::ConflictingStack],
    ) -> Vec<StackId> {
        let conflicting_stack_ids = correlate_conflicting_stack_ids(ws, conflicts);
        for conflicting_id in &conflicting_stack_ids {
            let stack = ws
                .stacks
                .iter_mut()
                .find(|stack| stack.id == *conflicting_id)
                .expect("if the stack id was found before it must still exist");
            stack.workspacecommit_relation = WorkspaceCommitRelation::Outside;
        }
        conflicting_stack_ids
    }

    fn ensure_no_missing_stacks(merge: &crate::commit::merge::Outcome) -> anyhow::Result<()> {
        if merge.missing_stacks.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Somehow some of the remaining stacks weren't part of the graph: {:#?}",
                merge.missing_stacks
            ))
        }
    }

    fn update_workspace_ref(
        repo: &gix::Repository,
        ws_ref_name: &FullNameRef,
        new_target_id: gix::ObjectId,
    ) -> anyhow::Result<()> {
        repo.reference(
            ws_ref_name,
            new_target_id,
            PreviousValue::Any,
            "updated by GitButler during unapply-branch",
        )?;
        Ok(())
    }

    struct RemainingStackSwitchTargets {
        prev_head_id: gix::ObjectId,
        current_workspace_target_id: gix::ObjectId,
        new_workspace_target_id: gix::ObjectId,
    }

    fn maybe_switch_to_remaining_stack(
        workspace: &but_graph::projection::Workspace,
        repo: &gix::Repository,
        ws_md: &Workspace,
        workspace_merge_commit: WorkspaceMergeCommit,
        workspace_reference: WorkspaceReference,
        RemainingStackSwitchTargets {
            prev_head_id,
            current_workspace_target_id,
            new_workspace_target_id,
        }: RemainingStackSwitchTargets,
    ) -> anyhow::Result<Option<gix::refs::FullName>> {
        if !matches!(
            workspace_merge_commit,
            WorkspaceMergeCommit::RemoveIfPossible
        ) || matches!(workspace_reference, WorkspaceReference::KeepCheckedOut)
            || !workspace.is_entrypoint()
            || prev_head_id != current_workspace_target_id
        {
            return Ok(None);
        }

        let mut remaining_stacks = ws_md.stacks.iter().filter(|stack| stack.is_in_workspace());
        let Some(remaining_stack) = remaining_stacks.next() else {
            return Ok(None);
        };
        if remaining_stacks.next().is_some() {
            return Ok(None);
        }

        for branch in &remaining_stack.branches {
            if repo.try_find_reference(branch.ref_name.as_ref())?.is_none() {
                continue;
            }
            match resolve_commit_id(repo, workspace, branch) {
                Ok(commit_id) if commit_id == new_workspace_target_id => {
                    return Ok(Some(branch.ref_name.clone()));
                }
                Ok(_) | Err(_) => {}
            }
        }

        Ok(None)
    }

    fn remove_workspace_reference_and_metadata(
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        ws_ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        if let Some(reference) = repo.try_find_reference(ws_ref_name)? {
            let safe_delete = SafeDelete::new(repo)?;
            let outcome = safe_delete.delete_reference(&reference)?;
            if let Some(paths) = outcome.checked_out_in_worktree_dirs {
                bail!("Refusing to delete a branch that is checked out. Worktrees are: {paths:?}");
            }
        }
        let _ = meta.remove(ws_ref_name)?;
        Ok(())
    }

    fn set_head_to_named_reference(
        repo: &gix::Repository,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: "GitButler switch to remaining stack during unapply-branch".into(),
                },
                expected: PreviousValue::Any,
                new: Target::Symbolic(ref_name.to_owned()),
            },
            name: "HEAD".try_into().expect("well-formed root ref"),
            deref: false,
        })?;
        Ok(())
    }

    fn set_head_to_workspace_ref(
        repo: &gix::Repository,
        ws_ref_name: &FullNameRef,
        new_target_id: gix::ObjectId,
    ) -> anyhow::Result<()> {
        repo.edit_references(vec![
            RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "GitButler switch to workspace during unapply-branch".into(),
                    },
                    expected: PreviousValue::Any,
                    new: Target::Symbolic(ws_ref_name.to_owned()),
                },
                name: "HEAD".try_into().expect("well-formed root ref"),
                deref: false,
            },
            RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "updated by GitButler during unapply-branch".into(),
                    },
                    expected: PreviousValue::Any,
                    new: Target::Object(new_target_id),
                },
                name: ws_ref_name.to_owned(),
                deref: false,
            },
        ])?;
        Ok(())
    }
}
