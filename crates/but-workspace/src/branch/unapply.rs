use crate::branch::OnWorkspaceMergeConflict;
use but_core::ref_metadata::StackId;
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use std::borrow::Cow;

/// Returned by [unapply()](function::unapply()).
pub struct Outcome<'workspace> {
    /// The updated workspace, if owned, or the one that was passed in if borrowed, to show how the workspace looks after unapplying.
    ///
    /// If borrowed, the graph already didn't contain the desired branch and nothing had to be unapplied. Note that metadata changes
    /// might not be included in this case, as they aren't the source of truth.
    pub workspace: Cow<'workspace, but_graph::Workspace>,
    /// The unapply operation ended in checking out the last remaining stack in the workspace, whose tip name is listed here.
    /// This will only happen if the commit to check out is named.
    ///
    /// This can happen only if [WorkspaceDisposition] allows forgetting the workspace merge commit and switching away from
    /// the workspace reference.
    pub checked_out: Option<gix::refs::FullName>,
    /// If not `None`, an actual merge was attempted while rebuilding the workspace merge commit after removing the stack.
    /// Depending on [OnWorkspaceMergeConflict], this was persisted or only reported.
    pub workspace_merge: Option<crate::commit::merge::Outcome>,
    /// The ids of all stacks that conflicted while rebuilding the workspace merge commit.
    ///
    /// Tip ref names can be derived from these ids.
    pub conflicting_stack_ids: Vec<StackId>,
}

impl Outcome<'_> {
    /// Return `true` if a new graph traversal was performed, which always is a sign for an operation which changed the workspace.
    /// This is `false` if the branch to unapply was already absent from the current workspace.
    pub fn workspace_changed(&self) -> bool {
        matches!(self.workspace, Cow::Owned(_))
    }
}

impl std::fmt::Debug for Outcome<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Outcome {
            workspace: _,
            checked_out,
            workspace_merge: _,
            conflicting_stack_ids,
        } = self;
        let checked_out = checked_out.as_ref().map(|rn| rn.to_string());
        let mut f = f.debug_struct("Outcome");
        f.field("workspace_changed", &self.workspace_changed())
            .field("checked_out", &checked_out);
        if !conflicting_stack_ids.is_empty() {
            f.field("conflicting_stack_ids", conflicting_stack_ids);
        }
        f.finish()
    }
}

/// What to do with the workspace commit and workspace reference after unapplying a stack.
///
/// Unapplying can make the workspace merge commit unnecessary. That happens when the workspace commit would only connect to
/// a single remaining stack, or if none of the remaining stacks have their own commits, so all rest on the base and are thus virtual.
/// These variants describe the possibilities between these conditions:
///
/// - whether to keep or remove the workspace commit
/// - whether to keep or delete the workspace reference
///
/// and whether the worktree may switch away from the workspace reference, i.e. change `HEAD`.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkspaceDisposition {
    /// Keep the workspace merge commit and keep the workspace reference checked out, even if the merge commit is no longer
    /// required to represent the workspace.
    ///
    /// Use this when callers want a stable checked-out workspace ref and do not want unapply to collapse it to a branch,
    /// target, or workspace base commit. This is the conservative default.
    // TODO: this is for compatibility with old code and should not be the default or even an option.
    #[default]
    KeepWorkspaceMergeCommit,
    /// Remove the workspace merge commit if it is unnecessary, but keep the workspace reference and metadata.
    ///
    /// The workspace reference remains checked out, and set to point directly to the remaining
    /// stack tip, or workspace base commit of the workspace. If the workspace is purely virtual, i.e. it governs
    /// no reference that points to a non-base commit, then the reference may already be sufficient
    /// without a merge commit.
    // TODO: make this the default when this is the default in apply().
    RemoveWorkspaceMergeCommitKeepWorkspaceReference,
    /// Remove the workspace merge commit if it is unnecessary, keep the workspace reference and metadata, and allow checking out
    /// the remaining named reference directly.
    ///
    /// Direct checkout can happen only when there is an unambiguous reference to switch to, such as the last remaining named
    /// stack or the workspace base commit. If there is no such reference, unapply keeps the workspace reference checked out.
    RemoveWorkspaceMergeCommitAndSwitch,
    /// Remove the workspace merge commit if it is unnecessary, switch to the remaining named reference, and delete the managed
    /// workspace reference and its metadata.
    ///
    /// Direct checkout can happen only when there is an unambiguous reference to switch to, such as the last remaining named
    /// stack or the workspace base commit. If there is no such reference, unapply keeps the workspace reference checked out
    /// and keeps its metadata.
    RemoveWorkspaceMergeCommitAndDeleteWorkspaceReference,
}

