use std::borrow::Cow;

use but_core::{ref_metadata::StackId, worktree::checkout::UncommitedWorktreeChanges};

use crate::branch::OnWorkspaceMergeConflict;

/// Returned by [function::apply()].
pub struct Outcome<'graph> {
    /// The newly created graph, if owned, useful to project a workspace and see how the workspace looks like with the branch applied.
    /// If borrowed, the graph already contains the desired branch and nothing had to be applied.
    pub graph: Cow<'graph, but_graph::Graph>,
    /// The name of the branch(es) that were actually applied.
    ///
    /// If a remote tracking branch is given to apply, it will actually apply its local tracking branch, which is created on demand as well.
    /// Further, if there is no target or if the current branch isn't the target branch, then the current branch and the given one
    /// will be applied.
    pub applied_branches: Vec<gix::refs::FullName>,
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
    /// This is `false` if the to be applied branch was already contained in the current workspace.
    pub fn workspace_changed(&self) -> bool {
        matches!(self.graph, Cow::Owned(_))
    }
}

impl<'a> Outcome<'a> {
    /// Convert this instance into a fully-owned one.
    pub fn into_owned(self) -> Outcome<'static> {
        let Outcome {
            graph,
            applied_branches,
            workspace_ref_created,
            workspace_merge,
            conflicting_stack_ids,
        } = self;

        Outcome {
            graph: Cow::Owned(graph.into_owned()),
            applied_branches,
            workspace_ref_created,
            workspace_merge,
            conflicting_stack_ids,
        }
    }
}

impl std::fmt::Debug for Outcome<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Outcome {
            graph: _,
            workspace_ref_created,
            workspace_merge: _,
            conflicting_stack_ids,
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
        if !conflicting_stack_ids.is_empty() {
            f.field("conflicting_stack_ids", conflicting_stack_ids);
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

/// Options for [function::apply()].
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How the branch should be brought into the workspace.
    pub workspace_merge: WorkspaceMerge,
    /// Decide how to deal with conflicts when creating the workspace merge commit to bring in each stack.
    pub on_workspace_conflict: OnWorkspaceMergeConflict,
    /// How the workspace reference should be named should it be created.
    /// The creation is always needed if there are more than one branch applied.
    pub workspace_reference_naming: WorkspaceReferenceNaming,
    /// How the worktree checkout should behave int eh light of uncommitted changes in the worktree.
    pub uncommitted_changes: UncommitedWorktreeChanges,
    /// If not `None`, the applied branch should be merged into the workspace commit at the N'th parent position.
    /// This is useful if the tip of a branch (at a specific position) was unapplied, and a segment within that branch
    /// should now be re-applied, but of course, be placed at the same spot and not end up at the end of the workspace.
    pub order: Option<usize>,
    /// Create new stack id, which by default is a function that generates a new StackId.
    pub new_stack_id: Option<fn(&gix::refs::FullNameRef) -> StackId>,
}

#[allow(clippy::indexing_slicing)]
pub(crate) mod function {
    use std::borrow::Cow;

    use anyhow::{Context as _, bail};
    use but_core::{
        ObjectStorageExt, RefMetadata, RepositoryExt, extract_remote_name, ref_metadata,
        ref_metadata::{
            StackId, StackKind,
            StackKind::AppliedAndUnapplied,
            Workspace,
            WorkspaceCommitRelation::{Merged, Outside},
        },
    };
    use but_graph::{SegmentIndex, init::Overlay, petgraph::Direction, projection::WorkspaceKind};
    use gix::{
        prelude::ObjectIdExt,
        reference::Category,
        refs::{
            FullNameRef, Target,
            transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
        },
    };
    use tracing::instrument;

