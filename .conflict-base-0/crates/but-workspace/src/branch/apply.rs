use crate::branch::OnWorkspaceMergeConflict;
use crate::branch::checkout::UncommitedWorktreeChanges;
use std::borrow::Cow;

/// Returned by [function::apply()].
pub struct Outcome<'graph> {
    /// The newly created graph, if owned, useful to project a workspace and see how the workspace looks like with the branch applied.
    /// If borrowed, the graph already contains the desired branch and nothing had to be applied.
    pub graph: Cow<'graph, but_graph::Graph>,
    /// `true` if we created the given workspace ref as it didn't exist yet.
    pub workspace_ref_created: bool,
    /// If not `None`, an actual merge was attempted, but depending on [the settings](OnWorkspaceMergeConflict), this was persisted or not.
    pub workspace_merge: Option<crate::commit::merge::Outcome>,
}

impl Outcome<'_> {
    /// Return `true` if a new graph traversal was performed, which always is a sign for an operation which changed the workspace.
    /// This is `false` if the to be applied branch was already contained in the current workspace.
    pub fn workspace_changed(&self) -> bool {
        matches!(self.graph, Cow::Owned(_))
    }
}

impl std::fmt::Debug for Outcome<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outcome")
            .field("workspace_changed", &self.workspace_changed())
            .field("workspace_ref_created", &self.workspace_ref_created)
            .finish()
    }
}

/// How the newly applied branch should be integrated into the workspace.
#[derive(Default, Debug, Copy, Clone)]
pub enum IntegrationMode {
    /// Do nothing but to merge it into the workspace commit, *even* if it's not needed as the workspace reference
    /// can connect directly with the *one* workspace base.
    /// This also ensures that there is a workspace merge commit.
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
    /// how the branch should be brought into the workspace.
    pub integration_mode: IntegrationMode,
    /// Decide how to deal with conflicts when creating the workspace merge commit to bring in each stack.
    pub on_workspace_conflict: OnWorkspaceMergeConflict,
    /// How the workspace reference should be named should it be created.
    /// The creation is always needed if there are more than one branch applied.
    pub workspace_reference_naming: WorkspaceReferenceNaming,
    /// How the worktree checkout should behave int eh light of uncommitted changes in the worktree.
    pub uncommitted_changes: UncommitedWorktreeChanges,
    /// If not `None`, the applied branch should be merged into the workspace commit at the N'th parent position.
    /// This is useful if the tip of a branc (at a specific position) was unapplied, and a segment within that branch
    /// should now be re-applied, but of course, be placed at the same spot and not end up at the end of the workspace.
    pub order: Option<usize>,
}

pub(crate) mod function {
    use super::{IntegrationMode, Options, Outcome, WorkspaceReferenceNaming};
    use crate::WorkspaceCommit;
    use crate::branch::checkout;
    use crate::ext::ObjectStorageExt;
    use crate::ref_info::WorkspaceExt;
    use anyhow::{Context, bail};
    use but_core::RefMetadata;
    use but_core::ref_metadata::Workspace;
    use but_graph::init::Overlay;
    use but_graph::projection::WorkspaceKind;
    use gitbutler_oxidize::GixRepositoryExt;
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
    use gix::refs::{FullNameRef, Target};
    use std::borrow::Cow;
    use tracing::instrument;

