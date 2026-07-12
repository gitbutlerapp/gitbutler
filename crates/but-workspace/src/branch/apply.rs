use bstr::ByteSlice as _;
use but_core::{
    WORKSPACE_REF_NAME,
    ref_metadata::{StackId, StackKind},
};

use crate::branch::{OnWorkspaceMergeConflict, try_find_validated_ref};
use std::ops::ControlFlow;

/// A stack that conflicted while applying a branch.
#[derive(Clone)]
pub struct ConflictingStack {
    /// The stable id of the stack in workspace metadata.
    pub id: StackId,
    /// The tip branch name of the stack.
    /// Currently we require it to be named.
    pub ref_name: gix::refs::FullName,
}

impl std::fmt::Debug for ConflictingStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConflictingStack")
            .field("id", &self.id)
            .field("ref_name", &self.ref_name.to_string())
            .finish()
    }
}

/// What kind of apply operation completed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum OutcomeStatus {
    /// The branch was already active in the current workspace, so nothing changed.
    AlreadyApplied,
    /// The branch was applied or recorded in the workspace.
    Applied,
    /// A workspace merge was attempted and conflicts prevented persistence.
    ConflictAborted,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(OutcomeStatus);

impl OutcomeStatus {
    /// The stable lower-camel-case name used by machine-readable CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            OutcomeStatus::AlreadyApplied => "alreadyApplied",
            OutcomeStatus::Applied => "applied",
            OutcomeStatus::ConflictAborted => "conflictAborted",
        }
    }

    /// Whether this status represents a persisted repository/workspace mutation.
    pub fn persisted_mutation(self) -> bool {
        matches!(self, OutcomeStatus::Applied)
    }
}

/// Returned by [apply()].
pub struct Outcome {
    /// The workspace as it looks like now.
    ///
    /// The returned workspace can also be a proposed state that was not persisted because
    /// [Options::on_workspace_conflict] aborted the operation. Check [Outcome::conflicting_stacks]
    /// and [Outcome::status] to tell whether anything was actually persisted.
    pub workspace: but_graph::Workspace,
    /// The precise kind of apply operation that completed.
    pub status: OutcomeStatus,
    /// The branch(es) that were activated or recorded by the operation.
    ///
    /// This is empty when `apply()` did not persist any branch, including when the branch was already
    /// present in the workspace or when workspace merge conflicts aborted the operation. Use [Outcome::status]
    /// to distinguish applied, no-op, and conflict-aborted outcomes.
    ///
    /// If a remote tracking branch is given to apply, it will actually apply its local tracking branch, which is created on demand as well.
    /// Further, if there is no target or if the current branch isn't the target branch, then the current branch and the given one
    /// will be applied, and two branches are listed here.
    pub applied_branches: Vec<gix::refs::FullName>,
    /// `true` if we created the given workspace ref as it didn't exist yet.
    pub workspace_ref_created: bool,
    /// If not `None`, an actual merge was attempted, but depending on [the settings](OnWorkspaceMergeConflict),
    /// this was persisted or not.
    pub workspace_merge: Option<crate::commit::merge::Outcome>,
    /// Stacks that conflicted while trying to apply the branch.
    ///
    /// Each entry includes the stable stack id and its tip ref name, so callers don't have to
    /// recover names from the returned workspace graph.
    pub conflicting_stacks: Vec<ConflictingStack>,
}

impl Outcome {
    /// Return `true` if apply performed work that should be visible to callers, including metadata-only repairs.
    /// This is `false` only for a true already-applied no-op.
    pub fn workspace_changed(&self) -> bool {
        !matches!(self.status, OutcomeStatus::AlreadyApplied)
    }

    /// The resulting workspace, cloned — a convenience for render sites.
    pub fn display_workspace(&self) -> anyhow::Result<but_graph::Workspace> {
        Ok(self.workspace.clone())
    }
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Outcome {
            workspace: _,
            status: _,
            workspace_ref_created,
            workspace_merge: _,
            conflicting_stacks,
            applied_branches,
        } = self;
        let mut f = f.debug_struct("Outcome");
        f.field("workspace_changed", &self.workspace_changed())
            .field("workspace_ref_created", workspace_ref_created)
            .field(
                "applied_branches",
                &format!(
                    "[{}]",
                    applied_branches
                        .iter()
                        .map(|rn| rn.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        if !conflicting_stacks.is_empty() {
            f.field("conflicting_stacks", conflicting_stacks);
        }
        f.finish()
    }
}

/// How the newly applied branch should be merged into the workspace commit.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkspaceMerge {
    /// Do nothing but to merge it into the workspace commit, *even* if it's not needed as the workspace reference
    /// can connect directly with the *one* workspace base.
    /// This also ensures that there is a workspace merge commit, even if it is none-sensical.
    #[default]
    AlwaysMerge,
    /// Only create a merge commit if a new commit is effectively merged in. This avoids *unnecessary* merge commits,
    /// but also requires support for this when creating commits (which may then have to create a merge-commit themselves).
    // TODO: make this the default
    MergeIfNeeded,
}

/// Decide how a newly created workspace reference should be named.
#[derive(Default, Debug, Clone)]
pub enum WorkspaceReferenceNaming {
    /// Create a default workspace branch
    #[default]
    Default,
    /// Create a workspace with the given name instead.
    Given(gix::refs::FullName),
}

/// Options for [apply()].
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How the branch should be brought into the workspace.
    pub workspace_merge: WorkspaceMerge,
    /// Decide how to deal with conflicts when creating the workspace merge commit to bring in each stack.
    pub on_workspace_conflict: OnWorkspaceMergeConflict,
    /// How the workspace reference should be named should it be created.
    /// The creation is always needed if there are more than one branch applied.
    pub workspace_reference_naming: WorkspaceReferenceNaming,
    /// If not `None`, the applied branch should be merged into the workspace commit at the N'th parent position.
    /// This is useful if the tip of a branch (at a specific position) was unapplied, and a segment within that branch
    /// should now be re-applied, but of course, be placed at the same spot and not end up at the end of the workspace.
    pub order: Option<usize>,
    /// Create new stack id, which by default is a function that generates a new StackId.
    pub new_stack_id: Option<fn(&gix::refs::FullNameRef) -> StackId>,
    /// By default applying branches that are already applied is considered an error. Setting this
    /// to `true` changes that so if we're not in a workspace, applying already applied branches is
    /// allowed.
    ///
    /// The apply follows the normal apply path, as if the branch wasn't applied.
    ///
    /// The use case for this is to go from single branch mode to workspace mode. If we're in SBM
    /// with branch `foo` applied and we apply `foo` with
    /// `allow_applying_already_applied_branch_when_outside_workspace` set to `true`, we'll be put
    /// into a workspace with only `foo` applied, regardless which branches were previously
    /// applied.
    pub allow_applying_already_applied_branch_when_outside_workspace: bool,
}

