/// Returned by [unapply()](function::unapply()).
pub struct Outcome {
    /// The updated workspace substrate, or `None` if the graph already didn't contain the
    /// desired branch and nothing had to be unapplied — the caller's workspace remains
    /// current then (note that metadata changes might not be included in that case, as they
    /// aren't the source of truth). Display callers materialize the pruned shape via
    /// [`display_stacks`](but_graph::Workspace::display_stacks) (or
    /// [`Self::display_workspace`]).
    pub workspace: Option<but_graph::Workspace>,
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
    /// The rebuilt workspace merge commit, when the unapply re-merged the remaining
    /// stack tips (absent when the workspace collapsed, emptied, or was already right).
    pub workspace_merge: Option<gix::ObjectId>,
}

impl Outcome {
    fn new(ws: Option<but_graph::Workspace>) -> Self {
        Outcome {
            workspace: ws,
            checked_out: None,
            workspace_merge: None,
        }
    }

    /// Return `true` if a new graph traversal was performed, which always is a sign for an operation which changed the workspace.
    /// This is `false` if the branch to unapply was already absent from the current workspace.
    pub fn workspace_changed(&self) -> bool {
        self.workspace.is_some()
    }

    /// The carried workspace, cloned for rendering and other display boundaries; pruning happens
    /// when the caller asks it for [`display_stacks`](but_graph::Workspace::display_stacks).
    /// Errors when the unapply was a no-op: the caller's own workspace is the current one then.
    pub fn display_workspace(&self) -> anyhow::Result<but_graph::Workspace> {
        use anyhow::Context as _;
        Ok(self
            .workspace
            .as_ref()
            .context("unapply was a no-op; the input workspace is unchanged")?
            .clone())
    }
}

impl std::fmt::Debug for Outcome {
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

/// The four variants are a 2x2 over two INDEPENDENT questions, which is why one of them wears a
/// name that is two names glued together. Nothing reads the cross-product — every use collapses to
/// one axis or the other, so both are named here and asked for by name.
impl WorkspaceDisposition {
    /// Axis 1: may unapply leave the workspace ref entirely, switching `HEAD` to a plain branch?
    fn may_switch_away_from_workspace(self) -> bool {
        matches!(
            self,
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences
                | WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit
        )
    }

    /// Axis 2: where the workspace ref DOES remain, must the merge commit remain under it even
    /// when nothing needs it? Independent of axis 1: it only decides what is left behind.
    fn insists_on_workspace_commit(self) -> bool {
        matches!(
            self,
            WorkspaceDisposition::KeepWorkspaceCommit
                | WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit
        )
    }

    fn may_delete_workspace_reference(self) -> bool {
        // Deleting the workspace reference always requires switching away from it, so today the
        // two permissions coincide; kept as distinct names for the distinct questions callers ask.
        self.may_switch_away_from_workspace()
    }
}

/// Options for [branch::unapply()](function::unapply()).
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How to represent the workspace after the stack has been removed.
    pub workspace_disposition: WorkspaceDisposition,
}

pub(crate) mod function {
    use super::{Options, Outcome, WorkspaceDisposition};
    use anyhow::{Context as _, bail, ensure};

    use but_core::{RefMetadata, ref_metadata::ProjectMeta};
    use but_graph::walk::Overlay;
    use gix::{
        prelude::ObjectIdExt,
        refs::{
            FullName, FullNameRef, Target,
            transaction::{Change, PreviousValue, RefEdit, RefLog},
        },
    };

    use crate::branch::try_find_validated_ref;