/// Options for [branch::unapply()](function::unapply()).
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How to represent the workspace after the stack has been removed.
    pub workspace_disposition: WorkspaceDisposition,
    /// How the worktree checkout should behave when uncommitted changes are present in the worktree that it would
    /// want to modify to accommodate the new workspace commit, with the unapplied stack removed.
    pub uncommitted_changes: UncommitedWorktreeChanges,
    /// Decide how to deal with conflicts when updating the workspace merge commit after removing a stack.
    ///
    /// Note that it should be incredibly unlikely, but can we prove it's impossible?
    pub on_workspace_conflict: OnWorkspaceMergeConflict,
}

pub(crate) mod function {
    use super::{Options, Outcome, WorkspaceDisposition};
    use anyhow::{Context as _, bail};
    use std::borrow::Cow;

    use but_core::{
        ObjectStorageExt as _, RefMetadata, RepositoryExt as _,
        ref_metadata::{StackKind, WorkspaceCommitRelation::Outside},
    };
    use but_graph::init::Overlay;
    use gix::{
        reference::Category,
        refs::{
            FullName, FullNameRef, Target,
            transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
        },
    };

    use crate::WorkspaceCommit;
    use crate::{
        branch::{
            anon_stacks, correlate_conflicting_stack_ids,
            correlate_conflicting_stack_ids_and_remove_from_workspace, ensure_no_missing_stacks,
            try_find_validated_ref,
        },
        ref_info::WorkspaceExt,
    };