    /// Apply `branch` to the given `workspace`, and possibly create the workspace reference in `repo`
    /// along with its `meta`-data if it doesn't exist yet.
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
    #[instrument(level = tracing::Level::DEBUG, skip(workspace, repo, meta), err(Debug))]
    pub fn apply<'graph>(
        branch: &gix::refs::FullNameRef,
        workspace: &but_graph::projection::Workspace<'graph>,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            integration_mode,
            on_workspace_conflict,
            workspace_reference_naming,
            uncommitted_changes,
            order,
        }: Options,
    ) -> anyhow::Result<Outcome<'graph>> {
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
        if workspace.is_branch_the_target_or_its_local_tracking_branch(branch) {
            bail!("Cannot add the target '{branch}' branch to its own workspace");
        }
        if workspace.is_reachable_from_entrypoint(branch) {
            let workspace_ref_created = false;
            // When exiting early, don't try to adjust the ws commit.
            return Ok(Outcome {
                graph: Cow::Borrowed(workspace.graph),
                workspace_ref_created,
                workspace_merge: None,
            });
        } else if workspace.refname_is_segment(branch) {
            // This means our workspace encloses the desired branch, but it's not checked out yet.
            let commit_to_checkout = workspace
                .tip_commit()
                .context("Workspace must point to a commit to check out")?;
            let current_head_commit = workspace
                .graph.entrypoint_commit()
                .context("The entrypoint must have a commit - it's equal to HEAD, and we skipped unborn earlier")?;
            crate::branch::safe_checkout(
                current_head_commit.id,
                commit_to_checkout.id,
                repo,
                checkout::Options {
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
            WorkspaceKind::Managed { ref_name }
            | WorkspaceKind::ManagedMissingWorkspaceCommit { ref_name } => {
                (ref_name.clone(), vec![branch])
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
                let tip = workspace.tip_commit().map(|c| c.id)
                    .context("BUG: how can an empty ad-hoc workspace exist? Should have at least one stack-segment with commit")?;
                (tip, false)
            }
            Some(mut existing_workspace_reference) => {
                let id = existing_workspace_reference.peel_to_id()?;
                (id.detach(), true)
            }
        };

        let mut ws_md = meta.workspace(workspace_ref_name_to_update.as_ref())?;
        {
            let ws_mut: &mut Workspace = &mut ws_md;
            for rn in &branches_to_apply {
                ws_mut.add_or_insert_new_stack_if_not_present(rn, order);
            }
        }
        let ws_md_override = Some((workspace_ref_name_to_update.clone(), (*ws_md).clone()));
        let branch_mds = branches_to_apply
            .iter()
            .copied()
            .map(|rn| meta.branch(rn).map(|md| (rn.to_owned(), (*md).clone())))
            .collect::<Result<Vec<_>, _>>()?;

        let overlay = Overlay::default()
            .with_entrypoint(ws_ref_id, Some(workspace_ref_name_to_update.clone()))
            .with_branch_metadata_override(branch_mds)
            .with_workspace_metadata_override(ws_md_override);
        let graph = workspace
            .graph
            .redo_traversal_with_overlay(repo, meta, overlay.clone())?;

        let workspace = graph.to_workspace()?;
        let all_applied_branches_are_already_visible = branches_to_apply
            .iter()
            .all(|rn| workspace.refname_is_segment(rn));
        let needs_ws_ref_creation = !ws_ref_exists;
        if all_applied_branches_are_already_visible {
            let head_id = repo.head_id().context("BUG: we assume HEAD is born here")?;
            if head_id != ws_ref_id {
                bail!(
                    "Sanity check failed: we assume HEAD already points to where it must, but it really doesn't: {head_id} != {ws_ref_id}"
                );
            }
            persist_metadata(meta, &branches_to_apply, &ws_md)?;
            let ws_commit_with_new_message = WorkspaceCommit::from_graph_workspace(
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
            });
        }
        // We will want to merge, but be sure the branch exists, can't apply non-existing.
        if branch_ref.is_none() {
            bail!(
                "Cannot apply non-existing branch '{branch}'",
                branch = branch.shorten()
            );
        }
        // At this point, the workspace-metadata already knows the new branch(es), but the workspace itself
        // doesn't see one or more of to-be-applied branches (to become stacks).
        // These are, however, part of the graph by now, and we want to try to create a workspace
        // merge.
        let mut in_memory_repo = repo.clone().for_tree_diffing()?.with_object_memory();
        let merge_result = WorkspaceCommit::from_new_merge_with_metadata(
            &ws_md.stacks,
            workspace.graph,
            &in_memory_repo,
            Some(branch),
        )?;

        if merge_result.has_conflicts() && on_workspace_conflict.should_abort() {
            return Ok(Outcome {
                graph: Cow::Owned(graph),
                workspace_ref_created: needs_ws_ref_creation,
                workspace_merge: Some(merge_result),
            });
        }

        let prev_head_id = graph
            .entrypoint_commit()
            .context("BUG: how is it possible that there is no head commit?")?
            .id;
        let mut new_head_id = merge_result.workspace_commit_id;
        let overlay =
            overlay.with_entrypoint(new_head_id, Some(workspace_ref_name_to_update.clone()));
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
            for branch_to_remove in &unapplied_branches {
                ws_mut.remove_segment(branch_to_remove);
            }
            for rn in &unapplied_branches {
                // Here we have to check if the new ref would be able to become its own stack,
                // or if it has to be a dependent branch. Stacks only work if the ref rests on a base
                // outside the workspace, so if we find it in the workspace (in an ambiguous spot) it must be
                // a dependent branch
                if let Some(segment_to_insert_above) = workspace
                    .stacks
                    .iter()
                    .flat_map(|stack| stack.segments.iter())
                    .find_map(|segment| {
                        segment.commits.iter().flat_map(|c| c.refs.iter()).find_map(
                            |ambiguous_rn| {
                                (ambiguous_rn.as_ref() == **rn)
                                    .then_some(segment.ref_name.as_ref())
                                    .flatten()
                            },
                        )
                    })
                {
                    if ws_mut
                        .insert_new_segment_above_anchor_if_not_present(
                            rn,
                            segment_to_insert_above.as_ref(),
                        )
                        .is_none()
                    {
                        // For now bail, until we know it's worth fixing this case automatically.
                        bail!(
                            "Missing reference {segment_to_insert_above} which should be known to workspace metadata to serve as insertion position for {rn}"
                        );
                    }
                } else {
                    bail!(
                        "Unexpectedly failed to find anchor for {rn} to make it a dependent branch"
                    )
                }
            }

            // Redo the merge, with the different stack configuration.
            // Note that this is the exception, typically using stacks will be fine.
            let merge_result = WorkspaceCommit::from_new_merge_with_metadata(
                &ws_md.stacks,
                workspace.graph,
                &in_memory_repo,
                Some(branch),
            )?;

            if merge_result.has_conflicts() && on_workspace_conflict.should_abort() {
                return Ok(Outcome {
                    graph: Cow::Owned(graph),
                    workspace_ref_created: needs_ws_ref_creation,
                    workspace_merge: Some(merge_result),
                });
            }
            new_head_id = merge_result.workspace_commit_id;
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
        persist_metadata(meta, &branches_to_apply, &ws_md)?;
        crate::branch::safe_checkout(
            prev_head_id,
            new_head_id,
            repo,
            checkout::Options {
                uncommitted_changes,
                skip_head_update: true,
            },
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
        })
    }

    fn persist_metadata<T: RefMetadata>(
        meta: &mut T,
        branches_to_apply: &Vec<&FullNameRef>,
        ws_md: &T::Handle<Workspace>,
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
        integration_mode: IntegrationMode,
    ) -> bool {
        match integration_mode {
            IntegrationMode::AlwaysMerge => match ws.kind {
                WorkspaceKind::Managed { .. } => false,
                WorkspaceKind::AdHoc => {
                    // If it's still ad-hoc, there must be a reason, and we don't try to create a managed commit
                    false
                }
                WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => true,
            },
            IntegrationMode::MergeIfNeeded => false,
        }
    }
}
