use but_core::worktree::checkout::UncommitedWorktreeChanges;
use std::borrow::Cow;

/// Returned by [unapply()](function::unapply()).
pub struct Outcome<'workspace> {
    /// The updated workspace, if owned, or the one that was passed in if borrowed, to show how the workspace looks after unapplying.
    ///
    /// If borrowed, the graph already didn't contain the desired branch and nothing had to be unapplied. Note that metadata changes
    /// might not be included in this case, as they aren't the source of truth.
    pub workspace: Cow<'workspace, but_graph::Workspace>,
    /// The unapply operation ended by checking out this ref.
    ///
    /// This is set when the operation switches back to the enclosing workspace ref after unapplying the checked-out stack,
    /// or when [WorkspaceDisposition] allows deleting the workspace reference and switching away from it.
    pub checked_out: Option<gix::refs::FullName>,
    /// If not `None`, a non-conflicting workspace merge was materialized while rebuilding the
    /// workspace merge commit after removing the stack.
    ///
    /// Unapply does not return conflicted merge outcomes. If rebuilding the workspace merge commit
    /// conflicts, `unapply()` fails before refs, metadata, index, or worktree are updated.
    pub workspace_merge: Option<crate::commit::merge::Outcome>,
}

impl<'workspace> Outcome<'workspace> {
    fn new(ws: Cow<'workspace, but_graph::Workspace>) -> Self {
        Outcome {
            workspace: ws,
            checked_out: None,
            workspace_merge: None,
        }
    }
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
        } = self;
        let checked_out = checked_out.as_ref().map(|rn| rn.to_string());
        let mut f = f.debug_struct("Outcome");
        f.field("workspace_changed", &self.workspace_changed())
            .field("checked_out", &checked_out);
        f.finish()
    }
}

/// How to represent the workspace after unapplying a stack.
///
/// Unapplying can make the workspace merge commit unnecessary. That happens when the workspace
/// commit would only connect to a single remaining stack, or if none of the remaining stacks have
/// their own commits, so all rest on the base and are thus virtual.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkspaceDisposition {
    /// Keep the workspace merge commit and keep the workspace reference checked out, even if the merge commit is no longer
    /// required to represent the workspace.
    ///
    /// Use this when callers want a stable checked-out workspace ref and do not want unapply to collapse it to a branch,
    /// target, or workspace base commit. This is the conservative default.
    // TODO: this is for compatibility with old code and should not be the default or even an option.
    #[default]
    KeepWorkspaceCommit,
    /// Remove the workspace merge commit if it is unnecessary, but keep the workspace reference checked out.
    /// Useful if you want to allow empty workspaces, i.e. prefer to stay in a workspace after it was created.
    ///
    /// The workspace reference remains checked out, and set to point directly to the remaining
    /// stack tip, or workspace base commit of the workspace. If the workspace is purely virtual, i.e. it governs
    /// no reference that points to a non-base commit, then the reference may already be sufficient
    /// without a merge commit.
    // TODO: make this the default when this is the default in apply().
    //       WARNING: ANY MUTATION now has to be able to re-merge the workspace commit if they turn a virtual stack
    //       into a non-virtual one or vice-versa.
    KeepWorkspaceReference,
    /// Remove the workspace merge commit if it is unnecessary, switch to a non-workspace ref , and delete the
    /// managed workspace reference and its metadata.
    /// Use this if the workspace should be dissolved as soon as it serves no purpose anymore.
    ///
    /// Direct checkout can happen when the future workspace has exactly one named tip, or when it has no tips and the
    /// workspace target has a local tracking branch to fall back to. If there is no such reference, unapply keeps the
    /// workspace reference checked out and keeps its metadata.
    PreventUnnecessaryWorkspaceReferences,
    /// Like [WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences], but always keep the workspace
    /// merge commit whenever the workspace reference itself remains.
    ///
    /// Use this as a compatibility mode for mutations that do not yet deal gracefully with a
    /// workspace reference pointing directly to a stack tip, target, or workspace base commit.
    /// If unapply can remove the whole workspace reference and switch `HEAD` away, it still does so.
    PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
}

impl WorkspaceDisposition {
    fn may_switch_away_from_workspace(self) -> bool {
        matches!(
            self,
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences
                | WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit
        )
    }

    fn may_delete_workspace_reference(self) -> bool {
        matches!(
            self,
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences
                | WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit
        )
    }
}

