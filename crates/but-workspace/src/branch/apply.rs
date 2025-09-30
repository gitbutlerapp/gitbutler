use crate::branch::checkout::UncommitedWorktreeChanges;
use std::borrow::Cow;

/// Returned by [function::apply()].
pub struct Outcome<'graph> {
    /// The newly created graph, if owned, useful to project a workspace and see how the workspace looks like with the branch applied.
    /// If borrowed, the graph already contains the desired branch and nothing had to be applied.
    pub graph: Cow<'graph, but_graph::Graph>,
    /// `true` if we created the given workspace ref as it didn't exist yet.
    pub workspace_ref_created: bool,
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

/// What to do if the applied branch conflicts with the existing branches?
#[derive(Default, Debug, Copy, Clone)]
pub enum OnWorkspaceConflict {
    /// Provide additional information about the stack that conflicted and the files involved in it,
    /// and don't perform the operation.
    #[default]
    AbortAndReportConflictingStack,
    // TODO: unapply all conflicting (needs unapply)
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
    pub on_workspace_conflict: OnWorkspaceConflict,
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
    use crate::ref_info::WorkspaceExt;
    use anyhow::{Context, bail};
    use but_core::{RefMetadata, ref_metadata};
    use but_graph::init::Overlay;
    use but_graph::projection::WorkspaceKind;
    use gix::refs::Target;
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
    use std::borrow::Cow;

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
    pub fn apply<'graph>(
        branch: &gix::refs::FullNameRef,
        workspace: &but_graph::projection::Workspace<'graph>,
        repo: &mut gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            integration_mode,
            on_workspace_conflict: _,
            workspace_reference_naming,
            uncommitted_changes,
            order: _to_be_used_in_merge,
        }: Options,
    ) -> anyhow::Result<Outcome<'graph>> {
        if repo
            .try_find_reference(branch)?
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
            });
        } else if workspace.refname_is_segment(branch) {
            // This means our workspace encloses the desired branch, but it's not checked out yet.
            let commit_to_checkout = workspace
                .tip_commit()
                .context("Workspace must point to a commit to check out")?;
            let ep = workspace.graph.lookup_entrypoint()?;
            let current_head_commit = workspace
                .graph
                .tip_skip_empty(ep.segment_index)
                .context("The entrypoint must have a commit - it's equal to HEAD, and we skipped unborn earlier")?;
            crate::branch::safe_checkout(
                current_head_commit.id,
                commit_to_checkout.id,
                repo,
                checkout::Options {
                    uncommitted_changes,
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
            let ws_mut: &mut ref_metadata::Workspace = &mut ws_md;
            for rn in &branches_to_apply {
                ws_mut.add_new_stack_if_not_present(rn);
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
            .with_references_if_new(Some(gix::refs::Reference {
                name: workspace_ref_name_to_update.clone(),
                target: Target::Object(ws_ref_id),
                peeled: Some(ws_ref_id),
            }))
            .with_branch_metadata_override(branch_mds)
            .with_workspace_metadata_override(ws_md_override);
        let graph = workspace
            .graph
            .redo_traversal_with_overlay(repo, meta, overlay.clone())?;

        let workspace = graph.to_workspace()?;
        let all_applied_branches_are_already_visible = branches_to_apply
            .iter()
            .all(|rn| workspace.refname_is_segment(rn));
        if all_applied_branches_are_already_visible {
            let head_id = repo.head_id().context("BUG: we assume HEAD is born here")?;
            if head_id != ws_ref_id {
                bail!(
                    "Sanity check failed: we assume HEAD already points to where it must, but it really doesn't: {head_id} != {ws_ref_id}"
                );
            }
            meta.set_workspace(&ws_md)?;
            // Always re-obtain the branch information after it was set
            // or stuff will go wrong right now.
            // TODO: remove this note and keep using existing handles once vb.toml is gone.
            for rn in branches_to_apply {
                let mut md = meta.branch(rn)?;
                md.update_times(false /* is new ref */);
                meta.set_branch(&md)?;
            }

            let ws_commit = WorkspaceCommit::from_graph_workspace(
                &workspace,
                repo,
                head_id.object()?.peel_to_tree()?.id,
            )?;
            let new_head_id = ws_commit.id.detach();
            let (graph, new_head_id) = if (ws_commit.id != head_id
                && workspace.kind.has_managed_commit())
                || needs_workspace_commit_without_remerge(&workspace, integration_mode)
            {
                let graph = graph.redo_traversal_with_overlay(
                    repo,
                    meta,
                    overlay
                        .with_entrypoint(new_head_id, Some(workspace_ref_name_to_update.clone())),
                )?;
                (graph, new_head_id)
            } else {
                (graph, ws_ref_id)
            };

            let needs_ws_ref_creation = !ws_ref_exists;
            set_head_to_reference(
                repo,
                new_head_id,
                (!ws_ref_exists).then_some(workspace_ref_name_to_update.as_ref()),
            )?;
            return Ok(Outcome {
                graph: Cow::Owned(graph),
                workspace_ref_created: needs_ws_ref_creation,
            });
        }
        dbg!(
            workspace_ref_name_to_update,
            branches_to_apply
                .iter()
                .map(|rn| (rn, workspace.refname_is_segment(rn)))
                .collect::<Vec<_>>()
        );
        // Everything worked? Assure the ref exists now that (nearly nothing) can go wrong anymore.
        let _workspace_ref_created = false; // TODO: use rval of reference update to know if it existed.

        // if let Some(branch_md) = branch_to_apply_metadata {
        //     meta.set_branch(branch_md)?;
        // }

        todo!(
            "prepare outcome once all values were written out and the graph was regenerated - the simulation is now reality"
        );
        // Ok(Outcome {
        //     graph: Cow::Borrowed(workspace.graph),
        //     workspace_ref_created,
        // })
    }

    /// Set `HEAD` to point to `new_ref` if not `None`, but in any case, set what `HEAD` points to to be `new_ref_target`.
    fn set_head_to_reference(
        repo: &gix::Repository,
        new_ref_target: gix::ObjectId,
        new_ref: Option<&gix::refs::FullNameRef>,
    ) -> anyhow::Result<()> {
        // This also means we want HEAD to point to it.
        let head_message = "GitButler switch to workspace during apply-branch".into();
        let edits = match new_ref {
            None => {
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