    /// Remove `branch` from `workspace`, updating `repo` and `meta` so the resulting workspace is the inverse of applying that branch.
    ///
    /// `branch` must name a branch to remove from the workspace. If it is already absent from
    /// `workspace`, this is a no-op and the returned [`Outcome`] borrows `workspace`. Symbolic names such as `HEAD` are
    /// rejected to avoid ambiguity.
    ///
    /// `workspace` is the current projected workspace and is used as the source of truth for which stacks are applied,
    /// which stack should be removed, and whether the workspace is in a state that can be modified. `repo` supplies the
    /// Git object database, references, and the *worktree* (which is assumed to belong to the workspace reference that governs `workspace`).
    /// `meta` is updated to remove the branch from the workspace metadata, persist the resulting stack/workspace metadata,
    /// and optionally delete workspace metadata when `opts` requests deleting the workspace reference.
    ///
    /// `opts`is best looked up via [`Options`].
    #[tracing::instrument(skip(workspace, repo, meta), err(Debug))]
    pub fn unapply<'ws>(
        branch: &FullNameRef,
        workspace: &'ws but_graph::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            workspace_disposition,
            uncommitted_changes,
            on_workspace_conflict,
        }: Options,
    ) -> anyhow::Result<Outcome<'ws>> {
        let ws = workspace;
        let branch_ref = try_find_validated_ref(repo, branch, "unapply")?;

        if ws.has_workspace_commit_in_ancestry(repo) {
            bail!("Refusing to work on workspace whose workspace commit isn't at the top");
        }

        let Some(workspace_ref_name) = ws.ref_name().map(ToOwned::to_owned) else {
            if !ws.refname_is_segment(branch) {
                if branch_ref.is_none() {
                    bail!(
                        "Cannot unapply non-existing branch '{branch}'",
                        branch = branch.shorten()
                    );
                }
                return Ok(Outcome {
                    workspace: Cow::Borrowed(ws),
                    checked_out: None,
                    workspace_merge: None,
                    conflicting_stack_ids: Vec::new(),
                });
            }
            bail!("Cannot unapply a branch from an ad-hoc workspace");
        };
        let mut ws_md = meta.workspace(workspace_ref_name.as_ref())?;
        let removed = remove_branch_from_workspace_metadata(&mut ws_md, branch);

        if !removed && !ws.refname_is_segment(branch) {
            if branch_ref.is_none() {
                bail!(
                    "Cannot unapply non-existing branch '{branch}'",
                    branch = branch.shorten()
                );
            }
            return Ok(Outcome {
                workspace: Cow::Borrowed(ws),
                checked_out: None,
                workspace_merge: None,
                conflicting_stack_ids: Vec::new(),
            });
        }

        if !removed {
            return Ok(Outcome {
                workspace: Cow::Borrowed(ws),
                checked_out: None,
                workspace_merge: None,
                conflicting_stack_ids: Vec::new(),
            });
        }

        let checked_out = checkout_target_after_unapply(ws, repo, &ws_md, workspace_disposition)?;
        let delete_workspace_ref = checked_out.is_some()
            && matches!(
                workspace_disposition,
                WorkspaceDisposition::RemoveWorkspaceMergeCommitAndDeleteWorkspaceReference
            );

        if delete_workspace_ref {
            let prev_head_id = ws
                .graph
                .entrypoint()?
                .commit()
                .context("BUG: should not be unborn by now")?
                .id;
            let new_head_id = checked_out
                .as_ref()
                .expect("delete requires a checkout target")
                .as_ref();
            let new_head_id = repo.find_reference(new_head_id)?.peel_to_id()?.detach();
            but_core::worktree::safe_checkout(
                prev_head_id,
                new_head_id,
                repo,
                but_core::worktree::checkout::Options {
                    uncommitted_changes,
                    skip_head_update: true,
                    ..Default::default()
                },
            )?;
            switch_head_and_delete_workspace_ref(
                repo,
                checked_out
                    .as_ref()
                    .expect("delete requires a checkout target")
                    .as_ref(),
                workspace_ref_name.as_ref(),
            )?;
            meta.remove(workspace_ref_name.as_ref())?;
        } else {
            let (graph, workspace_merge, conflicting_stack_ids, persist_metadata) =
                update_workspace_ref_after_unapply(
                    ws,
                    repo,
                    meta,
                    workspace_ref_name.as_ref(),
                    &mut ws_md,
                    workspace_disposition,
                    uncommitted_changes,
                    on_workspace_conflict,
                )?;
            if persist_metadata {
                meta.set_workspace(&ws_md)?;
            }
            return Ok(Outcome {
                workspace: Cow::Owned(graph.into_workspace()?),
                checked_out,
                workspace_merge,
                conflicting_stack_ids,
            });
        }

        let overlay = match checked_out.as_ref() {
            Some(checked_out) => {
                let checked_out_id = repo.find_reference(checked_out)?.peel_to_id()?.detach();
                Overlay::default().with_entrypoint(checked_out_id, Some(checked_out.clone()))
            }
            None => Overlay::default(),
        };
        let graph = ws.graph.redo_traversal_with_overlay(repo, meta, overlay)?;

        Ok(Outcome {
            workspace: Cow::Owned(graph.into_workspace()?),
            checked_out,
            workspace_merge: None,
            conflicting_stack_ids: Vec::new(),
        })
    }

    /// Update the managed workspace reference after metadata has removed the branch.
    ///
    /// If the remaining applied stacks still need a workspace merge commit, this rebuilds it in
    /// memory first, then safely checks out the resulting tree and moves the workspace ref. If the
    /// merge conflicts and `on_workspace_conflict` asks to abort, refs and metadata are left
    /// untouched; the returned `bool` tells the caller whether the adjusted metadata should be
    /// persisted.
    #[expect(clippy::too_many_arguments)]
    fn update_workspace_ref_after_unapply(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        workspace_ref_name: &FullNameRef,
        ws_md: &mut but_core::ref_metadata::Workspace,
        disposition: WorkspaceDisposition,
        uncommitted_changes: but_core::worktree::checkout::UncommitedWorktreeChanges,
        on_workspace_conflict: crate::branch::OnWorkspaceMergeConflict,
    ) -> anyhow::Result<(
        but_graph::Graph,
        Option<crate::commit::merge::Outcome>,
        Vec<but_core::ref_metadata::StackId>,
        bool,
    )> {
        let prev_head_id = ws
            .graph
            .entrypoint()?
            .commit()
            .context("BUG: should not be unrorn by now")?
            .id;

        let applied_stack_count = ws_md.stacks(StackKind::Applied).count();
        let keep_workspace_commit = match disposition {
            WorkspaceDisposition::KeepWorkspaceMergeCommit => ws.kind.has_managed_commit(),
            WorkspaceDisposition::RemoveWorkspaceMergeCommitKeepWorkspaceReference
            | WorkspaceDisposition::RemoveWorkspaceMergeCommitAndSwitch
            | WorkspaceDisposition::RemoveWorkspaceMergeCommitAndDeleteWorkspaceReference => {
                applied_stack_count > 1
            }
        };

        if keep_workspace_commit {
            let mut in_memory_repo = repo.clone().for_tree_diffing()?.with_object_memory();
            let mut merge_result = WorkspaceCommit::from_new_merge_with_metadata(
                ws_md.stacks.iter().filter(|s| s.is_in_workspace()),
                anon_stacks(&ws.stacks),
                &ws.graph,
                &in_memory_repo,
                None,
            )?;
            ensure_no_missing_stacks(&merge_result)?;

            if merge_result.has_conflicts() && on_workspace_conflict.should_abort() {
                let conflicting_stack_ids =
                    correlate_conflicting_stack_ids(ws_md, &merge_result.conflicting_stacks);
                let graph = ws.graph.redo_traversal_with_overlay(
                    &in_memory_repo,
                    meta,
                    Overlay::default()
                        .with_entrypoint(prev_head_id, Some(workspace_ref_name.to_owned()))
                        .with_workspace_metadata_override(Some((
                            workspace_ref_name.to_owned(),
                            ws_md.clone(),
                        ))),
                )?;
                return Ok((graph, Some(merge_result), conflicting_stack_ids, false));
            }

            let conflicting_stack_ids = correlate_conflicting_stack_ids_and_remove_from_workspace(
                ws_md,
                &merge_result.conflicting_stacks,
            );
            if merge_result.has_conflicts() {
                merge_result = WorkspaceCommit::from_new_merge_with_metadata(
                    ws_md.stacks.iter().filter(|s| s.is_in_workspace()),
                    anon_stacks(&ws.stacks),
                    &ws.graph,
                    &in_memory_repo,
                    None,
                )?;
                ensure_no_missing_stacks(&merge_result)?;
            }
            let new_head_id = merge_result.workspace_commit_id;
            let graph = ws.graph.redo_traversal_with_overlay(
                &in_memory_repo,
                meta,
                Overlay::default()
                    .with_entrypoint(new_head_id, Some(workspace_ref_name.to_owned()))
                    .with_workspace_metadata_override(Some((
                        workspace_ref_name.to_owned(),
                        ws_md.clone(),
                    ))),
            )?;

            if let Some(storage) = in_memory_repo.objects.take_object_memory() {
                storage.persist(repo)?;
                drop(in_memory_repo);
            }
            checkout_and_update_workspace_ref(
                repo,
                prev_head_id,
                new_head_id,
                workspace_ref_name,
                uncommitted_changes,
            )?;
            return Ok((graph, Some(merge_result), conflicting_stack_ids, true));
        }

        let new_head_id = commit_to_point_workspace_ref_to_after_unapply(ws, repo, ws_md)?;
        let overlay = Overlay::default()
            .with_entrypoint(new_head_id, Some(workspace_ref_name.to_owned()))
            .with_workspace_metadata_override(Some((workspace_ref_name.to_owned(), ws_md.clone())));
        let graph = ws.graph.redo_traversal_with_overlay(repo, meta, overlay)?;
        checkout_and_update_workspace_ref(
            repo,
            prev_head_id,
            new_head_id,
            workspace_ref_name,
            uncommitted_changes,
        )?;
        Ok((graph, None, Vec::new(), true))
    }

    /// Return the commit the workspace ref should point to when no workspace merge commit remains.
    ///
    /// A single remaining applied stack is preferred. Otherwise, an empty workspace falls back to
    /// the remembered target commit, the resolved target, or the workspace lower bound.
    fn commit_to_point_workspace_ref_to_after_unapply(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        ws_md: &but_core::ref_metadata::Workspace,
    ) -> anyhow::Result<gix::ObjectId> {
        if let Some(stack_name) = ws_md
            .stacks(StackKind::Applied)
            .find_map(|stack| stack.ref_name())
        {
            return Ok(repo.find_reference(stack_name)?.peel_to_id()?.detach());
        }
        ws_md
            .target_commit_id
            .or_else(|| ws.resolved_target_commit_id())
            .or(ws.lower_bound)
            .context("Cannot determine commit for empty workspace after unapply")
    }

    /// Safely update the worktree and move the managed workspace ref to `new_head_id`.
    ///
    /// The checkout runs before the ref update so uncommitted-change conflicts abort without
    /// changing repository refs. `HEAD` is expected to remain symbolically attached to the workspace
    /// ref, so the checkout skips its own head update and this function updates only the ref target.
    fn checkout_and_update_workspace_ref(
        repo: &gix::Repository,
        prev_head_id: gix::ObjectId,
        new_head_id: gix::ObjectId,
        workspace_ref_name: &FullNameRef,
        uncommitted_changes: but_core::worktree::checkout::UncommitedWorktreeChanges,
    ) -> anyhow::Result<()> {
        but_core::worktree::safe_checkout(
            prev_head_id,
            new_head_id,
            repo,
            but_core::worktree::checkout::Options {
                uncommitted_changes,
                skip_head_update: true,
                ..Default::default()
            },
        )?;
        repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: "GitButler update workspace during unapply-branch".into(),
                },
                expected: PreviousValue::Any,
                new: Target::Object(new_head_id),
            },
            name: workspace_ref_name.to_owned(),
            deref: false,
        })?;
        Ok(())
    }

    /// Remove `branch` from applied workspace metadata.
    ///
    /// If `branch` is the only segment in its stack, the whole stack metadata entry is removed so
    /// virtual apply/unapply roundtrips return to the original empty-workspace shape. If `branch`
    /// is the tip of a multi-segment stack, the remaining stack is marked outside the workspace
    /// because its visible tip was removed.
    #[expect(clippy::indexing_slicing)]
    fn remove_branch_from_workspace_metadata(
        ws_md: &mut but_core::ref_metadata::Workspace,
        branch: &FullNameRef,
    ) -> bool {
        let Some((stack_idx, segment_idx)) =
            ws_md.find_owner_indexes_by_name(branch, StackKind::Applied)
        else {
            return false;
        };

        let stack = &mut ws_md.stacks[stack_idx];
        stack.branches.remove(segment_idx);

        if stack.branches.is_empty() {
            ws_md.stacks.remove(stack_idx);
        } else if segment_idx == 0 {
            stack.workspacecommit_relation = Outside;
        }
        true
    }

    /// Determine whether unapply should switch `HEAD` away from the workspace reference.
    ///
    /// Switching is considered only for dispositions that explicitly allow it, and only once no
    /// applied stacks remain in `ws_md`. The returned ref is the branch that should become the
    /// checkout target, or `None` if the workspace reference must remain checked out.
    fn checkout_target_after_unapply(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        ws_md: &but_core::ref_metadata::Workspace,
        disposition: WorkspaceDisposition,
    ) -> anyhow::Result<Option<FullName>> {
        if !matches!(
            disposition,
            WorkspaceDisposition::RemoveWorkspaceMergeCommitAndSwitch
                | WorkspaceDisposition::RemoveWorkspaceMergeCommitAndDeleteWorkspaceReference
        ) {
            return Ok(None);
        }

        if ws_md.stacks(StackKind::Applied).next().is_some() {
            return Ok(None);
        }

        local_checkout_ref_for_workspace_target(ws, repo, ws_md)
    }

    /// Return the local branch to check out for the workspace target.
    ///
    /// `ws` is preferred because it reflects the current graph projection, while `ws_md` is used as
    /// a fallback for target metadata. Remote-tracking targets are converted to their local tracking
    /// branch when the repository can resolve one, because checking out the remote-tracking ref
    /// directly would detach `HEAD`.
    fn local_checkout_ref_for_workspace_target(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        ws_md: &but_core::ref_metadata::Workspace,
    ) -> anyhow::Result<Option<FullName>> {
        let target_ref = ws
            .target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref())
            .or_else(|| ws_md.target_ref.as_ref().map(|rn| rn.as_ref()));
        let Some(target_ref) = target_ref else {
            return Ok(None);
        };

        if target_ref
            .category()
            .is_some_and(|category| category == Category::RemoteBranch)
            && let Some((local_tracking_branch, _remote_name)) =
                repo.upstream_branch_and_remote_for_tracking_branch(target_ref)?
        {
            return Ok(Some(local_tracking_branch));
        }

        Ok(Some(target_ref.to_owned()))
    }

    /// Delete `workspace_ref_name` and point `HEAD` symbolically at `target_ref`.
    ///
    /// The workspace ref is deleted first so the symbolic `HEAD` update cannot be dereferenced
    /// through the workspace branch. This helper intentionally leaves worktree contents alone; the
    /// current minimal unapply path only uses it when both refs point at the same commit.
    fn switch_head_and_delete_workspace_ref(
        repo: &gix::Repository,
        target_ref: &FullNameRef,
        workspace_ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        repo.find_reference(workspace_ref_name)?
            .delete()
            .context("expected workspace ref to exist")?;
        repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: "GitButler switch away from workspace during unapply-branch".into(),
                },
                expected: PreviousValue::Any,
                new: Target::Symbolic(target_ref.to_owned()),
            },
            name: "HEAD".try_into().expect("well-formed root ref"),
            deref: false,
        })?;
        Ok(())
    }
}