use anyhow::{Context as _, bail};
use but_core::{
    ObjectStorageExt, RefMetadata, RepositoryExt, extract_remote_name_and_short_name, ref_metadata,
    ref_metadata::{
        Workspace,
        WorkspaceCommitRelation::{Merged, Outside},
    },
};
use but_graph::{
    walk::Overlay,
    workspace::{StackTip, WorkspaceKind},
};
use gix::{
    prelude::ObjectIdExt,
    reference::Category,
    refs::{FullNameRef, Target, transaction::PreviousValue},
};
use tracing::instrument;

use crate::{
    WorkspaceCommit,
    branch::{anon_stacks, ensure_no_missing_stacks},
    commit::merge::Seed,
};

/// Apply `branch` to the given `workspace`, and possibly create the workspace reference in `repo`
/// along with its `meta`-data if it doesn't exist yet.
/// The changed workspace will be checked out.
/// If `branch` is a remote tracking branch, we will instead apply the local tracking branch if it exists or fail otherwise.
/// Otherwise, add it to the existing `workspace`, and update its metadata accordingly.
/// **This means that the contents of `branch` is observable from the new state of `repo`**.
///
/// Note that `workspace` is expected to match the state in `repo` as it's used instead of querying `repo` directly
/// where possible.
///
/// Also note that we will create a managed workspace reference as needed if necessary, and a workspace commit if there is more than
/// one reference in the workspace afterward.
///
/// On `error`, neither `repo` nor `meta` will have been changed, but `repo` may contain in-memory objects.
/// Otherwise, objects will have been persisted, and references and metadata will have been updated.
///
/// Note that when we exit early as the branch is already present, we ignore the `integration_mode` which controls how the workspace
/// merge commit is treated.
///
/// Note that options have no effect if `branch` is already in the workspace, so `apply` is *not* a way
/// to alter certain aspects of the workspace by applying the same branch again.
#[instrument(skip(workspace, repo, meta), err(Debug))]
pub fn apply(
    branch: &FullNameRef,
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
    meta: &mut impl RefMetadata,
    Options {
        workspace_merge: integration_mode,
        on_workspace_conflict,
        workspace_reference_naming,
        order,
        new_stack_id,
        allow_applying_already_applied_branch_when_outside_workspace,
    }: Options,
) -> anyhow::Result<Outcome> {
    let new_stack_id = new_stack_id.unwrap_or(generate_new_stack_id);
    let branch_orig = branch;
    let (
        ws,
        ResolvedBranch {
            branch,
            branch_ref,
            incoming_branch_is_remote_tracking_without_local_tracking,
        },
    ) = match resolve_and_validate(
        branch,
        workspace.clone(),
        repo,
        meta,
        order,
        new_stack_id,
        allow_applying_already_applied_branch_when_outside_workspace,
    )? {
        ControlFlow::Break(outcome) => return Ok(outcome),
        ControlFlow::Continue(resolved) => resolved,
    };
    // In general, we only have to deal with one branch to apply. But when we are on an adhoc workspace,
    // we need to assure both branches go into the existing or the new workspace:
    //  - the current one and the one to apply, if these are different.
    // The returned workspace ref name will be set to the new merge commit, if created, or it may not change
    // at all if the workspace can be created by just setting metadata.
    let (workspace_ref_name_to_update, branches_to_apply) = match ws.kind() {
        WorkspaceKind::Managed { ref_info }
        | WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info, .. } => {
            (ref_info.ref_name.clone(), vec![branch.clone()])
        }
        WorkspaceKind::AdHoc => {
            // We need to switch over to a possibly existing workspace.
            // We know that the current branch is *not* reachable from the workspace or isn't naturally included,
            // so it needs to be added as well.
            let next_ws_ref_name = match workspace_reference_naming {
                WorkspaceReferenceNaming::Default => {
                    gix::refs::FullName::try_from(WORKSPACE_REF_NAME).expect("known statically")
                }
                WorkspaceReferenceNaming::Given(name) => name,
            };
            let mut current_unmanaged_head_branch_name = ws.ref_name().map(|rn| rn.to_owned());
            // HEAD on the reserved workspace ref, in an ad-hoc view, means
            // `gitbutler/workspace` is a plain branch with no managed merge. It cannot be
            // applied as a stack: it is about to become the merge commit. Its content is
            // preserved another way — a declared stack already names it, or, being unnamed, it
            // rides along as an anonymous merge parent — so the reserved ref leaves the apply set.
            let head_is_reserved_ws_ref = current_unmanaged_head_branch_name
                .as_ref()
                .is_some_and(|rn| rn.as_bstr() == WORKSPACE_REF_NAME);
            if head_is_reserved_ws_ref {
                current_unmanaged_head_branch_name.take();
            }
            if let Some(ref current_head_ref) = current_unmanaged_head_branch_name {
                // If our current branch is related to the target, don't add it to the
                // soon-to-be-created workspace.
                // This is a 'trick' to allow callers to prevent 'main' to be added to the workspace automatically
                // even though the new workspace is supposed to have it as target.
                // The target is project-wide, so the current view answers for the next
                // workspace too.
                if ws.is_target_or_its_local_tracking(current_head_ref.as_ref()) {
                    current_unmanaged_head_branch_name.take();
                }
            }

            (
                next_ws_ref_name,
                current_unmanaged_head_branch_name
                    .into_iter()
                    .chain(Some(branch.clone()))
                    .collect(),
            )
        }
    };
    // Whether HEAD already points at the workspace ref, or sits directly on a branch. When it's on a
    // branch we move HEAD onto the workspace ref and rebuild the workspace around the branches we keep.
    let head_ref_name = repo.head_name()?.map(|rn| rn.to_owned());
    let head_on_workspace_ref = head_ref_name
        .as_ref()
        .is_some_and(|head| head.as_bstr() == workspace_ref_name_to_update.as_bstr());

    // First, see if the branches to apply would naturally emerge if they had metadata.
    let (ws_ref_id, ws_ref_exists) = match repo
        .try_find_reference(workspace_ref_name_to_update.as_ref())?
    {
        None => {
            // Pretend to create a workspace reference later at the current AdHoc workspace id
            let tip = ws.tip_commit_id().context(
                    "BUG: how can an empty ad-hoc workspace exist? Should have at least one stack-segment with commit",
                )?;
            (tip, false)
        }
        Some(mut existing_workspace_reference) => {
            let id = existing_workspace_reference.peel_to_id()?;
            (id.detach(), true)
        }
    };

    let mut ws_md = meta.workspace(workspace_ref_name_to_update.as_ref())?;
    // When HEAD is on a branch that's already in the workspace, applying another branch re-roots
    // the workspace around just that branch plus the ones we apply.
    let head_branch_in_workspace = head_ref_name.as_ref().is_some_and(|head| {
        ws_md
            .find_branch(head.as_ref(), StackKind::Applied)
            .is_some()
    });
    restage_metadata_stacks(
        &mut ws_md,
        &ws,
        &branches_to_apply,
        head_ref_name.as_ref(),
        head_branch_in_workspace,
        order,
        new_stack_id,
    );
    let ws_md_retry_base = ws_md.clone();

    let (local_tracking_config_and_ref_info, commit_to_create_branch_at) =
        if incoming_branch_is_remote_tracking_without_local_tracking {
            setup_local_tracking_configuration(repo, branch.as_ref(), branch_orig)?
                .map(|(config, lock, commit)| (Some((config, lock)), Some(commit)))
                .unwrap_or_default()
        } else {
            (None, None)
        };
    let ws_md_override = Some((workspace_ref_name_to_update.clone(), (*ws_md).clone()));
    let branch_mds = branches_to_apply
        .iter()
        .map(|rn| {
            meta.branch(rn.as_ref())
                .map(|md| (rn.to_owned(), (*md).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let overlay = Overlay::default()
        .with_entrypoint(ws_ref_id, Some(workspace_ref_name_to_update.clone()))
        .with_references_if_new(commit_to_create_branch_at.map(|tracking_commit_id| {
            gix::refs::Reference {
                name: branch.to_owned(),
                target: Target::Object(tracking_commit_id),
                peeled: Some(tracking_commit_id),
            }
        }))
        .with_branch_metadata_override(branch_mds)
        .with_workspace_metadata_override(ws_md_override);
    let ws = ws.rederive_with(repo, meta, overlay.clone())?;

    // A branch is visible when it names a segment of the workspace. An advanced branch (its ref
    // moved outside) names none — correctly not visible, so the workspace merge gets rebuilt.
    let all_applied_branches_are_already_visible = branches_to_apply
        .iter()
        .all(|rn| ws.find_branch(rn.as_ref()).is_some());
    let needs_ws_ref_creation = !ws_ref_exists;
    let local_tracking_config_and_ref_info = local_tracking_config_and_ref_info
        .zip(commit_to_create_branch_at.map({
            let branch = branch.clone();
            |commit| (branch, branch_orig, commit.attach(repo))
        }))
        .map(|((config, lock), ref_info)| (config, lock, ref_info));
    let applied_branches = branches_to_apply
        .iter()
        .map(|rn| (*rn).to_owned())
        .collect();
    // Take the no-merge shortcut only when the branches to apply are already visible *and* HEAD is
    // already at the workspace tip - either because it's on the workspace ref, or on a branch that
    // points at the same commit (e.g. a freshly created ad-hoc workspace, or a detached HEAD on the
    // sole stack). If HEAD sits on a different commit we must fall through to the merge path to
    // re-root the workspace around the applied branches, which is what legitimately (re)creates the
    // workspace commit.
    let head_id = repo.head_id().context("BUG: we assume HEAD is born here")?;
    if all_applied_branches_are_already_visible && head_id == ws_ref_id {
        persist_metadata_and_gitconfig(
            meta,
            &branches_to_apply,
            &ws_md,
            local_tracking_config_and_ref_info,
        )?;
        let ws_commit_with_new_message = WorkspaceCommit::from_graph_workspace_and_tree(
            &ws,
            repo,
            head_id.object()?.peel_to_tree()?.id,
        )?;
        let ws_commit_with_new_message = ws_commit_with_new_message.id.detach();
        let (ws, new_head_id) = if (ws_commit_with_new_message != head_id
            && ws.kind().has_managed_commit())
            || needs_workspace_commit_without_remerge(&ws, integration_mode)
        {
            let ws = ws.rederive_with(
                repo,
                meta,
                overlay.with_entrypoint(
                    ws_commit_with_new_message,
                    Some(workspace_ref_name_to_update.clone()),
                ),
            )?;
            (ws, ws_commit_with_new_message)
        } else {
            (ws, ws_ref_id)
        };

        set_head_to_reference(
            repo,
            new_head_id,
            // Point HEAD at the workspace ref whenever it isn't already there (covers creating the
            // ref and switching off a directly-checked-out branch).
            (!head_on_workspace_ref).then_some(workspace_ref_name_to_update.as_ref()),
        )?;
        return Ok(Outcome {
            workspace: ws,
            status: OutcomeStatus::Applied,
            workspace_ref_created: needs_ws_ref_creation,
            workspace_merge: None,
            conflicting_stacks: Vec::new(),
            applied_branches,
        });
    }
    // We will want to merge, but be sure the branch exists, can't apply non-existing.
    if branch_ref.is_none() && !incoming_branch_is_remote_tracking_without_local_tracking {
        bail!(
            "Cannot apply non-existing branch '{branch}'",
            branch = branch.shorten()
        );
    }

    let existing_stacks_superseded_by_branch =
        find_superseded_stacks(branch.as_ref(), &ws, &mut ws_md);
    // At this point, the workspace-metadata already knows the new branch(es), but the workspace itself
    // doesn't see one or more of to-be-applied branches (to become stacks).
    // These are, however, part of the graph by now, and we want to try to create a workspace
    // merge.
    let mut in_memory_repo = repo.clone().for_tree_diffing()?.with_object_memory();
    let merged = match merge_workspace_and_redo(
        ws,
        &mut ws_md,
        &overlay,
        &in_memory_repo,
        meta,
        branch.as_ref(),
        &workspace_ref_name_to_update,
        on_workspace_conflict,
        &existing_stacks_superseded_by_branch,
    )? {
        MergeAttempt::Aborted(outcome) => return Ok(outcome),
        MergeAttempt::Merged(merged) => merged,
    };
    let mut merge_result = merged.merge_result;
    let mut new_head_id = merged.new_head_id;
    let mut conflicting_stacks = merged.conflicting_stacks;
    let mut ws = merged.ws;
    let collect_unapplied_branches = |ws: &but_graph::Workspace| {
        branches_to_apply
            .iter()
            .filter(|rn| !ws.refname_is_segment(rn.as_ref()))
            .collect::<Vec<_>>()
    };
    let unapplied_branches = collect_unapplied_branches(&ws);
    if !unapplied_branches.is_empty() {
        // Now that the merge is done, try to redo the operation one last time with dependent branches instead.
        // Only do that for the still unapplied branches, which should always find some sort of anchor.
        let ws_mut: &mut Workspace = &mut ws_md;
        // Reset to the post-restage metadata, where every branch-to-apply is already a stack. The
        // came-through branches stay stacks as-is (the loop below only re-homes the still-unapplied
        // ones as dependent branches), so re-adding them here would be a no-op.
        *ws_mut = ws_md_retry_base;
        for rn in &unapplied_branches {
            // Here we have to check if the new ref would be able to become its own stack,
            // or if it has to be a dependent branch. Stacks only work if the ref rests on a base
            // outside the workspace, so if we find it in the workspace (in an ambiguous spot) it must be
            // a dependent branch
            if let Some(segment_to_insert_above) = ws
                .commit_graph()
                .commit_by_ref(rn.as_ref())
                .and_then(|on| ws.find_commit(on))
                .filter(|(_, segment)| segment.ref_name() != Some(rn.as_ref()))
                .and_then(|(_, segment)| segment.ref_name())
            {
                match ws_mut.insert_new_segment_above_anchor_if_not_present(
                    rn.as_ref(),
                    segment_to_insert_above,
                ) {
                    None => {
                        // For now bail, until we know it's worth fixing this case automatically.
                        bail!(
                            "Missing reference {segment_to_insert_above} which should be known to workspace metadata to serve as insertion position for {rn}"
                        );
                    }
                    Some(false) => {
                        // The branch already existed, probably as stack, but it didn't come through. Remove it and use the anchor.
                        ws_mut.remove_segment(rn.as_ref());
                        if ws_mut.insert_new_segment_above_anchor_if_not_present(
                            rn.as_ref(),
                            segment_to_insert_above,
                        ) != Some(true)
                        {
                            bail!(
                                "Failed to assure that {rn} is in the workspace as dependent branch after removing it"
                            );
                        }
                    }
                    Some(true) => {}
                }
            } else {
                bail!("Unexpectedly failed to find anchor for {rn} to make it a dependent branch")
            }
        }

        // Redo the merge, with the different stack configuration.
        // Note that this is the exception, typically using stacks will be fine.
        let existing_stacks_superseded_by_branch =
            find_superseded_stacks(branch.as_ref(), &ws, &mut ws_md);
        let merged = match merge_workspace_and_redo(
            ws,
            &mut ws_md,
            &overlay,
            &in_memory_repo,
            meta,
            branch.as_ref(),
            &workspace_ref_name_to_update,
            on_workspace_conflict,
            &existing_stacks_superseded_by_branch,
        )? {
            MergeAttempt::Aborted(outcome) => return Ok(outcome),
            MergeAttempt::Merged(merged) => merged,
        };
        merge_result = merged.merge_result;
        new_head_id = merged.new_head_id;
        conflicting_stacks = merged.conflicting_stacks;
        ws = merged.ws;
        let unapplied_branches = collect_unapplied_branches(&ws);

        if !unapplied_branches.is_empty() {
            bail!(
                "Unexpectedly failed to apply {branches} which is/are still not in the workspace",
                branches = unapplied_branches
                    .iter()
                    .map(|rn| rn.shorten().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    // All work is done, persist and exit.
    // Note that it could be that some stacks aren't merged in,
    // while being present in the workspace metadata.
    // This is OK for us. We also trust that the hero-branch was merged in, no matter what.
    if let Some(storage) = in_memory_repo.objects.take_object_memory() {
        storage.persist(repo)?;
        drop(in_memory_repo);
    }
    but_core::worktree::safe_checkout_from_head(
        new_head_id,
        repo,
        but_core::worktree::checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    persist_metadata_and_gitconfig(
        meta,
        &branches_to_apply,
        &ws_md,
        local_tracking_config_and_ref_info,
    )?;

    set_head_to_reference(
        repo,
        new_head_id,
        // Point HEAD at the workspace ref whenever it isn't already there (covers creating the ref
        // and switching off a directly-checked-out branch).
        (!head_on_workspace_ref).then_some(workspace_ref_name_to_update.as_ref()),
    )?;
    Ok(Outcome {
        workspace: ws,
        status: OutcomeStatus::Applied,
        workspace_ref_created: needs_ws_ref_creation,
        workspace_merge: Some(merge_result),
        conflicting_stacks,
        applied_branches,
    })
}

/// The validated branch to apply: `branch` resolved to its local tracking name when a remote branch
/// was given, its on-disk `branch_ref` (if any), and whether the given branch was a remote-tracking
/// ref with no local tracking ref.
struct ResolvedBranch<'repo> {
    branch: gix::refs::FullName,
    branch_ref: Option<gix::Reference<'repo>>,
    incoming_branch_is_remote_tracking_without_local_tracking: bool,
}

/// Resolve the branch to apply (remote→local tracking name) and run the cheap validations. Returns
/// a ready-to-return [`Outcome`] via `Break` for the two short-circuits — an already-applied branch,
/// or one the workspace encloses but hasn't checked out — and bails on an unborn workspace ref, a
/// workspace commit that isn't at the top, or a ref that is itself a workspace. `Continue` yields the
/// workspace and the resolved branch.
fn resolve_and_validate<'repo>(
    branch: &FullNameRef,
    ws: but_graph::Workspace,
    repo: &'repo gix::Repository,
    meta: &mut impl RefMetadata,
    order: Option<usize>,
    new_stack_id: fn(&FullNameRef) -> StackId,
    allow_applying_already_applied_branch_when_outside_workspace: bool,
) -> anyhow::Result<ControlFlow<Outcome, (but_graph::Workspace, ResolvedBranch<'repo>)>> {
    let (mut branch_ref, mut incoming_branch_is_remote_tracking_without_local_tracking) =
        (try_find_validated_ref(repo, branch, "apply")?, false);
    // The TARGET itself cannot be applied — a workspace cannot contain the thing it integrates
    // into. Its LOCAL tracking branch can (product decision, 2026-07-26): a workspace resting
    // straight on the target has that branch as its one stack, and refusing it left a managed
    // merge commit with no stacks at all.
    if ws.target_ref_name() == Some(branch) {
        bail!("Cannot add the target '{branch}' branch to its own workspace");
    }
    let mut branch = branch.to_owned();
    if branch
        .category()
        .is_some_and(|c| c == Category::RemoteBranch)
    {
        // TODO(gix): we really want to have a function to return the local tracking branch
        //            fix this in other places, too.
        let Some((upstream_branch_name, _remote_name)) =
            repo.upstream_branch_and_remote_for_tracking_branch(branch.as_ref())?
        else {
            // TODO: actually create a local tracking branch with proper configuration.
            bail!("Couldn't find remote refspecs that would match {branch}");
        };
        // Pretend the upstream branch is also the local tracking name.
        incoming_branch_is_remote_tracking_without_local_tracking = true;
        branch = upstream_branch_name;
        branch_ref = try_find_validated_ref(repo, branch.as_ref(), "apply")?;
    }
    let branch_has_applied_metadata =
        branch_has_applied_workspace_metadata(branch.as_ref(), &ws, meta)?;
    let branch_already_applied =
        ws.is_reachable_from_entrypoint(branch.as_ref()) && branch_has_applied_metadata;
    // Applying an already-applied branch is a no-op — unless the caller asked to re-enter the
    // workspace from outside it (HEAD on some other ref), where the apply proceeds so the
    // workspace ref gets checked out again.
    let head_on_managed_workspace_ref = ws.kind().has_managed_ref()
        && repo.head_name()?.as_ref().map(|h| h.as_ref()) == ws.ref_name();
    if branch_already_applied
        && (!allow_applying_already_applied_branch_when_outside_workspace
            || head_on_managed_workspace_ref)
    {
        // When exiting early, don't try to adjust the ws commit.
        return Ok(ControlFlow::Break(Outcome {
            workspace: ws,
            status: OutcomeStatus::AlreadyApplied,
            workspace_ref_created: false,
            workspace_merge: None,
            conflicting_stacks: Vec::new(),
            applied_branches: Vec::new(),
        }));
    }
    if !branch_has_applied_metadata && ws.refname_is_segment(branch.as_ref()) {
        // The workspace encloses the desired branch, but it's not checked out yet.
        return checkout_enclosed_branch(ws, repo, meta, branch.as_ref(), order, new_stack_id)
            .map(ControlFlow::Break);
    }

    if let Some(ws_ref_name) = ws.ref_name()
        && repo.try_find_reference(ws_ref_name)?.is_none()
    {
        // The workspace is the probably ad-hoc, and doesn't exist, *assume* unborn.
        bail!(
            "Cannot create reference on unborn branch '{}'",
            ws_ref_name.shorten()
        );
    }

    crate::branch::ensure_workspace_commit_at_top(&ws, repo)?;

    if meta.workspace_opt(branch.as_ref())?.is_some() {
        bail!(
            "Refusing to apply a reference that already is a workspace: '{}'",
            branch.shorten()
        );
    }
    Ok(ControlFlow::Continue((
        ws,
        ResolvedBranch {
            branch,
            branch_ref,
            incoming_branch_is_remote_tracking_without_local_tracking,
        },
    )))
}

/// Handle the case where the workspace already encloses `branch` but it isn't checked out yet:
/// check out the workspace tip, record the branch as a stack in the workspace metadata, re-derive
/// the workspace, and point HEAD at it. Only reached with the branch's applied metadata missing (the
/// caller guards on that), so the metadata repair is unconditional. Returns the finished [`Outcome`].
fn checkout_enclosed_branch(
    ws: but_graph::Workspace,
    repo: &gix::Repository,
    meta: &mut impl RefMetadata,
    branch: &FullNameRef,
    order: Option<usize>,
    new_stack_id: fn(&FullNameRef) -> StackId,
) -> anyhow::Result<Outcome> {
    let commit_to_checkout = ws
        .tip_commit_id()
        .context("Workspace must point to a commit to check out")?;
    let ws_ref_name = ws.ref_name().map(|rn| rn.to_owned());
    but_core::worktree::safe_checkout_from_head(
        commit_to_checkout,
        repo,
        but_core::worktree::checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    let applied_branches = vec![branch.to_owned()];
    // The applied metadata is missing here, so record the branch as a stack and persist. Scoped so
    // the required-`ws_ref_name` unwrap doesn't shadow the `Option` the re-derive/set-head below need.
    {
        let ws_ref_name = ws_ref_name
            .as_ref()
            .context("Workspace metadata must be available to repair stale applied state")?;
        let mut ws_md = meta.workspace(ws_ref_name.as_ref())?;
        add_branch_as_stack_forcefully(&mut ws_md, branch, order, new_stack_id);
        persist_metadata_and_gitconfig(meta, &applied_branches, &ws_md, None)?;
    }
    let ws = ws.rederive_with(
        repo,
        meta,
        Overlay::default().with_entrypoint(commit_to_checkout, ws_ref_name.clone()),
    )?;
    set_head_to_reference(
        repo,
        commit_to_checkout,
        ws_ref_name.as_ref().map(|rn| rn.as_ref()),
    )?;
    Ok(Outcome {
        workspace: ws,
        status: OutcomeStatus::Applied,
        workspace_ref_created: false,
        workspace_merge: None,
        conflicting_stacks: Vec::new(),
        applied_branches,
    })
}

/// Demote metadata stacks that should no longer sit in the workspace, then force-add the branches
/// being applied. A stack is demoted when it is stale AdHoc metadata (absent from the derived
/// view — AdHoc only, since a managed workspace's metadata is authoritative) or when re-rooting
/// around a checked-out workspace branch keeps only that branch and the ones being applied.
fn restage_metadata_stacks(
    ws_md: &mut ref_metadata::Workspace,
    ws: &but_graph::Workspace,
    branches_to_apply: &[gix::refs::FullName],
    head_ref_name: Option<&gix::refs::FullName>,
    head_branch_in_workspace: bool,
    order: Option<usize>,
    new_stack_id: fn(&FullNameRef) -> StackId,
) {
    // The view, not the pruned display, so a stack hidden by display pruning isn't mistaken for stale.
    let projected_refs = matches!(ws.kind(), WorkspaceKind::AdHoc).then(|| {
        ws.segment_names()
            .map(|rn| rn.as_bstr())
            .collect::<std::collections::HashSet<_>>()
    });
    for stack in &mut ws_md.stacks {
        let dropped_from_projection = projected_refs.as_ref().is_some_and(|projected| {
            !stack
                .branches
                .iter()
                .any(|b| projected.contains(b.ref_name.as_ref().as_bstr()))
        });
        let stack_is_kept = stack.branches.iter().any(|b| {
            branches_to_apply
                .iter()
                .any(|rn| rn.as_ref() == b.ref_name.as_ref())
                || head_ref_name.is_some_and(|head| head.as_ref() == b.ref_name.as_ref())
        });
        if dropped_from_projection || (head_branch_in_workspace && !stack_is_kept) {
            stack.workspacecommit_relation = Outside;
        }
    }
    // RECORD WHAT IS APPLIED (see the crate docs). A stack can be in the workspace without being
    // declared — checked out on `feature`, it stands as the only stack by virtue of the merge
    // alone. Applying a second branch must not drop it, so declare it first and the applied branch
    // after: you were on `feature`, you added `A`, and metadata now says both. Nothing else
    // notices this: the vb-toml write-back used to, by copying the view back into the
    // declaration, which is exactly the authoring we removed.
    //
    // This is NOT the never-strand-the-user rule: it is about the declaration matching the merge,
    // and it would still be needed if a workspace were allowed to be empty.
    // Only where a MERGE defines membership: there a stack is in the workspace because the merge
    // holds it, so an undeclared one is a real omission. An ad-hoc view has no merge and its
    // segments are just what is checked out — declaring those would invent stacks.
    //
    // The target's own branch is exempt: an empty workspace rests directly on it, which makes it
    // LOOK like a stack tip, but the merge holds it as the base — declaring it would sweep
    // `master` into every workspace created on top of it.
    let undeclared_stack_tips: Vec<gix::refs::FullName> = if ws.kind().has_managed_commit() {
        ws.segment_names()
            .filter(|rn| ws.is_stack_tip(rn))
            .filter(|rn| !ws.is_target_or_its_local_tracking(rn))
            .filter(|rn| !ws_md.contains_ref(rn, StackKind::AppliedAndUnapplied))
            .map(ToOwned::to_owned)
            .collect()
    } else {
        Vec::new()
    };
    for rn in undeclared_stack_tips {
        add_branch_as_stack_forcefully(ws_md, rn.as_ref(), None, new_stack_id);
    }
    for rn in branches_to_apply {
        add_branch_as_stack_forcefully(ws_md, rn.as_ref(), order, new_stack_id);
    }
}

/// The workspace produced by a successful [`merge_workspace_and_redo`] pass.
struct MergedWorkspace {
    merge_result: crate::commit::merge::Outcome,
    new_head_id: gix::ObjectId,
    conflicting_stacks: Vec<ConflictingStack>,
    ws: but_graph::Workspace,
}

/// The result of one workspace-merge pass: a ready-to-return conflict abort, or a merge.
enum MergeAttempt {
    /// Conflicts aborted the merge; the `Outcome` is ready to return from [`apply`].
    Aborted(Outcome),
    /// The merge produced a new workspace commit.
    Merged(MergedWorkspace),
}

/// One workspace-merge pass, shared by [`apply`]'s first attempt and its dependent-branch retry:
/// merge the superseded-filtered stacks, bail on missing stacks, then either abort on conflict
/// (per `on_conflict`) or correlate + drop the conflicting stacks and re-derive the workspace around the
/// new merge commit. Extracted so the two passes can't drift. `base_overlay`'s entrypoint and
/// workspace-metadata override are replaced each pass (both keyed on `workspace_ref_name`, so a
/// fresh pass from the base is equivalent to chaining onto the previous one).
#[expect(clippy::too_many_arguments)]
fn merge_workspace_and_redo(
    ws: but_graph::Workspace,
    ws_md: &mut ref_metadata::Workspace,
    base_overlay: &Overlay,
    in_memory_repo: &gix::Repository,
    meta: &mut impl RefMetadata,
    branch: &FullNameRef,
    workspace_ref_name: &gix::refs::FullName,
    on_conflict: OnWorkspaceMergeConflict,
    superseded: &[StackTip],
) -> anyhow::Result<MergeAttempt> {
    // The segment graph answers — already derived from the recorded facts.
    let anon: Vec<(usize, crate::commit::merge::Seed)> = anon_stacks(&ws.stacks).collect();
    let merge_result = WorkspaceCommit::from_new_merge_with_metadata(
        filter_superseded_metadata_stacks(ws_md.stacks.iter(), superseded),
        filter_superseded_anon_stacks(anon.into_iter(), superseded),
        &ws,
        in_memory_repo,
        Some(branch),
    )?;
    ensure_no_missing_stacks(in_memory_repo, &merge_result)?;

    if merge_result.has_conflicts() && on_conflict.should_abort() {
        let conflicting_stacks =
            correlate_conflicting_stacks(ws_md, &merge_result.conflicting_stacks);
        return Ok(MergeAttempt::Aborted(Outcome {
            workspace: ws,
            status: OutcomeStatus::ConflictAborted,
            workspace_ref_created: false,
            workspace_merge: Some(merge_result),
            conflicting_stacks,
            applied_branches: Vec::new(),
        }));
    }

    let new_head_id = merge_result.workspace_commit_id;
    let conflicting_stacks = correlate_conflicting_stacks(ws_md, &merge_result.conflicting_stacks);
    remove_conflicting_stacks_from_workspace(ws_md, &conflicting_stacks);
    let ws_md_override = Some((workspace_ref_name.clone(), (*ws_md).clone()));
    let ws = ws.rederive_with(
        in_memory_repo,
        meta,
        base_overlay
            .clone()
            .with_entrypoint(new_head_id, Some(workspace_ref_name.clone()))
            .with_workspace_metadata_override(ws_md_override),
    )?;
    Ok(MergeAttempt::Merged(MergedWorkspace {
        merge_result,
        new_head_id,
        conflicting_stacks,
        ws,
    }))
}

/// Map conflicting merge tips back to workspace stack metadata.
///
/// Merge conflicts report the tip ref names that could not be merged. This function resolves each
/// reported ref name against the current workspace metadata, including both applied and unapplied
/// stacks, so callers receive the stable stack id together with the stack's current tip branch name.
///
/// Conflicts without a ref name, or whose ref name is no longer present in workspace metadata, are
/// skipped.
fn correlate_conflicting_stacks(
    ws_md: &Workspace,
    conflicts: &[crate::commit::merge::ConflictingStack],
) -> Vec<ConflictingStack> {
    conflicts
        .iter()
        .filter_map(|cs| cs.ref_name.as_ref())
        .filter_map(|conflicting_ref_name| {
            let stack = ws_md.find_stack_with_branch(
                conflicting_ref_name.as_ref(),
                StackKind::AppliedAndUnapplied,
            )?;
            Some(ConflictingStack {
                id: stack.id,
                ref_name: conflicting_ref_name.to_owned(),
            })
        })
        .collect()
}

/// Mark conflicting stacks as outside of the workspace commit.
///
/// This is used when the caller materializes a best-effort merge result despite conflicts. The
/// stack entries and branch metadata remain available, but their workspace relation is changed so
/// the conflicted branches are no longer represented by the checked-out workspace tree.
///
/// Each stack is expected to come from [correlate_conflicting_stacks], so a missing stack indicates
/// a programming error in the caller.
fn remove_conflicting_stacks_from_workspace(
    ws_md: &mut Workspace,
    conflicting_stacks: &[ConflictingStack],
) {
    for conflicting_stack in conflicting_stacks {
        let stack = ws_md
            .stacks
            .iter_mut()
            .find(|s| s.id == conflicting_stack.id)
            .expect("if it was found before it will be found as id");
        // TODO: this might as well be 'Unmerged' to keep them in the workspace, but not let them be merged.
        stack.workspacecommit_relation = Outside;
    }
}

fn branch_has_applied_workspace_metadata(
    branch: &FullNameRef,
    ws: &but_graph::Workspace,
    meta: &impl RefMetadata,
) -> anyhow::Result<bool> {
    let Some(ws_ref_name) = ws.ref_name() else {
        return Ok(true);
    };
    let Some(ws_md) = meta.workspace_opt(ws_ref_name)? else {
        return Ok(true);
    };
    Ok(ws_md.find_branch(branch, StackKind::Applied).is_some()
        || (ws.is_entrypoint() && ws_ref_name == branch))
}

fn filter_superseded_metadata_stacks<'a>(
    stack_iter: impl Iterator<Item = &'a ref_metadata::WorkspaceStack>,
    existing_stacks_superseded_by_branch: &[StackTip],
) -> impl Iterator<Item = &'a ref_metadata::WorkspaceStack> {
    stack_iter.into_iter().filter(|ws_stack| {
        !existing_stacks_superseded_by_branch
            .iter()
            .any(|tip| ws_stack.ref_name() == tip.ref_name.as_ref())
    })
}

fn filter_superseded_anon_stacks(
    tips_iter: impl Iterator<Item = (usize, Seed)>,
    existing_stacks_superseded_by_branch: &[StackTip],
) -> impl Iterator<Item = (usize, Seed)> {
    tips_iter.filter(|(_parent_idx, anon_tip)| {
        !existing_stacks_superseded_by_branch
            .iter()
            .any(|tip| tip.commit_id.is_some_and(|cid| cid == anon_tip.commit_id))
    })
}

/// If the branch to be applied already flows into the workspace, find the stacks it *whose tips* it flows
/// into, and remove these.
/// Note that we don't do that if it doesn't include the entire segment.
/// This check is lenient, and we allow the branch to be applied to not be in the graph yet for any known (or unknown) reason.
/// We keep enough information to identify these superseded stacks and recognise them by
///
/// `branch` is the branch to find in `workspace` and start the traversal from, whereas the existing `workspace` stacks
/// will be used as candidates for being superseded by it.
///
/// `ws_meta` will be adjusted to indicate that the superseded branches are outside the workspace.
#[expect(clippy::indexing_slicing)]
fn find_superseded_stacks(
    branch: &FullNameRef,
    workspace: &but_graph::Workspace,
    ws_meta: &mut ref_metadata::Workspace,
) -> Vec<StackTip> {
    let superseded = workspace
        .stack_tip_segments_below_ref(branch)
        .unwrap_or_else(|| {
            tracing::warn!(
                ?branch,
                "Didn't find branch in graph to do the 'reaches into workspace' check"
            );
            Vec::new()
        });

    let metadata_stacks_to_remove = superseded
        .iter()
        .filter_map(|tip| tip.ref_name.as_ref().map(|rn| rn.as_ref()))
        .filter_map(|superseded_tip_name| {
            ws_meta
                .find_owner_indexes_by_name(superseded_tip_name, StackKind::Applied)
                .map(|t| t.0)
        })
        .collect::<Vec<_>>();
    for superseded_stack_idx in metadata_stacks_to_remove {
        ws_meta.stacks[superseded_stack_idx].workspacecommit_relation = Outside;
    }

    superseded
}

/// Setup `local_tracking_ref` to track `remote_tracking_ref` using the typical pattern, and prepare the configuration file
/// so that it can replace `.git/config` of `repo` when written back, with everything the same but the branch configuration added.
/// We also return the commit at which `local_tracking_ref` should be placed, which is assumed to not exist.
fn setup_local_tracking_configuration(
    repo: &gix::Repository,
    local_tracking_ref: &FullNameRef,
    remote_tracking_ref: &FullNameRef,
) -> anyhow::Result<Option<(gix::config::File, gix::lock::File, gix::ObjectId)>> {
    let remote_tracking_commit_id = repo
        .find_reference(remote_tracking_ref)?
        .peel_to_commit()?
        .id();

    // TODO(gix): Make config refreshes possible, and use the higher level API, and add a way
    //       to only write back what changed and of course to add local sections more obviously.
    //       Make it way easier to work with sections.
    let (mut config, lock) = repo.local_common_config_for_editing()?;
    let mut section =
        config.section_mut_or_create_new("branch", Some(local_tracking_ref.shorten()))?;
    // Only edit the configuration if truly empty, let's not overwrite user data.
    if section.num_values() == 0
        && let Some((remote_name, _short_name)) =
            extract_remote_name_and_short_name(remote_tracking_ref, &repo.remote_names())
    {
        section
            .push(
                gix::config::tree::Branch::REMOTE.name,
                Some(remote_name.as_bytes().as_bstr()),
            )?
            .push(
                gix::config::tree::Branch::MERGE.name,
                Some(local_tracking_ref.as_bstr()),
            )?;
    }
    Ok(Some((config, lock, remote_tracking_commit_id.into())))
}

#[expect(clippy::indexing_slicing)]
fn add_branch_as_stack_forcefully(
    ws_md: &mut Workspace,
    rn: &FullNameRef,
    order: Option<usize>,
    new_stack_id: impl FnOnce(&gix::refs::FullNameRef) -> StackId,
) {
    let (stack_idx, branch_idx) =
        ws_md.add_or_insert_new_stack_if_not_present(rn, order, Merged, new_stack_id);

    let stack = &mut ws_md.stacks[stack_idx];
    if branch_idx != 0 && !stack.is_in_workspace() {
        // For now, just delete the branches that came before it so it's index 0/top most.
        // That way we bring in a new portion of the stack, but discard information like the `archived` flag
        // which probably leads to other issues down the line.
        let mut segment_idx = 0;
        stack.branches.retain(|_| {
            let keep = segment_idx >= branch_idx;
            segment_idx += 1;
            keep
        });
    }
    // Just be sure the new (or old) stack is in the workspace, and we will bring in the whole stack.
    stack.workspacecommit_relation = Merged;
}

fn persist_metadata_and_gitconfig<T: RefMetadata>(
    meta: &mut T,
    branches_to_apply: &[gix::refs::FullName],
    ws_md: &T::Handle<Workspace>,
    config_and_ref: Option<(
        gix::config::File,
        gix::lock::File,
        (gix::refs::FullName, &gix::refs::FullNameRef, gix::Id),
    )>,
) -> anyhow::Result<()> {
    meta.set_workspace(ws_md)?;
    // Always re-obtain the branch information after it was set
    // or stuff will go wrong right now.
    // TODO: remove this note and keep using existing entries once vb.toml is gone.
    for rn in branches_to_apply {
        let mut md = meta.branch(rn.as_ref())?;
        md.update_times(false /* is new ref */);
        meta.set_branch(&md)?;
    }

    if let Some((config, lock, (ref_to_create, remote_tracking_ref, ref_target_id))) =
        config_and_ref
    {
        let repo = ref_target_id.repo;
        repo.write_locked_config(&config, lock)?;

        repo.reference(
            ref_to_create,
            ref_target_id,
            PreviousValue::MustNotExist,
            format!("GitButler creates local tracking for {remote_tracking_ref}"),
        )?;
    }
    Ok(())
}

/// Set `HEAD` to point to `new_ref` if not `None`, but in any case, set what `HEAD` points to to be `new_ref_target`.
fn set_head_to_reference(
    repo: &gix::Repository,
    new_ref_target: gix::ObjectId,
    new_ref: Option<&gix::refs::FullNameRef>,
) -> anyhow::Result<()> {
    use crate::branch::ref_edits;
    let edits = match new_ref {
        None => vec![ref_edits::head_to_commit(
            new_ref_target,
            "GitButler checkout workspace during apply-branch",
        )],
        Some(new_ref) => {
            // This also means we want HEAD to point to it.
            vec![
                ref_edits::head_to_ref(
                    new_ref,
                    "GitButler switch to workspace during apply-branch",
                ),
                ref_edits::ref_to_commit(
                    new_ref.to_owned(),
                    new_ref_target,
                    "created by GitButler during apply-branch",
                ),
            ]
        }
    };
    repo.edit_references(edits)?;
    Ok(())
}

fn needs_workspace_commit_without_remerge(
    ws: &but_graph::Workspace,
    integration_mode: WorkspaceMerge,
) -> bool {
    match integration_mode {
        WorkspaceMerge::AlwaysMerge => match ws.kind() {
            WorkspaceKind::Managed { .. } => false,
            WorkspaceKind::AdHoc => {
                // If it's still ad-hoc, there must be a reason, and we don't try to create a managed commit
                false
            }
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => true,
        },
        WorkspaceMerge::MergeIfNeeded => false,
    }
}

fn generate_new_stack_id(_: &gix::refs::FullNameRef) -> StackId {
    StackId::generate()
}