    /// Remove `branch` from `workspace`, updating `repo` and `meta` so the resulting workspace is the inverse of applying that branch.
    ///
    /// Think of it as "remove `branch` from the workspace", somehow, and metadata may be a way to do. This also means
    /// this function can be applied indiscriminately, and in the worst case, it will do nothing.
    ///
    /// `branch` must name a branch to remove from the workspace. If it is already absent from
    /// `workspace`, this is a no-op and the returned [`Outcome`] carries no workspace of its own. Symbolic names such as `HEAD` are
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
    /// - Derive the future stack tips from the current workspace instead of reprojecting: the
    ///   carried stacks already say which stacks remain, which is what the merge and collapse
    ///   decisions need.
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
    ///   the workspace ref is replaced by its target's local branch, or by the named stack with the highest
    ///   generation (i.e. the topologically 'newest') if there is no target.
    #[tracing::instrument(skip(workspace, repo, meta), err(Debug))]
    pub fn unapply(
        branch: &FullNameRef,
        workspace: &but_graph::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            workspace_disposition,
        }: Options,
    ) -> anyhow::Result<Outcome> {
        let ws = workspace;
        let branch_ref = try_find_validated_ref(repo, branch, "unapply")?;

        crate::branch::ensure_workspace_commit_at_top(ws, repo)?;

        let workspace_ref_name = ws.ref_name_owned();
        // An ad-hoc workspace has no managed merge to remove a stack from: you can only
        // APPLY branches to build one, never unapply. A branch that isn't part of the
        // workspace at all stays a harmless no-op (idempotency) via the check below;
        // anything actually present — the workspace ref itself or a projected segment — is
        // refused here.
        if matches!(ws.kind(), but_graph::workspace::WorkspaceKind::AdHoc) {
            let is_workspace_ref = workspace_ref_name
                .as_ref()
                .is_some_and(|name| name.as_ref() == branch);
            if is_workspace_ref || ws.find_branch(branch).is_some() {
                bail!(
                    "Cannot unapply '{branch}' from an ad-hoc workspace; ad-hoc workspaces only support applying branches",
                    branch = branch.shorten()
                );
            }
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
            );
        }

        let branch_in_ws = ws.find_branch(branch);
        #[cfg(debug_assertions)]
        but_graph::declared::debug_assert_declared_branch_is_visible(
            ws,
            branch,
            branch_in_ws.map(|(stack, _)| stack.id),
        );
        if branch_in_ws.is_none() {
            if branch_ref.is_none() {
                bail!(
                    "Cannot unapply non-existing branch '{branch}'",
                    branch = branch.shorten()
                );
            }
            // The branch exists in Git, but does not in the workspace: Nothing to do.
            return Ok(Outcome::new(None));
        }
        let branch_stack_was_entrypoint = ws.entry_marks_stack_of(branch);
        let workspace_tip_was_entrypoint = ws.is_entrypoint();

        let Some(workspace_ref_name) = workspace_ref_name else {
            // This is an ad-hoc workspace by merit of being unnamed.
            bail!("Cannot unapply a branch from an ad-hoc detached workspace");
        };
        let mut ws_md = meta.workspace(workspace_ref_name.as_ref())?;
        // The branch leaves the DECLARATION with one targeted edit — nothing else in
        // metadata is touched. An undeclared branch (a natural workspace) simply has
        // nothing to remove; the graph edit below works either way.
        let branch_removed_from_ws_meta = ws_md.unapply_branch(branch);
        if !(branch_removed_from_ws_meta || ws.kind().has_managed_ref() || ws.has_metadata()) {
            // The branch wasn't in workspace metadata, yet it was present, so also delete its branch metadata
            // as it could be used to disambiguate the segment.
            // TODO: this will actually be observable even if it doens't work, unless it's run in a transaction, which right now it's not!
            //       Should be able to re-run the traversal with an overlay that hides branch metadata, but I'd say it's not important enough.
            meta.remove(branch)?;
            let workspace = ws.rederive_with(repo, meta, Overlay::default())?;
            if workspace.refname_is_segment(branch) {
                bail!(
                    "Cannot unapply branch '{branch}' from an ad-hoc workspace because non-tip branches can only disappear if their now removed metadata disambiguated them",
                    branch = branch.shorten()
                );
            }
            return Ok(Outcome::new(Some(workspace)));
        }

        // THE GRAPH EDIT: unapplying a stack's TIP branch
        // removes the entire stack — its workspace parent goes. A mid-stack branch is pure
        // de-listing: its commits stay with the stack and the graph is already right.
        enum ParentEdit {
            KeepAsIs,
            Remove,
        }
        let parent_edit = if ws.is_stack_tip(branch) {
            ParentEdit::Remove
        } else {
            ParentEdit::KeepAsIs
        };
        // A GENUINE projection question: which of THESE stacks is the subject, positionally.
        // Two stacks can rest on one commit, so the commit cannot name the subject and an index
        // is what distinguishes them. It stays inside this function and is only ever used against
        // the `ws` it came from — an index is meaningless in any other view of the same graph.
        let subject_stack_index = ws
            .segment_location(branch)
            .map(|(stack_idx, _)| stack_idx)
            .context("BUG: the subject was found in the workspace above")?;
        // The tips the workspace has AFTER the edit — the disposition decides on them
        // like it always has (collapse when one or none remains and the disposition
        // does not insist on a workspace commit).
        //
        // The segment graph answers — already derived from the recorded facts.
        // Index-based subject removal because stacks can share one commit.
        let future_tips: Vec<gix::ObjectId> = ws
            .stacks
            .iter()
            .enumerate()
            .filter_map(|(index, stack)| {
                if index == subject_stack_index && matches!(parent_edit, ParentEdit::Remove) {
                    None
                } else {
                    stack.resting_commit()
                }
            })
            .collect();
        let keep_workspace_commit = if workspace_disposition.insists_on_workspace_commit() {
            ws.kind().has_managed_commit()
        } else {
            future_tips.len() > 1
        };

        let (entrypoint_id, workspace_merge) = if !keep_workspace_commit {
            // Collapse: the workspace ref points straight at what remains.
            let new_head_id = future_tips
                .first()
                .copied()
                .or_else(|| ws.resolved_target_commit_id())
                .or(ws.lower_bound())
                .context("Cannot determine commit for empty workspace after unapply")?;
            checkout_and_update_workspace_ref(repo, new_head_id, workspace_ref_name.as_ref())?;
            (new_head_id, None)
        } else if matches!(parent_edit, ParentEdit::KeepAsIs) {
            // Mid-stack de-listing: the merge is already right.
            (
                ws.tip_commit_id()
                    .context("BUG: a kept workspace commit implies a tip")?,
                None,
            )
        } else {
            // One editor mutation: the workspace merge minus the subject's parent (onto
            // the base when nothing remains). Where the subject is the physical top the parent
            // is selected BY REFERENCE, routing the removal through the subject group's own
            // carried edges so duplicate parents disambiguate per ref; otherwise it is severed
            // by commit (see below). materialize() persists objects, safe-checkouts,
            // and only then moves refs — the abort-before-ref-moves discipline.
            let ws_commit = ws
                .tip_commit_id()
                .context("BUG: a kept workspace commit implies a tip")?;
            let mut editor = but_rebase::graph_rebase::Editor::for_workspace(ws, meta, repo)?;
            let ws_entry = editor.select_commit(ws_commit)?;
            // The subject is the stack's NAMED tip, but anonymous segments — residue of
            // removed refs — may sit physically above it, and then the stack's workspace
            // edge is carried up there, not by the subject's group. Route by ref when the
            // subject IS the physical top (per-ref duplicate disambiguation); else sever
            // by the stack's resting commit, which is the workspace parent itself.
            let subject_stack = ws
                .stacks
                .get(subject_stack_index)
                .context("BUG: subject stack index out of the segment graph's range")?;
            let subject_is_physical_top = subject_stack
                .top()
                .is_some_and(|segment| segment.ref_name() == Some(branch));
            let removed = if subject_is_physical_top {
                let subject_ref = editor.select_reference(branch)?;
                editor.detach(ws_entry, subject_ref)?
            } else {
                let parent = subject_stack
                    .resting_commit()
                    .context("BUG: a removable stack rests on a commit")?;
                // Detach-by-commit severs EVERY parent slot resolving to this
                // commit — with duplicate parents (a sibling collapsed onto the
                // same commit) that would silently unapply the sibling too. Refuse
                // until slot-precise surgery exists for this shape.
                let duplicated = ws
                    .stacks
                    .iter()
                    .enumerate()
                    .filter(|&(idx, stack)| {
                        idx != subject_stack_index && stack.resting_commit() == Some(parent)
                    })
                    .count();
                ensure!(
                    duplicated == 0,
                    "cannot unapply {branch}: its stack shares its workspace parent \
                     commit with {duplicated} sibling stack(s), and removing the \
                     parent edge would unapply them too",
                    branch = branch.shorten(),
                );
                let parent_pick = editor.select_commit(parent)?;
                editor.detach(ws_entry, parent_pick)?
            };
            if removed.is_empty() {
                // A leftover empty can rest on history inside or below another lane —
                // e.g. created free-standing while the workspace is DEGRADED (the ref
                // points at a bare lane tip, no managed merge) — and then there is no
                // physical leg to sever: the graph is already right and the removal is
                // declaration-only. A rest that IS a workspace parent with nothing
                // severed remains a hard error.
                let ws_parents: Vec<_> = ws.commit_graph().parents(ws_commit).collect();
                ensure!(
                    subject_stack
                        .resting_commit()
                        .is_none_or(|rest| !ws_parents.contains(&rest)),
                    "BUG: the subject's workspace parent must carry at least one edge"
                );
                (ws_commit, None)
            } else {
                if future_tips.is_empty() {
                    // An emptied workspace keeps its managed commit ON the base — the
                    // subject's parent was its only content, so re-parent unconditionally
                    // (a residual parent, were one ever present, must not leave the
                    // emptied commit off the base).
                    let base = ws
                        .resolved_target_commit_id()
                        .or(ws.lower_bound())
                        .context("Cannot determine commit for empty workspace after unapply")?;
                    let base_pick = editor.select_commit(base)?;
                    editor.insert_parent(ws_entry, base_pick, 0)?;
                }
                let materialized = editor.rebase()?.materialize()?;
                drop(materialized);
                let new_head_id = repo
                    .find_reference(workspace_ref_name.as_ref())?
                    .peel_to_id()?
                    .detach();
                (
                    new_head_id,
                    (!future_tips.is_empty()).then_some(new_head_id),
                )
            }
        };
        meta.set_workspace(&ws_md)?;
        // Update the workspace *only* after a successful workspace commit merge.
        let overlay = Overlay::default()
            .with_dropped_references([branch.to_owned()])
            .with_workspace_metadata_override(Some((workspace_ref_name.to_owned(), ws_md.clone())));
        let mut ws = ws.rederive_with(repo, meta, overlay)?;
        let checked_out = if !workspace_tip_was_entrypoint
            && (ws.is_entrypoint() || branch_stack_was_entrypoint)
        {
            // The workspace tip never was the entrypoint, meaning something inside
            // was the entrypoint, and now it's not visible anymore as that stack was unapplied.
            // Now we checkout the enclosing workspace instead.
            switch_head_to_workspace_ref(repo, workspace_ref_name.as_ref(), entrypoint_id)?;
            let overlay = Overlay::default()
                .with_dropped_references([branch.to_owned()])
                .with_entrypoint(entrypoint_id, Some(workspace_ref_name.to_owned()));
            ws = ws.rederive_with(repo, meta, overlay)?;
            Some(workspace_ref_name.to_owned())
        } else {
            None
        };
        if ws_md.stacks.iter().any(|stack| {
            stack.workspacecommit_relation.is_in_workspace()
                && stack
                    .branches
                    .iter()
                    .any(|stack_branch| stack_branch.ref_name.as_ref() == branch)
        }) {
            bail!(
                "BUG: branch '{}' is still present in rebuilt workspace metadata after unapply",
                branch.shorten()
            );
        }
        // Checkout-target selection is user-facing: it reads the pruned display stacks.
        let display = ws;
        match ref_to_checkout_after_workspace_deletion(&display, workspace_disposition)? {
            Some(ref_to_switch_to) => {
                // The rebuilt workspace can be discarded entirely, switching to another branch.
                safe_checkout_ref_to_checkout(
                    repo,
                    &ref_to_switch_to,
                    but_core::worktree::checkout::Options {
                        // We will be setting the HEAD ourselves.
                        skip_head_update: true,
                        ..Default::default()
                    },
                )?;
                switch_head_and_delete_workspace_ref(
                    repo,
                    ref_to_switch_to.ref_name.as_ref(),
                    workspace_ref_name.as_ref(),
                    display
                        .tip_commit_id()
                        .context("BUG: unborn should be impossible here")?,
                )?;
                // Keep the workspace metadata or we lose the target branch.
                // Currently that's a problem, so deal with it later.

                let overlay = Overlay::default()
                    .with_entrypoint(
                        ref_to_switch_to.commit_id,
                        Some(ref_to_switch_to.ref_name.clone()),
                    )
                    .with_dropped_references([branch.to_owned()]);
                let ws = display.rederive_with(repo, meta, overlay)?;

                Ok(Outcome {
                    workspace: Some(ws),
                    checked_out: Some(ref_to_switch_to.ref_name),
                    workspace_merge: None,
                })
            }
            None => Ok(Outcome {
                workspace: Some(display),
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
        repo.edit_reference(crate::branch::ref_edits::head_to_ref(
            workspace_ref_name,
            "GitButler switch to workspace during unapply-branch",
        ))?;
        Ok(())
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
    fn unapply_workspace_reference(
        ws: &but_graph::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        workspace_ref_name: &FullNameRef,
        disposition: WorkspaceDisposition,
    ) -> anyhow::Result<Outcome> {
        if !disposition.may_switch_away_from_workspace() {
            bail!(
                "Cannot unapply workspace reference '{}' without switching away from it",
                workspace_ref_name.shorten()
            );
        }

        // Checkout-target selection is user-facing: it reads the pruned display stacks
        // rather than the substrate operations read.
        let ref_to_checkout = ref_to_checkout_after_workspace_unapply(ws)?;
        safe_checkout_ref_to_checkout(
            repo,
            &ref_to_checkout,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )?;

        let workspace_ref_expected = ws
            .tip_commit_id()
            .context("BUG: unborn should be impossible here")?;
        switch_head_and_delete_workspace_ref(
            repo,
            ref_to_checkout.ref_name.as_ref(),
            workspace_ref_name,
            workspace_ref_expected,
        )?;
        // Fully remove the workspace, which includes the target branch.
        meta.remove(workspace_ref_name)?;
        // The project metadata ported to repo-local Git config mirrors the just-removed
        // workspace metadata, so clear it as well or the deleted target would keep resolving.
        ProjectMeta::remove_from_local_config(repo)?;

        let overlay = Overlay::default().with_entrypoint(
            ref_to_checkout.commit_id,
            Some(ref_to_checkout.ref_name.clone()),
        );
        let ws = ws.rederive_with(repo, meta, overlay)?;
        Ok(Outcome {
            workspace: Some(ws),
            checked_out: Some(ref_to_checkout.ref_name),
            workspace_merge: None,
        })
    }

    /// Local tracking branch of target ref or the most recent named stack tip.
    fn ref_to_checkout_after_workspace_unapply(
        ws: &but_graph::Workspace,
    ) -> anyhow::Result<RefToCheckout> {
        if let Some(target) = local_tracking_branch_of_target(ws)? {
            return Ok(target);
        }
        most_recent_named_stack(ws)?.with_context(
            || "Cannot unapply workspace reference because no target or named stack could be found",
        )
    }

    /// The idea here is to put the user at the topologically most recent stack — the one whose
    /// first segment's anchor commit (its tip, or the pointed-at commit for an empty splice)
    /// sits deepest in the carried commit graph; stack order breaks ties. This is also
    /// arbitrary, but *feels* like what one would want.
    fn most_recent_named_stack(ws: &but_graph::Workspace) -> anyhow::Result<Option<RefToCheckout>> {
        let cg = ws.commit_graph();
        let mut selected = None;
        for stack in ws.display_stacks()? {
            let Some((ref_info, anchor)) = stack.segments.first().and_then(|s| {
                s.ref_info
                    .as_ref()
                    .map(|ri| (ri, s.commits.first().map(|c| c.id).or(ri.commit_id)))
            }) else {
                continue;
            };
            let generation = anchor
                .and_then(|anchor| cg.generation_of(anchor))
                .unwrap_or(0);
            let ref_to_checkout = RefToCheckout::from_segment_ref_info(ws, ref_info)?;
            if selected
                .as_ref()
                .is_none_or(|(best_generation, _)| generation > *best_generation)
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
        new_head_id: gix::ObjectId,
        options: but_core::worktree::checkout::Options,
    ) -> anyhow::Result<but_core::worktree::checkout::Outcome> {
        if but_core::Commit::from_id(new_head_id.attach(repo))?.is_conflicted() {
            bail!("Cannot unapply branch by checking out conflicted commit {new_head_id}");
        }
        but_core::worktree::safe_checkout_from_head(new_head_id, repo, options)
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
        // The ref-aware conflict check above already covered what `safe_checkout` would re-check.
        but_core::worktree::safe_checkout_from_head(ref_to_checkout.commit_id, repo, options)
    }

    /// Safely update the worktree and move the managed workspace ref to `new_head_id`.
    ///
    /// The checkout runs before the ref update so uncommitted-change conflicts abort without
    /// changing repository refs. `HEAD` is expected to remain symbolically attached to the workspace
    /// ref, so the checkout skips its own head update and this function updates only the ref target.
    fn checkout_and_update_workspace_ref(
        repo: &gix::Repository,
        new_head_id: gix::ObjectId,
        workspace_ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        safe_checkout(
            repo,
            new_head_id,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )?;
        repo.edit_reference(crate::branch::ref_edits::ref_to_commit(
            workspace_ref_name.to_owned(),
            new_head_id,
            "GitButler update workspace during unapply-branch",
        ))?;
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

        match ws.display_stacks()?.first() {
            None => {
                if let Some(fallback) = local_tracking_branch_of_target(ws)? {
                    return Ok(Some(fallback));
                }

                tracing::warn!(
                    "keeping workspace reference after unapply because no non-stack checkout fallback is available"
                );
            }
            Some(first_stack) if ws.display_stacks()?.len() == 1 => {
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
                    .map(|ref_info| RefToCheckout::from_segment_ref_info(ws, ref_info))
            })
            .transpose()
    }

    /// The local branch tracking the workspace target (e.g. `main` for `origin/main`), resolved
    /// as data: the graph's carried tracking map names it, the carried commit graph positions it.
    ///
    /// `ws` is the current graph projection with adjusted metadata.
    fn local_tracking_branch_of_target(
        ws: &but_graph::Workspace,
    ) -> anyhow::Result<Option<RefToCheckout>> {
        let Some(target_ref) = ws.target_ref.as_ref() else {
            return Ok(None);
        };
        Ok(ws
            .local_tracking_branch(target_ref.ref_name.as_ref())
            .and_then(|local| {
                let cg = ws.commit_graph();
                cg.commit_by_ref(local.as_ref())
                    // A local proven behind the target is carried as a seed fact
                    // instead of being walked to — still the right checkout.
                    .or_else(|| cg.behind_target_local_tip(local.as_ref()))
                    .map(|commit_id| RefToCheckout {
                        ref_name: local.clone(),
                        commit_id,
                    })
            }))
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
            ref_info: &but_graph::RefInfo,
        ) -> anyhow::Result<Self> {
            Ok(RefToCheckout {
                ref_name: ref_info.ref_name.clone(),
                // Checkout-target selection is USER-FACING: resolve the resting commit over the
                // pruned DISPLAY (what the user sees), deliberately — this ref was itself picked
                // from the display. Structural OPERATION decisions resolve on the segment graph
                // instead; the checkout path is the one intended exception.
                commit_id: ws
                    .branch_resting_commit_id_in_display(ref_info.ref_name.as_ref())
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
            crate::branch::ref_edits::head_to_ref(
                target_ref,
                "GitButler switch away from workspace during unapply-branch",
            ),
        ])?;
        Ok(())
    }
}
