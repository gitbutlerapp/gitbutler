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
    use super::{Options, Outcome, WorkspaceReferenceNaming};
    use crate::branch::checkout;
    use crate::ref_info::WorkspaceExt;
    use anyhow::{Context, bail};
    use but_core::RefMetadata;
    use but_graph::init::Overlay;
    use but_graph::projection::WorkspaceKind;
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
    pub fn apply<'graph>(
        branch: &gix::refs::FullNameRef,
        workspace: &but_graph::projection::Workspace<'graph>,
        repo: &mut gix::Repository,
        meta: &mut impl RefMetadata,
        Options {
            integration_mode: _,
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
        if workspace.is_reachable_from_entrypoint(branch) {
            return Ok(Outcome {
                graph: Cow::Borrowed(workspace.graph),
                workspace_ref_created: false,
            });
        } else if workspace.refname_is_segment(branch) {
            // This means our workspace encloses the desired branch, but it's not checked out yet.
            let commit_to_checkout = workspace
                .tip_commit()
                .context("Workspace must point to a commit to check out")?;
            let current_head_commit = workspace
                .graph
                .lookup_entrypoint()?
                .commit
                .context("The entrypoint must have a commit - it's equal to HEAD")?;
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
        // we need to assure both branches go into the existing or the new workspace.
        let (_workspace_ref_name_to_update, _branches_to_apply) = match &workspace.kind {
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
                (
                    next_ws_ref_name,
                    workspace
                        .ref_name()
                        .into_iter()
                        .chain(Some(branch))
                        .collect(),
                )
                // let ws_ref_id = match repo.try_find_reference(next_ws_ref_name.as_ref())? {
                //     None => {
                //         // Create a workspace reference later at the current AdHoc workspace id
                //         let ws_id = workspace
                //             .stacks
                //             .first()
                //             .and_then(|s| s.segments.first())
                //             .and_then(|s| s.commits.first().map(|c| c.id))
                //             .context("BUG: how can an empty ad-hoc workspace exist? Should have at least one stack-segment with commit")?;
                //         ws_id
                //     }
                //     Some(mut existing_workspace_reference) => {
                //         let id = existing_workspace_reference.peel_to_id()?;
                //         id.detach()
                //     }
                // };

                // let mut ws_md = meta.workspace(next_ws_ref_name.as_ref())?;
                // {
                //     let ws_mut: &mut ref_metadata::Workspace = &mut *ws_md;
                //     ws_mut.stacks.push(WorkspaceStack {
                //         id: StackId::generate(),
                //         branches: vec![WorkspaceStackBranch {
                //             ref_name: branch.to_owned(),
                //             archived: false,
                //         }],
                //     })
                // }
                // let ws_md_override = Some((next_ws_ref_name.clone(), (*ws_md).clone()));

                // let graph = workspace.graph.redo_traversal_with_overlay(
                //     repo,
                //     meta,
                //     Overlay::default()
                //         .with_entrypoint(ws_ref_id, Some(next_ws_ref_name))
                //         .with_branch_metadata_override(branch_md_override)
                //         .with_workspace_metadata_override(ws_md_override),
                // )?;
            }
        };

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
}