/// Options for [branch::unapply()](function::unapply()).
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How to represent the workspace after the stack has been removed.
    pub workspace_disposition: WorkspaceDisposition,
    /// How the worktree checkout should behave when uncommitted changes are present in the worktree that it would
    /// want to modify to accommodate the new workspace commit, with the unapplied stack removed.
    pub uncommitted_changes: UncommitedWorktreeChanges,
}

pub(crate) mod function {
    use super::{Options, Outcome, WorkspaceDisposition};
    use anyhow::{Context as _, bail, ensure};
    use std::borrow::Cow;

    use but_core::{
        ObjectStorageExt as _, RefMetadata, RepositoryExt as _,
        ref_metadata::{ProjectedWorkspaceStack, StackId},
    };
    use but_graph::init::Overlay;
    use gix::{
        prelude::ObjectIdExt,
        refs::{
            FullName, FullNameRef, Target,
            transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
        },
    };

    use crate::{WorkspaceCommit, branch::try_find_validated_ref};
    use crate::{branch::anon_stacks, ref_info::WorkspaceExt};

    /// Remove `branch` from `workspace`, updating `repo` and `meta` so the resulting workspace is the inverse of applying that branch.
    ///
    /// Think of it as "remove `branch` from the workspace", somehow, and metadata may be a way to do. This also means
    /// this function can be applied indiscriminately, and in the worst case, it will do nothing.
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
    /// `opts` is best looked up via [`Options`].
    ///
    /// # Algorithm
    ///
    /// - Validate that `branch` is a real, non-symbolic ref and reject stale workspaces with a
    ///   workspace commit buried in history. Workspaces without a workspace commit are allowed.
    /// - Early-exit for missing branches, branches outside the workspace, ad-hoc workspaces, and
    ///   metadata-only no-ops.
    /// - Remove the branch from workspace metadata.
    ///      - This removes non-tip virtual segments or entire stacks if the branch to unapply is the tip.
    /// - Reproject immediately when metadata did not mention the branch, but branch metadata may
    ///   still disambiguate it in the workspace. This validates whether removing branch metadata
    ///   was enough and avoids touching refs or the worktree for a metadata-only unapply.
    /// - Reproject with the updated workspace metadata before touching refs. This gives merge and
    ///   collapse code the projected future stacks, including anonymous stacks that metadata alone
    ///   cannot describe.
    /// - Update the managed workspace ref, rebuilding or collapsing its merge commit, and persist
    ///   the workspace metadata.
    /// - Reproject from the updated workspace ref and persisted metadata. This is the canonical
    ///   post-unapply workspace shape and is used to verify that the branch is gone.
    /// - If unapplying hid the *checked-out* stack behind the workspace ref, switch `HEAD` back to
    ///   the workspace ref and reproject again from that entrypoint so the returned workspace
    ///   matches the actual checkout.
    /// - If the disposition allows deleting an unnecessary workspace ref, decide from that final
    ///   projection whether it has one stack, no stacks, or multiple stacks. If deleting it is
    ///   possible, check out the selected destination, delete the workspace ref plus metadata, and
    ///   reproject one last time from the new checkout target.
    /// - If the branch to unapply is the managed workspace ref itself, and the disposition allows switching,
    ///   the workspace ref is replaced by its target's local branch, or by the named stack with the lowest
    ///   generation (i.e. the topologically 'newest') if there is no target.
    #[tracing::instrument(skip(workspace, repo, meta), err(Debug))]
    pub fn unapply<'ws>(
        branch: &FullNameRef,
        workspace: &'ws but_graph::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            workspace_disposition,
            uncommitted_changes,
        }: Options,
    ) -> anyhow::Result<Outcome<'ws>> {
        let ws = workspace;
        let branch_ref = try_find_validated_ref(repo, branch, "unapply")?;

        if ws.has_workspace_commit_in_ancestry(repo) {
            bail!("Refusing to work on workspace whose workspace commit isn't at the top");
        }

        let workspace_ref_name = ws.ref_name().map(ToOwned::to_owned);
        if matches!(ws.kind, but_graph::workspace::WorkspaceKind::AdHoc)
            && branch_ref
                .as_ref()
                .zip(workspace_ref_name.as_ref())
                .is_some_and(|(to_unapply, workspace_ref_name)| {
                    to_unapply.name() == workspace_ref_name.as_ref()
                })
        {
            bail!(
                "Cannot unapply branch '{branch}' from an ad-hoc workspace because the workspace cannot be empty",
                branch = branch.shorten()
            );
        }

        if let Some(workspace_ref_name) = workspace_ref_name.as_ref()
            && workspace_ref_name.as_ref() == branch
        {
            return unapply_workspace_reference(
                ws,
                repo,
                meta,
                workspace_ref_name.as_ref(),
                workspace_disposition,
                uncommitted_changes,
            );
        }

        let branch_in_ws = ws.find_segment_and_stack_by_refname(branch);
        if branch_in_ws.is_none() {
            if branch_ref.is_none() {
                bail!(
                    "Cannot unapply non-existing branch '{branch}'",
                    branch = branch.shorten()
                );
            }
            // The branch exists in Git, but does not in the workspace: Nothing to do.
            return Ok(Outcome::new(Cow::Borrowed(ws)));
        }
        let workspace_tip_was_entrypoint = ws.is_entrypoint();

        let Some(workspace_ref_name) = workspace_ref_name else {
            // This is an ad-hoc workspace by merit of being unnamed.
            bail!("Cannot unapply a branch from an ad-hoc detached workspace");
        };
        let mut ws_md = meta.workspace(workspace_ref_name.as_ref())?;
        if ws.kind.has_managed_ref() || ws.has_metadata() {
            ws_md.reconcile_projected_stacks(
                ws.stacks.iter().map(|stack| ProjectedWorkspaceStack {
                    id: stack.id,
                    branches: stack
                        .segments
                        .iter()
                        .filter_map(|segment| segment.ref_name().map(ToOwned::to_owned))
                        .collect(),
                }),
                |_| StackId::generate(),
            )?;
        }
        let branch_removed_from_ws_meta = ws_md.unapply_branch(branch);
        if !branch_removed_from_ws_meta {
            // The branch wasn't in workspace metadata, yet it was present, so also delete its branch metadata
            // as it could be used to disambiguate the segment.
            // TODO: this will actually be observable even if it doens't work, unless it's run in a transaction, which right now it's not!
            //       Should be able to redo the traversal with an overlay that hides branch metadata, but I'd say it's not important enough.
            meta.remove(branch)?;
            let graph = ws
                .graph
                .redo_traversal_with_overlay(repo, meta, Overlay::default())?;
            let workspace = graph.into_workspace()?;
            if workspace.refname_is_segment(branch) {
                bail!(
                    "Cannot unapply branch '{branch}' from an ad-hoc workspace because non-tip branches can only disappear if their now removed metadata disambiguated them",
                    branch = branch.shorten()
                );
            }
            return Ok(Outcome::new(Cow::Owned(workspace)));
        }

        // Everything past this point is stricly in non-dry-run mode and we may totally end up in intermediate states
        // if something fails.
        // Redo the traversal with the changed workspace metadata so code below can rely on the reconciled version.
        let ws = ws
            .graph
            .redo_traversal_with_overlay(
                repo,
                meta,
                Overlay::default().with_workspace_metadata_override(Some((
                    workspace_ref_name.to_owned(),
                    ws_md.clone(),
                ))),
            )?
            .into_workspace()?;
        // Normal unapply first:
        // - re-merge or collapse the workspace commit
        // - point workspace to it
        // - update metadata and workspace
        let WorkspaceRefUpdateAfterUnapply {
            entrypoint_id,
            workspace_merge,
        } = update_workspace_ref_after_unapply(
            &ws,
            repo,
            workspace_ref_name.as_ref(),
            &ws_md,
            workspace_disposition,
            uncommitted_changes,
        )?;
        meta.set_workspace(&ws_md)?;
        // Update the workspace *only* after a successful workspace commit merge.
        let overlay = Overlay::default()
            .with_workspace_metadata_override(Some((workspace_ref_name.to_owned(), ws_md.clone())));
        let mut ws = ws
            .graph
            .redo_traversal_with_overlay(repo, meta, overlay)?
            .into_workspace()?;
        let checked_out = if !workspace_tip_was_entrypoint && ws.is_entrypoint() {
            // The workspace tip never was the entrypoint, meaning something inside
            // was the entrypoint, and now it's not visible anymore as that stack was unapplied.
            // Now we checkout the enclosing workspace instead.
            switch_head_to_workspace_ref(repo, workspace_ref_name.as_ref(), entrypoint_id)?;
            let overlay = Overlay::default()
                .with_entrypoint(entrypoint_id, Some(workspace_ref_name.to_owned()));
            ws = ws
                .graph
                .redo_traversal_with_overlay(repo, meta, overlay)?
                .into_workspace()?;
            Some(workspace_ref_name.to_owned())
        } else {
            None
        };
        if ws.refname_is_segment(branch) {
            bail!(
                "BUG: branch '{}' is still present in rebuilt workspace after unapply",
                branch.shorten()
            );
        }
        match ref_to_checkout_after_workspace_deletion(&ws, workspace_disposition)? {
            Some(ref_to_switch_to) => {
                // The rebuilt workspace can be discarded entirely, switching to another branch.
                safe_checkout_ref_to_checkout(
                    &ws,
                    repo,
                    &ref_to_switch_to,
                    but_core::worktree::checkout::Options {
                        uncommitted_changes,
                        // We will be setting the HEAD ourselves.
                        skip_head_update: true,
                        ..Default::default()
                    },
                )?;
                switch_head_and_delete_workspace_ref(
                    repo,
                    ref_to_switch_to.ref_name.as_ref(),
                    workspace_ref_name.as_ref(),
                    ws.tip_commit()
                        .context("BUG: unborn should be impossible here")?
                        .id,
                )?;
                // Keep the workspace metadata or we lose the target branch.
                // Currently that's a problem, so deal with it later.

                let overlay = Overlay::default().with_entrypoint(
                    ref_to_switch_to.commit_id,
                    Some(ref_to_switch_to.ref_name.clone()),
                );
                let ws = ws
                    .graph
                    .redo_traversal_with_overlay(repo, meta, overlay)?
                    .into_workspace()?;

                Ok(Outcome {
                    workspace: Cow::Owned(ws),
                    checked_out: Some(ref_to_switch_to.ref_name),
                    workspace_merge: None,
                })
            }
            None => Ok(Outcome {
                workspace: Cow::Owned(ws),
                checked_out,
                workspace_merge,
            }),
        }
    }

    /// Point `HEAD` back to the managed workspace reference after unapplying the
    /// branch that was previously checked out directly.
    /// It is assumed that the worktree and index already match what `HEAD` will
    /// point to next.
    ///
    /// `repo` is the repository whose `HEAD` will become symbolic again.
    ///
    /// `workspace_ref_name` is the managed workspace reference to attach `HEAD` to.
    /// It must already point to the commit checked out into the index and worktree.
    ///
    /// `expected_workspace_ref_id` is that checked-out commit. The helper verifies
    /// the workspace ref points to this id before changing `HEAD`, so the symbolic
    /// switch cannot silently attach `HEAD` to a different commit than the one the
    /// index/worktree were updated to.
    fn switch_head_to_workspace_ref(
        repo: &gix::Repository,
        workspace_ref_name: &FullNameRef,
        expected_workspace_ref_id: gix::ObjectId,
    ) -> anyhow::Result<()> {
        let actual_workspace_ref_id = repo
            .find_reference(workspace_ref_name)?
            .peel_to_id()?
            .detach();
        ensure!(
            actual_workspace_ref_id == expected_workspace_ref_id,
            "BUG: workspace ref '{}' points to {actual_workspace_ref_id}, expected {expected_workspace_ref_id}",
            workspace_ref_name.shorten()
        );
        repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: "GitButler switch to workspace during unapply-branch".into(),
                },
                expected: PreviousValue::Any,
                new: Target::Symbolic(workspace_ref_name.to_owned()),
            },
            name: "HEAD".try_into().expect("well-formed root ref"),
            deref: false,
        })?;
        Ok(())
    }

    /// Update the managed workspace reference after metadata has removed the branch.
    ///
    /// If the remaining applied stacks still need a workspace merge commit, this rebuilds it in
    /// memory first, then safely checks out the resulting tree and moves the workspace ref.
    /// Workspace merge conflicts are errors and leave refs, metadata, index, and worktree
    /// untouched.
    ///
    /// `ws` is the workspace projection after the branch was removed from workspace metadata.
    /// `ws_md` is the metadata that produced that projection. `repo` is the repository whose
    /// workspace ref and worktree may be updated. `workspace_ref_name` is the managed workspace
    /// ref to move to the new workspace commit. `disposition` controls whether an unnecessary
    /// workspace merge commit is kept or not. `uncommitted_changes` controls checkout conflict
    /// handling.
    fn update_workspace_ref_after_unapply(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        workspace_ref_name: &FullNameRef,
        ws_md: &but_core::ref_metadata::Workspace,
        disposition: WorkspaceDisposition,
        uncommitted_changes: but_core::worktree::checkout::UncommitedWorktreeChanges,
    ) -> anyhow::Result<WorkspaceRefUpdateAfterUnapply> {
        let prev_head_id = ws
            .graph
            .entrypoint()?
            .commit()
            .context("BUG: unborn was skipped, why no entrypoint")?
            .id;

        let future_workspace_tips = future_workspace_tips(ws_md, ws)?;
        let remaining_tip_count = future_workspace_tips.len();
        let keep_workspace_commit = match disposition {
            WorkspaceDisposition::KeepWorkspaceCommit
            | WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit => {
                ws.kind.has_managed_commit()
            }
            WorkspaceDisposition::KeepWorkspaceReference
            | WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences => {
                remaining_tip_count > 1
            }
        };

        if !keep_workspace_commit {
            let new_head_id =
                commit_to_point_workspace_ref_to_after_unapply(ws, &future_workspace_tips)?;
            checkout_and_update_workspace_ref(
                repo,
                prev_head_id,
                new_head_id,
                workspace_ref_name,
                uncommitted_changes,
            )?;
            return Ok(WorkspaceRefUpdateAfterUnapply {
                entrypoint_id: new_head_id,
                workspace_merge: None,
            });
        }

        let mut in_memory_repo = repo.clone().for_tree_diffing()?.with_object_memory();
        let merge = merge_workspace_after_unapply(ws, &future_workspace_tips, &in_memory_repo)?;

        // materialize the merged objects.
        let new_head_id = merge.workspace_commit_id;
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
        Ok(WorkspaceRefUpdateAfterUnapply {
            entrypoint_id: new_head_id,
            workspace_merge: merge.workspace_merge,
        })
    }

    /// Result of updating a managed workspace ref while keeping the workspace metadata around.
    struct WorkspaceRefUpdateAfterUnapply {
        /// Commit to use as the entrypoint when rebuilding the workspace projection.
        entrypoint_id: gix::ObjectId,
        /// Merge attempt, present when rebuilding or trying to rebuild the workspace commit.
        workspace_merge: Option<crate::commit::merge::Outcome>,
    }

    struct WorkspaceMergeAfterUnapply {
        /// The commit to materialize as the new workspace ref target.
        workspace_commit_id: gix::ObjectId,
        /// The merge attempt callers should receive, if it represents a real workspace merge.
        ///
        /// This is `None` for the legacy empty-workspace keep-merge case, where the merge outcome
        /// is only an implementation detail used to create a managed no-op workspace commit.
        workspace_merge: Option<crate::commit::merge::Outcome>,
    }

    fn future_workspace_tips(
        ws_md: &but_core::ref_metadata::Workspace,
        ws: &but_graph::Workspace,
    ) -> anyhow::Result<Vec<crate::commit::merge::Tip>> {
        let crate::commit::merge::ResolvedTips {
            tips,
            missing_stacks,
        } = WorkspaceCommit::tips_from_metadata(
            ws_md.stacks.iter(),
            anon_stacks_to_preserve(ws),
            &ws.graph,
        );
        ensure!(
            missing_stacks.is_empty(),
            "Somehow some of the workspace stacks weren't part of the graph: {missing_stacks:#?}"
        );
        Ok(tips)
    }

    /// Unapply the managed workspace reference itself.
    ///
    /// - Run before normal branch removal because the workspace ref is a container, not a stack segment.
    /// - Reject dispositions that keep the workspace ref because unapplying it requires switching away.
    /// - Pick a named checkout target from the current projection.
    /// - Reject GitButler-conflicted target commits with a ref-aware error.
    /// - Safely checkout the target commit, then switch `HEAD` to the target ref.
    /// - Optionally delete the managed workspace ref and metadata.
    /// - Retraverse from the target ref so the returned workspace matches the new `HEAD`.
    fn unapply_workspace_reference<'ws>(
        ws: &'ws but_graph::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        workspace_ref_name: &FullNameRef,
        disposition: WorkspaceDisposition,
        uncommitted_changes: but_core::worktree::checkout::UncommitedWorktreeChanges,
    ) -> anyhow::Result<Outcome<'ws>> {
        if !disposition.may_switch_away_from_workspace() {
            bail!(
                "Cannot unapply workspace reference '{}' without switching away from it",
                workspace_ref_name.shorten()
            );
        }

        let ref_to_checkout = ref_to_checkout_after_workspace_unapply(ws)?;
        safe_checkout_ref_to_checkout(
            ws,
            repo,
            &ref_to_checkout,
            but_core::worktree::checkout::Options {
                uncommitted_changes,
                skip_head_update: true,
                ..Default::default()
            },
        )?;

        let workspace_ref_expected = ws
            .tip_commit()
            .context("BUG: unborn should be impossible here")?
            .id;
        switch_head_and_delete_workspace_ref(
            repo,
            ref_to_checkout.ref_name.as_ref(),
            workspace_ref_name,
            workspace_ref_expected,
        )?;
        // Fully remove the workspace, which includes the target branch.
        // For this we will need more flexibility, on-demand inference or
        // git-configuration based configuration, so there is always a fallback.
        meta.remove(workspace_ref_name)?;

        let overlay = Overlay::default().with_entrypoint(
            ref_to_checkout.commit_id,
            Some(ref_to_checkout.ref_name.clone()),
        );
        let ws = ws
            .graph
            .redo_traversal_with_overlay(repo, meta, overlay)?
            .into_workspace()?;
        Ok(Outcome {
            workspace: Cow::Owned(ws),
            checked_out: Some(ref_to_checkout.ref_name),
            workspace_merge: None,
        })
    }

    /// Rebuild the managed workspace commit after `branch` has been removed from metadata.
    ///
    /// The metadata is resolved to tips first so all rebuilds use the lower-level tip merge API.
    /// If metadata and anonymous workspace inputs resolve to no tips, the legacy keep-merge path
    /// still needs a managed commit, so we synthesize a single unnamed tip at the post-unapply
    /// base/checkout commit.
    fn merge_workspace_after_unapply(
        ws: &but_graph::Workspace,
        future_workspace_tips: &[crate::commit::merge::Tip],
        repo: &gix::Repository,
    ) -> anyhow::Result<WorkspaceMergeAfterUnapply> {
        let mut tips = future_workspace_tips.to_vec();
        let report_workspace_merge = !tips.is_empty();
        if tips.is_empty() {
            tips.push(base_tip_after_unapply(ws, future_workspace_tips)?);
        }
        let outcome = WorkspaceCommit::from_new_merge_with_tips(tips, &ws.graph, repo, None)?;
        ensure_workspace_merge_has_no_conflicts(&outcome)?;
        let workspace_commit_id = outcome.workspace_commit_id;
        Ok(WorkspaceMergeAfterUnapply {
            workspace_commit_id,
            workspace_merge: report_workspace_merge.then_some(outcome),
        })
    }

    fn ensure_workspace_merge_has_no_conflicts(
        outcome: &crate::commit::merge::Outcome,
    ) -> anyhow::Result<()> {
        ensure!(
            !outcome.has_conflicts(),
            "Failed to unapply branch due to conflicts while rebuilding the workspace merge commit: {}",
            describe_conflicting_stacks(&outcome.conflicting_stacks)
        );
        Ok(())
    }

    fn describe_conflicting_stacks(
        conflicting_stacks: &[crate::commit::merge::ConflictingStack],
    ) -> String {
        let ref_names = conflicting_stacks
            .iter()
            .filter_map(|stack| stack.ref_name.as_ref())
            .map(|ref_name| ref_name.shorten().to_string())
            .collect::<Vec<_>>();
        if ref_names.is_empty() {
            format!("{} stack(s)", conflicting_stacks.len())
        } else {
            ref_names.join(", ")
        }
    }

    /// Convert the commit that should remain checked out after unapply into a synthetic merge tip.
    fn base_tip_after_unapply(
        ws: &but_graph::Workspace,
        future_workspace_tips: &[crate::commit::merge::Tip],
    ) -> anyhow::Result<crate::commit::merge::Tip> {
        let commit_id = commit_to_point_workspace_ref_to_after_unapply(ws, future_workspace_tips)?;
        let segment_idx = ws
            .graph
            .segment_by_commit_id(commit_id)
            .with_context(|| {
                format!("BUG: could not find workspace segment for base commit {commit_id}")
            })?
            .id;
        Ok(crate::commit::merge::Tip {
            name: None,
            commit_id,
            segment_idx,
        })
    }

    /// Return anonymous stacks that should be kept while rebuilding the workspace merge commit.
    ///
    /// Reprojecting after metadata changes can leave old workspace-commit parents visible as
    /// anonymous stacks. If such a stack still has a metadata [stack id](but_core::ref_metadata::StackId),
    /// it is a projected remnant of the old workspace commit rather than independent anonymous work,
    /// and feeding it back into the merge would preserve the removed stack.
    fn anon_stacks_to_preserve<'a>(
        ws: &'a but_graph::Workspace,
    ) -> impl Iterator<Item = (usize, crate::commit::merge::Tip)> + 'a {
        anon_stacks(&ws.stacks)
            .filter(move |(idx, _)| ws.stacks.get(*idx).is_none_or(|stack| stack.id.is_none()))
    }

    /// Local tracking branch of target ref or the most recent named stack tip.
    fn ref_to_checkout_after_workspace_unapply(
        ws: &but_graph::Workspace,
    ) -> anyhow::Result<RefToCheckout> {
        if let Some(target) = local_tracking_branch_of_target(ws)? {
            return Ok(target);
        }
        named_stack_with_lowest_generation(ws)?.with_context(
            || "Cannot unapply workspace reference because no target or named stack could be found",
        )
    }

    /// The idea here is to put the user at the topologically most recent stack.
    /// This is also arbitrary, but *feels* like what one would want.
    fn named_stack_with_lowest_generation(
        ws: &but_graph::Workspace,
    ) -> anyhow::Result<Option<RefToCheckout>> {
        let mut selected = None;
        for stack in &ws.stacks {
            let Some((sidx, ref_info)) = stack
                .segments
                .first()
                .and_then(|s| s.ref_info.as_ref().map(|ri| (s.id, ri)))
            else {
                continue;
            };
            let generation = ws.graph[sidx].generation;
            let ref_to_checkout = RefToCheckout::from_segment_ref_info(ws, sidx, ref_info)?;
            if selected
                .as_ref()
                .is_none_or(|(best_generation, _)| generation < *best_generation)
            {
                selected = Some((generation, ref_to_checkout));
            }
        }
        Ok(selected.map(|(_, ref_to_checkout)| ref_to_checkout))
    }

    /// Run `safe_checkout`, but provide better error messages if the commit to checkout
    /// is conflicted.
    fn safe_checkout(
        repo: &gix::Repository,
        prev_head_id: gix::ObjectId,
        new_head_id: gix::ObjectId,
        options: but_core::worktree::checkout::Options,
    ) -> anyhow::Result<but_core::worktree::checkout::Outcome> {
        if but_core::Commit::from_id(new_head_id.attach(repo))?.is_conflicted() {
            bail!("Cannot unapply branch by checking out conflicted commit {new_head_id}");
        }
        but_core::worktree::safe_checkout(prev_head_id, new_head_id, repo, options)
    }

    /// Check out `ref_to_checkout` using the workspace traversal entrypoint as the
    /// current worktree/index source.
    ///
    /// `ws` must be the workspace projection for the currently checked-out `HEAD`.
    /// Its graph entrypoint is therefore the commit the index and worktree are
    /// expected to match before checkout. This matters when `HEAD` points at a
    /// stack segment inside the workspace rather than the workspace tip.
    ///
    /// The helper only updates the index/worktree according to `options`; callers
    /// remain responsible for any subsequent `HEAD`, reference, metadata, and
    /// projection updates.
    fn safe_checkout_ref_to_checkout(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        ref_to_checkout: &RefToCheckout,
        options: but_core::worktree::checkout::Options,
    ) -> anyhow::Result<but_core::worktree::checkout::Outcome> {
        if but_core::Commit::from_id(ref_to_checkout.commit_id.attach(repo))?.is_conflicted() {
            bail!(
                "Cannot unapply workspace reference by checking out conflicted commit at '{}'",
                ref_to_checkout.ref_name.shorten()
            );
        }
        let prev_head_id = ws
            .graph
            .entrypoint()?
            .commit()
            .context("BUG: unborn was skipped, why no entrypoint")?
            .id;
        safe_checkout(repo, prev_head_id, ref_to_checkout.commit_id, options)
    }

    /// Return the commit the workspace ref should point to when no workspace merge commit remains.
    ///
    /// A single remaining future tip is preferred. Otherwise, an empty workspace falls back to the
    /// resolved target or the workspace lower bound.
    fn commit_to_point_workspace_ref_to_after_unapply(
        ws: &but_graph::Workspace,
        future_workspace_tips: &[crate::commit::merge::Tip],
    ) -> anyhow::Result<gix::ObjectId> {
        if let Some(tip) = future_workspace_tips.first() {
            return Ok(tip.commit_id);
        }
        ws.resolved_target_commit_id()
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
        safe_checkout(
            repo,
            prev_head_id,
            new_head_id,
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

    /// Determine whether unapply should delete the workspace reference and switch `HEAD` to a regular ref.
    ///
    /// `ws` is the current workspace projection after the managed workspace ref was already
    /// updated and re-projected. It is the source of truth for the resulting stack shape, including
    /// virtual stacks. `disposition` controls whether deleting the workspace reference is allowed
    /// at all.
    ///
    /// Return the ref to check out after deleting the workspace ref, or `None` if the workspace
    /// reference must remain checked out.
    fn ref_to_checkout_after_workspace_deletion(
        ws: &but_graph::Workspace,
        disposition: WorkspaceDisposition,
    ) -> anyhow::Result<Option<RefToCheckout>> {
        if !disposition.may_delete_workspace_reference() {
            return Ok(None);
        }

        match ws.stacks.first() {
            None => {
                if let Some(fallback) = local_tracking_branch_of_target(ws)? {
                    return Ok(Some(fallback));
                }

                tracing::warn!(
                    "keeping workspace reference after unapply because no non-stack checkout fallback is available"
                );
            }
            Some(first_stack) if ws.stacks.len() == 1 => {
                if let Some(ref_to_checkout) = stack_to_checkout(ws, first_stack)? {
                    return Ok(Some(ref_to_checkout));
                }
                tracing::warn!(
                    "keeping workspace reference after unapply because the remaining stack has no ref to check out"
                );
            }
            _ => {}
        }
        Ok(None)
    }

    fn stack_to_checkout(
        ws: &but_graph::Workspace,
        stack: &but_graph::workspace::Stack,
    ) -> anyhow::Result<Option<RefToCheckout>> {
        stack
            .segments
            .first()
            .and_then(|segment| {
                segment
                    .ref_info
                    .as_ref()
                    .map(|ref_info| RefToCheckout::from_segment_ref_info(ws, segment.id, ref_info))
            })
            .transpose()
    }

    /// Return the local branch to check out for the workspace target.
    ///
    /// `ws` is the current graph projection with adjusted metadata. The workspace target already
    /// carries the local tracking branch inferred while building the graph, including the peeled
    /// commit id to check out.
    fn local_tracking_branch_of_target(
        ws: &but_graph::Workspace,
    ) -> anyhow::Result<Option<RefToCheckout>> {
        let Some(target_ref) = ws.target_ref.as_ref() else {
            return Ok(None);
        };
        let Some(local_target_ref_sidx) = ws.graph[target_ref.segment_index].sibling_segment_id
        else {
            return Ok(None);
        };
        let Some(ref_info) = ws.graph[local_target_ref_sidx].ref_info.as_ref() else {
            return Ok(None);
        };
        RefToCheckout::from_segment_ref_info(ws, local_target_ref_sidx, ref_info).map(Some)
    }

    /// Ref name and peeled commit id selected from the workspace projection for checkout.
    struct RefToCheckout {
        ref_name: FullName,
        // The commit that `ref_name` is pointing to.
        commit_id: gix::ObjectId,
    }

    impl RefToCheckout {
        fn from_segment_ref_info(
            ws: &but_graph::Workspace,
            segment_id: but_graph::SegmentIndex,
            ref_info: &but_graph::RefInfo,
        ) -> anyhow::Result<Self> {
            Ok(RefToCheckout {
                ref_name: ref_info.ref_name.clone(),
                commit_id: ws
                    .tip_commit_by_segment_id(segment_id)
                    .map(|commit| commit.id)
                    .or(ref_info.commit_id)
                    .with_context(|| {
                        format!(
                            "Cannot check out '{}' because it does not point to a commit",
                            ref_info.ref_name.shorten()
                        )
                    })?,
            })
        }
    }

    /// Delete `workspace_ref_name` and point `HEAD` symbolically at `target_ref`.
    /// `workspace_ref_commit_id` is the commit that the workspace ref is pointing
    /// to currently.
    ///
    /// This is merely a ref-edit.
    fn switch_head_and_delete_workspace_ref(
        repo: &gix::Repository,
        target_ref: &FullNameRef,
        workspace_ref_name: &FullNameRef,
        workspace_ref_commit_id: gix::ObjectId,
    ) -> anyhow::Result<()> {
        repo.edit_references([
            RefEdit {
                change: Change::Delete {
                    log: RefLog::AndReference,
                    expected: PreviousValue::MustExistAndMatch(Target::Object(
                        workspace_ref_commit_id,
                    )),
                },
                name: workspace_ref_name.to_owned(),
                deref: false,
            },
            RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "GitButler switch away from workspace during unapply-branch"
                            .into(),
                    },
                    expected: PreviousValue::Any,
                    new: Target::Symbolic(target_ref.to_owned()),
                },
                name: "HEAD".try_into().expect("well-formed root ref"),
                deref: false,
            },
        ])?;
        Ok(())
    }
}