    use super::{Options, Outcome, WorkspaceMerge, WorkspaceReferenceNaming};
    use crate::{WorkspaceCommit, commit::merge::Tip, ref_info::WorkspaceExt};

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
    pub fn apply<'graph>(
        branch: &FullNameRef,
        workspace: &but_graph::projection::Workspace<'graph>,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            workspace_merge: integration_mode,
            on_workspace_conflict,
            workspace_reference_naming,
            uncommitted_changes,
            order,
            new_stack_id,
        }: Options,
    ) -> anyhow::Result<Outcome<'graph>> {
        let new_stack_id = new_stack_id.unwrap_or(generate_new_stack_id);
        let branch_orig = branch;
        let mut branch_ref = try_find_validated_ref(repo, branch)?;
        if workspace.is_branch_the_target_or_its_local_tracking_branch(branch) {
            bail!("Cannot add the target '{branch}' branch to its own workspace");
        }
        let branch_storage: gix::refs::FullName;
        let mut branch = branch;
        if branch
            .category()
            .is_some_and(|c| c == Category::RemoteBranch)
        {
            // TODO(gix): we really want to have a function to return the local tracking branch
            //            fix this in other places, too.
            let Some((upstream_branch_name, _remote_name)) =
                repo.upstream_branch_and_remote_for_tracking_branch(branch)?
            else {
                // TODO: actually create a local trakcing branch with proper configuration.
                bail!("Couldn't find remote refspecs that would match {branch}");
            };
            branch_storage = upstream_branch_name;
            // Pretend the upstream branch is also the local tracking name.
            branch = branch_storage.as_ref();
            branch_ref = try_find_validated_ref(repo, branch)?;
        }
        let conflicting_stack_ids = Vec::new();
        if workspace.is_reachable_from_entrypoint(branch) {
            let workspace_ref_created = false;
            // When exiting early, don't try to adjust the ws commit.
            return Ok(Outcome {
                graph: Cow::Borrowed(workspace.graph),
                workspace_ref_created,
                workspace_merge: None,
                conflicting_stack_ids,
                applied_branches: vec![branch.to_owned()],
            });
        } else if workspace.refname_is_segment(branch) {
            // This means our workspace encloses the desired branch, but it's not checked out yet.
            let commit_to_checkout = workspace
                .tip_commit()
                .context("Workspace must point to a commit to check out")?;
            let current_head_commit = workspace
                .graph
                .entrypoint_commit()
                .context("The entrypoint must have a commit - it's equal to HEAD, and we skipped unborn earlier")?;
            but_core::worktree::safe_checkout(
                current_head_commit.id,
                commit_to_checkout.id,
                repo,
                but_core::worktree::checkout::Options {
                    uncommitted_changes,
                    skip_head_update: false,
                },
            )?;
            let graph = workspace.graph.redo_traversal_with_overlay(
                repo,
                meta,
                Overlay::default().with_entrypoint(
                    commit_to_checkout.id,
                    workspace.ref_name().map(|rn| rn.to_owned()),
                ),
            )?;

            // When exiting early, don't try to adjust the ws commit.
            return Ok(Outcome {
                graph: Cow::Owned(graph),
                workspace_ref_created: false,
                workspace_merge: None,
                conflicting_stack_ids,
                applied_branches: vec![branch.to_owned()],
            });
        };

        if let Some(ws_ref_name) = workspace.ref_name()
            && repo.try_find_reference(ws_ref_name)?.is_none()
        {
            // The workspace is the probably ad-hoc, and doesn't exist, *assume* unborn.
            bail!(
                "Cannot create reference on unborn branch '{}'",
                ws_ref_name.shorten()
            );
        }

        if workspace.has_workspace_commit_in_ancestry(repo) {
            bail!("Refusing to work on workspace whose workspace commit isn't at the top");
        }

        if meta.workspace_opt(branch)?.is_some() {
            bail!(
                "Refusing to apply a reference that already is a workspace: '{}'",
                branch.shorten()
            );
        }
        // In general, we only have to deal with one branch to apply. But when we are on an adhoc workspace,
        // we need to assure both branches go into the existing or the new workspace:
        //  - the current one and the one to apply, if these are different.
        // The returned workspace ref name will be set to the new merge commit, if created, or it may not change
        // at all if the workspace can be created by just setting metadata.
        let (workspace_ref_name_to_update, branches_to_apply) = match &workspace.kind {
            WorkspaceKind::Managed { ref_info }
            | WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
                (ref_info.ref_name.clone(), vec![branch])
            }
            WorkspaceKind::AdHoc => {
                // We need to switch over to a possibly existing workspace.
                // We know that the current branch is *not* reachable from the workspace or isn't naturally included,
                // so it needs to be added as well.
                let next_ws_ref_name = match workspace_reference_naming {
                    WorkspaceReferenceNaming::Default => {
                        gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")
                            .expect("known statically")
                    }
                    WorkspaceReferenceNaming::Given(name) => name,
                };
                let mut current_unmanaged_head_branch_name = workspace.ref_name();
                if let Some(current_head_ref) = current_unmanaged_head_branch_name
                    && let Some(next_ws_md) = meta.workspace_opt(next_ws_ref_name.as_ref())?
                {
                    // If our current branch is related to the next workspace's target, don't add it to the
                    // soon-to-be-created workspace.
                    // This is a 'trick' to allow callers to prevent 'main' to be added to the workspace automatically
                    // even though the new workspace is supposed to have it as target.
                    if next_ws_md
                        .is_branch_the_target_or_its_local_tracking_branch(current_head_ref, repo)?
                    {
                        current_unmanaged_head_branch_name.take();
                    }
                }

                (
                    next_ws_ref_name,
                    current_unmanaged_head_branch_name
                        .into_iter()
                        .chain(Some(branch))
                        .collect(),
                )
            }
        };

        // First, see if the branches to apply would naturally emerge if they had metadata.
        let (ws_ref_id, ws_ref_exists) = match repo
            .try_find_reference(workspace_ref_name_to_update.as_ref())?
        {
            None => {
                // Pretend to create a workspace reference later at the current AdHoc workspace id
                let tip = workspace.tip_commit().map(|c| c.id).context(
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
        let ws_md_orig = ws_md.clone();
        {
            let ws_mut: &mut Workspace = &mut ws_md;
            for rn in &branches_to_apply {
                add_branch_as_stack_forcefully(ws_mut, rn, order, new_stack_id);
            }
        }

        let incoming_branch_is_remote_tracking_without_local_tracking =
            !std::ptr::eq(branch_orig, branch);
        let (local_tracking_config_and_ref_info, commit_to_create_branch_at) =
            if incoming_branch_is_remote_tracking_without_local_tracking {
                setup_local_tracking_configuration(repo, branch, branch_orig)?
                    .map(|(config, commit)| (Some(config), Some(commit)))
                    .unwrap_or_default()
            } else {
                (None, None)
            };
        let ws_md_override = Some((workspace_ref_name_to_update.clone(), (*ws_md).clone()));
        let branch_mds = branches_to_apply
            .iter()
            .copied()
            .map(|rn| meta.branch(rn).map(|md| (rn.to_owned(), (*md).clone())))
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
        let graph = workspace
            .graph
            .redo_traversal_with_overlay(repo, meta, overlay.clone())?;

        let workspace = graph.to_workspace()?;
        let all_applied_branches_are_already_visible = branches_to_apply.iter().all(|rn| {
            workspace
                .find_segment_and_stack_by_refname(rn)
                .is_some_and(|(_stack, segment)| !segment.is_projected_from_outside(&graph))
        });
        let needs_ws_ref_creation = !ws_ref_exists;
        let local_tracking_config_and_ref_info = local_tracking_config_and_ref_info.zip(
            commit_to_create_branch_at.map(|commit| (branch, branch_orig, commit.attach(repo))),
        );
        let applied_branches = branches_to_apply
            .iter()
            .map(|rn| (*rn).to_owned())
            .collect();
        if all_applied_branches_are_already_visible {
            let head_id = repo.head_id().context("BUG: we assume HEAD is born here")?;
            if head_id != ws_ref_id {
                bail!(
                    "Sanity check failed: we assume HEAD already points to where it must, but it really doesn't: {head_id} != {ws_ref_id}"
                );
            }
            persist_metadata_and_gitconfig(
                meta,
                &branches_to_apply,
                &ws_md,
                local_tracking_config_and_ref_info,
            )?;
            let ws_commit_with_new_message = WorkspaceCommit::from_graph_workspace_and_tree(
                &workspace,
                repo,
                head_id.object()?.peel_to_tree()?.id,
            )?;
            let ws_commit_with_new_message = ws_commit_with_new_message.id.detach();
            let (graph, new_head_id) = if (ws_commit_with_new_message != head_id
                && workspace.kind.has_managed_commit())
                || needs_workspace_commit_without_remerge(&workspace, integration_mode)
            {
                let graph = graph.redo_traversal_with_overlay(
                    repo,
                    meta,
                    overlay.with_entrypoint(
                        ws_commit_with_new_message,
                        Some(workspace_ref_name_to_update.clone()),
                    ),
                )?;
                (graph, ws_commit_with_new_message)
            } else {
                (graph, ws_ref_id)
            };

            set_head_to_reference(
                repo,
                new_head_id,
                (!ws_ref_exists).then_some(workspace_ref_name_to_update.as_ref()),
            )?;
            return Ok(Outcome {
                graph: Cow::Owned(graph),
                workspace_ref_created: needs_ws_ref_creation,
                workspace_merge: None,
                conflicting_stack_ids,
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
            find_superseded_stacks(branch, &workspace, &mut ws_md);
        // At this point, the workspace-metadata already knows the new branch(es), but the workspace itself
        // doesn't see one or more of to-be-applied branches (to become stacks).
        // These are, however, part of the graph by now, and we want to try to create a workspace
        // merge.
        let mut in_memory_repo = repo.clone().for_tree_diffing()?.with_object_memory();
        let mut merge_result = WorkspaceCommit::from_new_merge_with_metadata(
            filter_superseded_metadata_stacks(
                ws_md.stacks.iter().filter(|s| s.is_in_workspace()),
                &existing_stacks_superseded_by_branch,
            ),
            filter_superseded_anon_stacks(
                anon_stacks(&workspace.stacks),
                &existing_stacks_superseded_by_branch,
            ),
            &graph,
            &in_memory_repo,
            Some(branch),
        )?;
        ensure_no_missing_stacks(&merge_result)?;
        drop(existing_stacks_superseded_by_branch);

        if merge_result.has_conflicts() && on_workspace_conflict.should_abort() {
            let conflicting_stack_ids =
                correlate_conflicting_stack_ids(&ws_md, &merge_result.conflicting_stacks);
            return Ok(Outcome {
                graph: Cow::Owned(graph),
                workspace_ref_created: needs_ws_ref_creation,
                workspace_merge: Some(merge_result),
                conflicting_stack_ids,
                applied_branches,
            });
        }

        let prev_head_id = graph
            .entrypoint_commit()
            .context("BUG: how is it possible that there is no head commit?")?
            .id;
        let mut new_head_id = merge_result.workspace_commit_id;
        let mut conflicting_stack_ids = correlate_conflicting_stack_ids_and_remove_from_workspace(
            &mut ws_md,
            &merge_result.conflicting_stacks,
        );
        let ws_md_override = Some((workspace_ref_name_to_update.clone(), (*ws_md).clone()));
        let overlay = overlay
            .with_entrypoint(new_head_id, Some(workspace_ref_name_to_update.clone()))
            .with_workspace_metadata_override(ws_md_override);
        let mut graph =
            graph.redo_traversal_with_overlay(&in_memory_repo, meta, overlay.clone())?;

        let workspace = graph.to_workspace()?;
        let collect_unapplied_branches = |workspace: &but_graph::projection::Workspace| {
            branches_to_apply
                .iter()
                .filter(|rn| !workspace.refname_is_segment(rn))
                .collect::<Vec<_>>()
        };
        let unapplied_branches = collect_unapplied_branches(&workspace);
        if !unapplied_branches.is_empty() {
            // Now that the merge is done, try to redo the operation one last time with dependent branches instead.
            // Only do that for the still unapplied branches, which should always find some sort of anchor.
            let ws_mut: &mut Workspace = &mut ws_md;
            *ws_mut = ws_md_orig;
            for branch_to_add in branches_to_apply
                .iter()
                .filter(|rn| !unapplied_branches.contains(rn))
            {
                add_branch_as_stack_forcefully(ws_mut, branch_to_add, order, new_stack_id);
            }
            for rn in &unapplied_branches {
                // Here we have to check if the new ref would be able to become its own stack,
                // or if it has to be a dependent branch. Stacks only work if the ref rests on a base
                // outside the workspace, so if we find it in the workspace (in an ambiguous spot) it must be
                // a dependent branch
                if let Some(segment_to_insert_above) =
                    workspace
                        .stacks
                        .iter()
                        .flat_map(|stack| stack.segments.iter())
                        .find_map(|segment| {
                            segment.commits.iter().flat_map(|c| c.ref_iter()).find_map(
                                |ambiguous_rn| {
                                    (ambiguous_rn == **rn)
                                        .then_some(segment.ref_name())
                                        .flatten()
                                },
                            )
                        })
                {
                    match ws_mut
                        .insert_new_segment_above_anchor_if_not_present(rn, segment_to_insert_above)
                    {
                        None => {
                            // For now bail, until we know it's worth fixing this case automatically.
                            bail!(
                                "Missing reference {segment_to_insert_above} which should be known to workspace metadata to serve as insertion position for {rn}"
                            );
                        }
                        Some(false) => {
                            // The branch already existed, probably as stack, but it didn't come through. Remove it and use the anchor.
                            ws_mut.remove_segment(rn);
                            if ws_mut.insert_new_segment_above_anchor_if_not_present(
                                rn,
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
                    bail!(
                        "Unexpectedly failed to find anchor for {rn} to make it a dependent branch"
                    )
                }
            }

            // Redo the merge, with the different stack configuration.
            // Note that this is the exception, typically using stacks will be fine.
            let existing_stacks_superseded_by_branch =
                find_superseded_stacks(branch, &workspace, &mut ws_md);
            merge_result = WorkspaceCommit::from_new_merge_with_metadata(
                filter_superseded_metadata_stacks(
                    ws_md.stacks.iter().filter(|s| s.is_in_workspace()),
                    &existing_stacks_superseded_by_branch,
                ),
                filter_superseded_anon_stacks(
                    anon_stacks(&workspace.stacks),
                    &existing_stacks_superseded_by_branch,
                ),
                &graph,
                &in_memory_repo,
                Some(branch),
            )?;
            ensure_no_missing_stacks(&merge_result)?;

            if merge_result.has_conflicts() && on_workspace_conflict.should_abort() {
                let conflicting_stack_ids =
                    correlate_conflicting_stack_ids(&ws_md, &merge_result.conflicting_stacks);
                return Ok(Outcome {
                    graph: Cow::Owned(graph),
                    workspace_ref_created: needs_ws_ref_creation,
                    workspace_merge: Some(merge_result),
                    conflicting_stack_ids,
                    applied_branches,
                });
            }
            new_head_id = merge_result.workspace_commit_id;
            conflicting_stack_ids = correlate_conflicting_stack_ids_and_remove_from_workspace(
                &mut ws_md,
                &merge_result.conflicting_stacks,
            );
            let ws_md_override = Some((workspace_ref_name_to_update.clone(), (*ws_md).clone()));
            graph = graph.redo_traversal_with_overlay(
                &in_memory_repo,
                meta,
                overlay
                    .with_entrypoint(new_head_id, Some(workspace_ref_name_to_update.clone()))
                    .with_workspace_metadata_override(ws_md_override),
            )?;
            let workspace = graph.to_workspace()?;
            let unapplied_branches = collect_unapplied_branches(&workspace);

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
        but_core::worktree::safe_checkout(
            prev_head_id,
            new_head_id,
            repo,
            but_core::worktree::checkout::Options {
                uncommitted_changes,
                skip_head_update: true,
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
            (!ws_ref_exists).then_some(workspace_ref_name_to_update.as_ref()),
        )?;
        Ok(Outcome {
            graph: Cow::Owned(graph),
            workspace_ref_created: needs_ws_ref_creation,
            workspace_merge: Some(merge_result),
            conflicting_stack_ids,
            applied_branches,
        })
    }

    fn anon_stacks(stacks: &[but_graph::projection::Stack]) -> impl Iterator<Item = (usize, Tip)> {
        stacks.iter().enumerate().filter_map(|(idx, s)| {
            if s.ref_name().is_none() {
                s.tip_skip_empty().and_then(|cid| {
                    s.segments.first().map(|s| {
                        (
                            idx,
                            Tip {
                                name: None,
                                commit_id: cid,
                                segment_idx: s.id,
                            },
                        )
                    })
                })
            } else {
                None
            }
        })
    }

    fn filter_superseded_metadata_stacks<'a>(
        stack_iter: impl Iterator<Item = &'a ref_metadata::WorkspaceStack>,
        existing_stacks_superseded_by_branch: &[(
            SegmentIndex,
            Option<gix::refs::FullName>,
            Option<gix::ObjectId>,
        )],
    ) -> impl Iterator<Item = &'a ref_metadata::WorkspaceStack> {
        stack_iter.into_iter().filter(|ws_stack| {
            !existing_stacks_superseded_by_branch
                .iter()
                .any(|(_sidx, ref_name, _cid)| ws_stack.ref_name() == ref_name.as_ref())
        })
    }

    fn filter_superseded_anon_stacks(
        tips_iter: impl Iterator<Item = (usize, Tip)>,
        existing_stacks_superseded_by_branch: &[(
            SegmentIndex,
            Option<gix::refs::FullName>,
            Option<gix::ObjectId>,
        )],
    ) -> impl Iterator<Item = (usize, Tip)> {
        tips_iter.filter(|(_parent_idx, anon_tip)| {
            !existing_stacks_superseded_by_branch
                .iter()
                .any(|(sidx, _ref_name, cid)| {
                    anon_tip.segment_idx == *sidx
                        || cid.is_some_and(|cid| cid == anon_tip.commit_id)
                })
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
    fn find_superseded_stacks(
        branch: &FullNameRef,
        workspace: &but_graph::projection::Workspace,
        ws_meta: &mut ref_metadata::Workspace,
    ) -> Vec<(
        SegmentIndex,
        Option<gix::refs::FullName>,
        Option<gix::ObjectId>,
    )> {
        let graph = workspace.graph;
        let superseded = if let Some(branch_segment) = graph.named_segment_by_ref_name(branch) {
            // At this stage we know first segment isn't in the workspace, so exclude it.
            let _tip_commit_ids_and_sidx: Vec<_> = workspace
                .stacks
                .iter()
                .filter_map(|stack| {
                    stack
                        .segments
                        .first()
                        .and_then(|s| s.commits.first().map(|c| (c.id, s.id)))
                })
                .collect();
            let mut superseded = Vec::new();
            graph.visit_all_segments_excluding_start_until(
                branch_segment.id,
                Direction::Outgoing,
                |segment| {
                    let prune = _tip_commit_ids_and_sidx.iter().any(|(cid, sidx)| {
                        segment.id == *sidx || segment.commits.first().is_some_and(|c| c.id == *cid)
                    });
                    if prune {
                        superseded.push((
                            segment.id,
                            segment.ref_name().map(|rn| rn.to_owned()),
                            segment.commits.first().map(|c| c.id),
                        ));
                    }
                    prune
                },
            );
            superseded
        } else {
            tracing::warn!(
                ?branch,
                "Didn't find branch in graph to do the 'reaches into workspace' check"
            );
            Vec::new()
        };

        let metadata_stacks_to_remove = superseded
            .iter()
            .filter_map(|t| t.1.as_ref().map(|rn| rn.as_ref()))
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
    ) -> anyhow::Result<Option<(gix::config::File<'static>, gix::ObjectId)>> {
        let remote_tracking_commit_id = repo
            .find_reference(remote_tracking_ref)?
            .peel_to_commit()?
            .id();

        // TODO(gix): Make config refreshes possible, and use the higher level API, and add a way
        //       to only write back what changed and of course to add local sections more obviously.
        //       Make it way easier to work with sections.
        let mut config = repo.local_common_config_for_editing()?;
        let mut section =
            config.section_mut_or_create_new("branch", Some(local_tracking_ref.shorten()))?;
        // Only edit the configuration if truly empty, let's not overwrite user data.
        if section.num_values() == 0
            && let Some(remote_name) =
                extract_remote_name(remote_tracking_ref, &repo.remote_names())
        {
            section
                .push(
                    gix::config::tree::Branch::REMOTE.name.try_into()?,
                    Some(remote_name.as_str().into()),
                )
                .push(
                    gix::config::tree::Branch::MERGE.name.try_into()?,
                    Some(local_tracking_ref.as_bstr()),
                );
        }
        Ok(Some((config, remote_tracking_commit_id.into())))
    }

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

    fn correlate_conflicting_stack_ids(
        ws: &Workspace,
        conflicts: &[crate::commit::merge::ConflictingStack],
    ) -> Vec<StackId> {
        conflicts
            .iter()
            .filter_map(|cs| cs.ref_name.as_ref())
            .filter_map(|ref_name| {
                ws.find_stack_with_branch(ref_name.as_ref(), AppliedAndUnapplied)
                    .map(|stack| stack.id)
            })
            .collect()
    }

    /// Note that we chose to put it outside the workspace, instead of just leaving it unmerged.
    fn correlate_conflicting_stack_ids_and_remove_from_workspace(
        ws: &mut Workspace,
        conflicts: &[crate::commit::merge::ConflictingStack],
    ) -> Vec<StackId> {
        let conflicting_stack_ids = correlate_conflicting_stack_ids(ws, conflicts);
        for conflicting_id in &conflicting_stack_ids {
            let stack = ws
                .stacks
                .iter_mut()
                .find(|s| s.id == *conflicting_id)
                .expect("if it was found before it will be found as id");
            // TODO: this might as well be 'Unmerged' to keep them in the workspace, but not let them be merged.
            stack.workspacecommit_relation = Outside;
        }
        conflicting_stack_ids
    }

    fn persist_metadata_and_gitconfig<T: RefMetadata>(
        meta: &mut T,
        branches_to_apply: &Vec<&FullNameRef>,
        ws_md: &T::Handle<Workspace>,
        config_and_ref: Option<(gix::config::File, (&FullNameRef, &FullNameRef, gix::Id))>,
    ) -> anyhow::Result<()> {
        meta.set_workspace(ws_md)?;
        // Always re-obtain the branch information after it was set
        // or stuff will go wrong right now.
        // TODO: remove this note and keep using existing handles once vb.toml is gone.
        for rn in branches_to_apply {
            let mut md = meta.branch(rn)?;
            md.update_times(false /* is new ref */);
            meta.set_branch(&md)?;
        }

        if let Some((config, (ref_to_create, remote_tracking_ref, ref_target_id))) = config_and_ref
        {
            let repo = ref_target_id.repo;
            repo.write_local_common_config(&config)?;

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
        let edits = match new_ref {
            None => {
                let head_message = "GitButler checkout workspace during apply-branch".into();
                vec![RefEdit {
                    change: Change::Update {
                        log: LogChange {
                            mode: RefLog::AndReference,
                            force_create_reflog: false,
                            message: head_message,
                        },
                        expected: PreviousValue::Any,
                        new: Target::Object(new_ref_target),
                    },
                    name: "HEAD".try_into().expect("well-formed root ref"),
                    deref: true,
                }]
            }
            Some(new_ref) => {
                // This also means we want HEAD to point to it.
                let head_message = "GitButler switch to workspace during apply-branch".into();
                vec![
                    RefEdit {
                        change: Change::Update {
                            log: LogChange {
                                mode: RefLog::AndReference,
                                force_create_reflog: false,
                                message: head_message,
                            },
                            expected: PreviousValue::Any,
                            new: Target::Symbolic(new_ref.to_owned()),
                        },
                        name: "HEAD".try_into().expect("well-formed root ref"),
                        deref: false,
                    },
                    RefEdit {
                        change: Change::Update {
                            log: LogChange {
                                mode: RefLog::AndReference,
                                force_create_reflog: false,
                                message: "created by GitButler during apply-branch".into(),
                            },
                            expected: PreviousValue::MustNotExist,
                            new: Target::Object(new_ref_target),
                        },
                        name: new_ref.to_owned(),
                        deref: false,
                    },
                ]
            }
        };
        repo.edit_references(edits)?;
        Ok(())
    }

    fn needs_workspace_commit_without_remerge(
        ws: &but_graph::projection::Workspace<'_>,
        integration_mode: WorkspaceMerge,
    ) -> bool {
        match integration_mode {
            WorkspaceMerge::AlwaysMerge => match ws.kind {
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

    fn ensure_no_missing_stacks(merge: &crate::commit::merge::Outcome) -> anyhow::Result<()> {
        if merge.missing_stacks.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Somehow some of the new stacks weren't part of the graph: {:#?}",
                merge.missing_stacks
            ))
        }
    }

    fn try_find_validated_ref<'repo>(
        repo: &'repo gix::Repository,
        branch: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<gix::Reference<'repo>>> {
        let branch_ref = repo.try_find_reference(branch)?;
        if branch_ref
            .as_ref()
            .is_some_and(|r| matches!(r.target(), gix::refs::TargetRef::Symbolic(_)))
        {
            bail!(
                "Refusing to apply symbolic ref '{}' due to potential ambiguity",
                branch.shorten()
            );
        }
        Ok(branch_ref)
    }

    fn generate_new_stack_id(_: &gix::refs::FullNameRef) -> StackId {
        StackId::generate()
    }
}
