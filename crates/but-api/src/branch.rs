use std::{borrow::Cow, collections::BTreeMap};

use crate::WorkspaceState;
use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{
    DryRun, RefMetadata, WORKSPACE_REF_NAME,
    branch::unique_canned_refname,
    ref_metadata::StackId,
    sync::RepoExclusive,
    ui::TreeChanges,
    update_head_reference,
    worktree::{checkout, safe_checkout_from_head},
};
use but_ctx::Context;
use but_error::bail_precondition;
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::{Editor, SuccessfulRebase, mutate::InsertSide};
use but_workspace::branch::{
    BranchIntegrationStrategy, InitialBranchIntegration, OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
    integrate_branch_upstream::InteractiveIntegration,
};
use gix::refs::Target;
use gix::refs::transaction::PreviousValue;
use tracing::{instrument, warn};

/// Outcome after moving a branch.
pub struct MoveBranchResult {
    /// Workspace state after moving or tearing off a branch.
    pub workspace: WorkspaceState,
}

/// Outcome after creating a branch.
pub struct BranchCreateResult {
    /// Workspace state after creating the branch.
    pub workspace: WorkspaceState,
    /// The name of the crated reference
    pub new_ref: gix::refs::FullName,
}

/// Outcome after removing a branch.
#[derive(Debug)]
pub struct BranchRemoveResult {
    /// Workspace state after removing the branch.
    pub workspace: WorkspaceState,
}

/// Outcome after renaming a branch.
#[derive(Debug)]
pub struct BranchRenameResult {
    /// Workspace state after renaming the branch.
    pub workspace: WorkspaceState,
    /// The full name of the renamed reference.
    pub new_ref: gix::refs::FullName,
}

/// Outcome after integrating a branch with an interactive integration plan.
pub struct IntegrateBranchResult {
    /// Workspace state after applying or previewing the integration.
    pub workspace: WorkspaceState,
}

/// Outcome after checking out a branch.
#[derive(Debug)]
pub struct BranchCheckoutResult {
    /// Workspace state after checking out the branch.
    pub workspace: WorkspaceState,
}

/// JSON transport types for branch APIs.
pub mod json {
    use but_workspace::{branch::apply::OutcomeStatus, ui::ref_info::BranchReference};
    use serde::{Deserialize, Serialize};

    use crate::branch::{
        BranchCheckoutResult as InternalBranchCheckoutResult,
        BranchCreateResult as InternalBranchCreateResult,
        BranchRemoveResult as InternalBranchRemoveResult,
        BranchRenameResult as InternalBranchRenameResult,
        IntegrateBranchResult as InternalIntegrateBranchResult,
        MoveBranchResult as InternalMoveBranchResult,
    };

    /// JSON sibling of [`but_workspace::branch::apply::Outcome`].
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ApplyOutcome {
        /// What kind of apply operation completed.
        pub status: OutcomeStatus,
        /// Whether `apply()` produced a new workspace graph.
        ///
        /// This can be true even when merge conflicts prevented the result from being persisted.
        /// Use `status` to determine whether anything was persisted.
        pub workspace_changed: bool,
        /// Branches activated or recorded by the operation.
        ///
        /// This is empty for `alreadyApplied` and `conflictAborted`, and populated for `applied`.
        pub applied_branches: Vec<crate::json::FullRefName>,
        /// Whether the workspace reference had to be created.
        pub workspace_ref_created: bool,
        /// Stacks that conflicted while applying the branch.
        pub conflicting_stacks: Vec<ConflictingStack>,
    }

    /// A stack that conflicted while applying a branch.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ConflictingStack {
        /// The tip branch name of the stack.
        pub ref_name: crate::json::FullRefName,
        /// The shortened tip branch name, matching CLI display.
        pub short_name: String,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ConflictingStack);
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ApplyOutcome);

    impl From<but_workspace::branch::apply::Outcome> for ApplyOutcome {
        fn from(value: but_workspace::branch::apply::Outcome) -> Self {
            let workspace_changed = value.workspace_changed();
            let but_workspace::branch::apply::Outcome {
                workspace: _,
                status,
                applied_branches,
                workspace_ref_created,
                workspace_merge: _,
                conflicting_stacks,
            } = value;

            ApplyOutcome {
                status,
                workspace_changed,
                applied_branches: applied_branches.into_iter().map(Into::into).collect(),
                workspace_ref_created,
                conflicting_stacks: conflicting_stacks
                    .into_iter()
                    .map(|stack| {
                        let short_name = stack.ref_name.shorten().to_string();
                        ConflictingStack {
                            ref_name: stack.ref_name.into(),
                            short_name,
                        }
                    })
                    .collect(),
            }
        }
    }

    /// JSON transport type describing where to create a new branch.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase", tag = "type", content = "subject")]
    pub enum BranchCreatePlacement {
        /// Create the branch as a new independent stack at the workspace base.
        Independent,
        /// Create the branch relative to an existing commit or reference.
        ///
        /// When relative to a reference, the new branch points at the same commit
        /// as that reference and `side` only determines their ordering.
        /// When relative to a commit, `side` determines whether the branch points
        /// at the commit itself or at its parent.
        Dependent {
            /// The commit or reference to place the new branch next to.
            #[serde(rename = "relativeTo")]
            #[cfg_attr(feature = "export-schema", schemars(rename = "relativeTo"))]
            relative_to: crate::commit::json::RelativeTo,
            /// Which side of `relative_to` the new branch should be placed on.
            side: but_rebase::graph_rebase::mutate::InsertSide,
        },
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchCreatePlacement);

    /// JSON transport type for creating a branch.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct BranchCreateResult {
        /// Workspace state after creating the branch.
        pub workspace: crate::json::WorkspaceState,
        /// The name of the crated reference
        pub new_ref: BranchReference,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchCreateResult);

    impl TryFrom<InternalBranchCreateResult> for BranchCreateResult {
        type Error = anyhow::Error;

        fn try_from(value: InternalBranchCreateResult) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace: value.workspace.try_into()?,
                new_ref: value.new_ref.into(),
            })
        }
    }

    /// JSON transport type for removing a branch.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct BranchRemoveResult {
        /// Workspace state after removing the branch.
        pub workspace: crate::json::WorkspaceState,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchRemoveResult);

    impl TryFrom<InternalBranchRemoveResult> for BranchRemoveResult {
        type Error = anyhow::Error;

        fn try_from(value: InternalBranchRemoveResult) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace: value.workspace.try_into()?,
            })
        }
    }

    /// JSON transport type for renaming a branch.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct BranchRenameResult {
        /// Workspace state after renaming the branch.
        pub workspace: crate::json::WorkspaceState,
        /// The full name of the renamed reference.
        pub new_ref: BranchReference,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchRenameResult);

    impl TryFrom<InternalBranchRenameResult> for BranchRenameResult {
        type Error = anyhow::Error;

        fn try_from(value: InternalBranchRenameResult) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace: value.workspace.try_into()?,
                new_ref: value.new_ref.into(),
            })
        }
    }

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    /// JSON transport type for checking out a branch.
    pub struct BranchCheckoutResult {
        /// Workspace state after checking out the branch.
        pub workspace: crate::json::WorkspaceState,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchCheckoutResult);

    impl TryFrom<InternalBranchCheckoutResult> for BranchCheckoutResult {
        type Error = anyhow::Error;

        fn try_from(value: InternalBranchCheckoutResult) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace: value.workspace.try_into()?,
            })
        }
    }

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    /// JSON transport type for moving a branch.
    pub struct MoveBranchResult {
        /// Workspace state after moving or tearing off a branch.
        pub workspace: crate::json::WorkspaceState,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(MoveBranchResult);

    impl TryFrom<InternalMoveBranchResult> for MoveBranchResult {
        type Error = anyhow::Error;

        fn try_from(value: InternalMoveBranchResult) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace: value.workspace.try_into()?,
            })
        }
    }

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    /// JSON transport type for integrating a branch.
    pub struct IntegrateBranchResult {
        /// Workspace state after applying or previewing the integration.
        pub workspace: crate::json::WorkspaceState,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(IntegrateBranchResult);

    impl TryFrom<InternalIntegrateBranchResult> for IntegrateBranchResult {
        type Error = anyhow::Error;

        fn try_from(value: InternalIntegrateBranchResult) -> Result<Self, Self::Error> {
            Ok(Self {
                workspace: value.workspace.try_into()?,
            })
        }
    }

    /// JSON transport type for a divergence commit row.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase", tag = "kind")]
    pub enum IntegrationDivergenceTargetRelation {
        /// The commit is not present in the target branch.
        NotIntegrated,
        /// The exact commit is reachable from target branch history.
        HistoricallyIntegrated {
            /// The target branch commit that establishes the relation.
            #[serde(rename = "targetCommitId")]
            #[cfg_attr(feature = "export-schema", schemars(rename = "targetCommitId"))]
            target_commit_id: crate::json::HexHashString,
        },
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(IntegrationDivergenceTargetRelation);

    impl From<but_workspace::branch::IntegrationDivergenceTargetRelation>
        for IntegrationDivergenceTargetRelation
    {
        fn from(value: but_workspace::branch::IntegrationDivergenceTargetRelation) -> Self {
            match value {
                but_workspace::branch::IntegrationDivergenceTargetRelation::NotIntegrated => {
                    Self::NotIntegrated
                }
                but_workspace::branch::IntegrationDivergenceTargetRelation::HistoricallyIntegrated {
                    target_commit_id,
                } => Self::HistoricallyIntegrated {
                    target_commit_id: target_commit_id.into(),
                },
            }
        }
    }

    /// JSON transport type for a divergence commit row.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct IntegrationDivergenceCommit {
        /// The commit shown in the graph row.
        pub id: crate::json::HexHashString,
        /// The first-line subject shown for the commit.
        pub subject: String,
        /// The explicit GitButler Change-Id stored in the commit headers, if present.
        pub change_id: Option<String>,
        /// Commit creation time in Epoch milliseconds.
        pub created_at: i128,
        /// The author of the commit.
        pub author: but_workspace::ui::Author,
        /// Human-facing ref labels rendered inline on the commit row.
        pub refs: Vec<String>,
        /// How this commit relates to the configured target branch.
        pub target_relation: IntegrationDivergenceTargetRelation,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(IntegrationDivergenceCommit);

    impl From<but_workspace::branch::IntegrationDivergenceCommit> for IntegrationDivergenceCommit {
        fn from(value: but_workspace::branch::IntegrationDivergenceCommit) -> Self {
            let but_workspace::branch::IntegrationDivergenceCommit {
                id,
                subject,
                change_id,
                created_at,
                author,
                refs,
                target_relation,
            } = value;
            Self {
                id: id.into(),
                subject,
                change_id,
                created_at,
                author,
                refs,
                target_relation: target_relation.into(),
            }
        }
    }

    /// JSON transport type for current branch/upstream divergence information.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct IntegrationDivergenceDisplay {
        /// The local branch being integrated.
        pub branch_ref_name: crate::json::FullRefName,
        /// The upstream branch this local branch integrates with.
        pub upstream_ref_name: crate::json::FullRefName,
        /// Commits only reachable from the local branch tip down to the shared section.
        pub local_only: Vec<IntegrationDivergenceCommit>,
        /// Commits only reachable from the upstream branch tip down to the shared section.
        pub upstream_only: Vec<IntegrationDivergenceCommit>,
        /// The merge-base row shown once at the bottom.
        pub merge_base: IntegrationDivergenceCommit,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(IntegrationDivergenceDisplay);

    impl From<but_workspace::branch::IntegrationDivergenceDisplay> for IntegrationDivergenceDisplay {
        fn from(value: but_workspace::branch::IntegrationDivergenceDisplay) -> Self {
            let but_workspace::branch::IntegrationDivergenceDisplay {
                branch_ref_name,
                upstream_ref_name,
                local_only,
                upstream_only,
                merge_base,
            } = value;
            Self {
                branch_ref_name: branch_ref_name.into(),
                upstream_ref_name: upstream_ref_name.into(),
                local_only: local_only.into_iter().map(Into::into).collect(),
                upstream_only: upstream_only.into_iter().map(Into::into).collect(),
                merge_base: merge_base.into(),
            }
        }
    }

    /// JSON transport type for the preset used to generate initial branch integration steps.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub enum BranchIntegrationStrategy {
        /// Rebase local commits on top of the upstream commits.
        PullRebase,
        /// Keep local commits first, then merge the upstream tip.
        Merge,
        /// Rebuild the branch by picking upstream commits only.
        PickRemote,
        /// Fold upstream commits with matching explicit Change-Ids into local commits.
        SmartSquash,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchIntegrationStrategy);

    impl From<BranchIntegrationStrategy> for but_workspace::branch::BranchIntegrationStrategy {
        fn from(value: BranchIntegrationStrategy) -> Self {
            match value {
                BranchIntegrationStrategy::PullRebase => Self::PullRebase,
                BranchIntegrationStrategy::Merge => Self::Merge,
                BranchIntegrationStrategy::PickRemote => Self::PickRemote,
                BranchIntegrationStrategy::SmartSquash => Self::SmartSquash,
            }
        }
    }

    /// JSON transport type for a branch integration step.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase", tag = "kind")]
    pub enum InteractiveIntegrationStep {
        /// Pick a commit, keeping it in the branch.
        Pick {
            /// The local commit to keep in the rewritten branch.
            #[serde(rename = "commitId")]
            #[cfg_attr(feature = "export-schema", schemars(rename = "commitId"))]
            commit_id: crate::json::HexHashString,
        },
        /// Squash multiple commits into one.
        Squash {
            /// The ordered commits to squash together.
            commits: Vec<crate::json::HexHashString>,
            /// Optional replacement message for the squash commit.
            message: Option<String>,
        },
        /// Merge a commit into the previous one.
        Merge {
            /// The commit whose change range should be merged.
            #[serde(rename = "commitId")]
            #[cfg_attr(feature = "export-schema", schemars(rename = "commitId"))]
            commit_id: crate::json::HexHashString,
        },
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(InteractiveIntegrationStep);

    impl TryFrom<InteractiveIntegrationStep>
        for but_workspace::branch::integrate_branch_upstream::InteractiveIntegrationStep
    {
        type Error = anyhow::Error;

        fn try_from(value: InteractiveIntegrationStep) -> Result<Self, Self::Error> {
            Ok(match value {
                InteractiveIntegrationStep::Pick { commit_id } => Self::Pick {
                    commit_id: commit_id.try_into()?,
                },
                InteractiveIntegrationStep::Squash { commits, message } => Self::Squash {
                    commits: commits
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<_, _>>()?,
                    message,
                },
                InteractiveIntegrationStep::Merge { commit_id } => Self::Merge {
                    commit_id: commit_id.try_into()?,
                },
            })
        }
    }

    /// JSON transport type describing an interactive branch integration plan.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct InteractiveIntegration {
        /// Merge base between the upstream and the local reference.
        #[serde(rename = "mergeBase")]
        #[cfg_attr(feature = "export-schema", schemars(rename = "mergeBase"))]
        pub merge_base: crate::json::HexHashString,
        /// The first parent-to-child local commit that is not historically integrated into target.
        pub first_local_not_integrated: Option<crate::json::HexHashString>,
        /// The ordered integration steps to apply.
        pub steps: Vec<InteractiveIntegrationStep>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(InteractiveIntegration);

    impl TryFrom<InteractiveIntegration>
        for but_workspace::branch::integrate_branch_upstream::InteractiveIntegration
    {
        type Error = anyhow::Error;

        fn try_from(value: InteractiveIntegration) -> Result<Self, Self::Error> {
            let InteractiveIntegration {
                merge_base,
                first_local_not_integrated,
                steps,
            } = value;
            Ok(Self {
                merge_base: merge_base.try_into()?,
                first_local_not_integrated: first_local_not_integrated
                    .map(TryInto::try_into)
                    .transpose()?,
                steps: steps
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
            })
        }
    }

    /// JSON transport type for the initial branch integration proposal.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct InitialBranchIntegration {
        /// The editable execution plan for integrating the branch upstream.
        pub integration: InteractiveIntegration,
        /// The current divergence between local branch and upstream for display.
        pub divergence: IntegrationDivergenceDisplay,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(InitialBranchIntegration);

    impl TryFrom<but_workspace::branch::InitialBranchIntegration> for InitialBranchIntegration {
        type Error = anyhow::Error;

        fn try_from(
            value: but_workspace::branch::InitialBranchIntegration,
        ) -> Result<Self, Self::Error> {
            let but_workspace::branch::InitialBranchIntegration {
                integration,
                divergence,
            } = value;
            Ok(Self {
                integration: InteractiveIntegration {
                    merge_base: integration.merge_base.into(),
                    first_local_not_integrated: integration
                        .first_local_not_integrated
                        .map(Into::into),
                    steps: integration
                        .steps
                        .into_iter()
                        .map(|step| match step {
                            but_workspace::branch::integrate_branch_upstream::InteractiveIntegrationStep::Pick { commit_id } => {
                                InteractiveIntegrationStep::Pick {
                                    commit_id: commit_id.into(),
                                }
                            }
                            but_workspace::branch::integrate_branch_upstream::InteractiveIntegrationStep::Squash { commits, message } => {
                                InteractiveIntegrationStep::Squash {
                                    commits: commits.into_iter().map(Into::into).collect(),
                                    message,
                                }
                            }
                            but_workspace::branch::integrate_branch_upstream::InteractiveIntegrationStep::Merge { commit_id } => {
                                InteractiveIntegrationStep::Merge {
                                    commit_id: commit_id.into(),
                                }
                            }
                        })
                        .collect(),
                },
                divergence: divergence.into(),
            })
        }
    }
}

/// Applies a branch using the behavior described by [`apply_only_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx` before applying
/// `existing_branch`.
pub fn apply_only(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<but_workspace::branch::apply::Outcome> {
    let mut guard = ctx.exclusive_worktree_access();
    apply_only_with_perm(ctx, existing_branch, guard.write_permission())
}

/// Applies `existing_branch` to the current workspace under caller-held
/// exclusive repository access.
///
/// It applies the branch with the default workspace-apply options, updates the
/// in-memory workspace stored in `ctx` to the returned workspace state when the
/// state was persisted, and returns the apply outcome. This variant does not
/// create an oplog entry. For lower-level implementation details, see
/// [`but_workspace::branch::apply()`].
pub fn apply_only_with_perm(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
    perm: &mut RepoExclusive,
) -> anyhow::Result<but_workspace::branch::apply::Outcome> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let out = but_workspace::branch::apply(
        existing_branch,
        ws.clone(),
        &repo,
        &mut meta,
        // NOTE: Options can later be passed as parameter, or we have a separate function for that.
        //       Showing them off here while leaving defaults.
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::default(),
            on_workspace_conflict: OnWorkspaceMergeConflict::default(),
            workspace_reference_naming: WorkspaceReferenceNaming::default(),
            order: None,
            new_stack_id: None,
        },
    )?;

    if out.status.persisted_mutation() {
        *ws = out.workspace.clone();
    }
    Ok(out)
}

/// Applies `existing_branch` using the behavior described by
/// [`apply_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx`, applies
/// `existing_branch`, and records an oplog snapshot on success.
#[but_api(napi, json::ApplyOutcome)]
#[instrument(err(Debug))]
pub fn apply(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<but_workspace::branch::apply::Outcome> {
    let mut guard = ctx.exclusive_worktree_access();
    apply_with_perm(ctx, existing_branch, guard.write_permission())
}

/// Apply `existing_branch` to the workspace under caller-held exclusive
/// repository access and record an oplog snapshot on success.
///
/// It behaves like [`apply_only_with_perm()`], but first prepares a best-effort
/// oplog snapshot for a create-branch operation, annotated with the branch
/// name, and commits that snapshot only if the apply succeeds. For lower-level
/// implementation details, see [`but_workspace::branch::apply()`].
pub fn apply_with_perm(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
    perm: &mut RepoExclusive,
) -> anyhow::Result<but_workspace::branch::apply::Outcome> {
    // NOTE: since this is optional by nature, the same would be true if snapshotting/undo would be disabled via `ctx` app settings, for instance.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateBranch)
            .with_trailers([Trailer::Name(existing_branch.to_string())]),
        perm.read_permission(),
        DryRun::No,
    );

    let res = apply_only_with_perm(ctx, existing_branch, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res
            .as_ref()
            .is_ok_and(|out| out.status.persisted_mutation())
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}

/// Creates a new branch named `new_ref` at `placement`.
///
/// This acquires exclusive worktree access from `ctx`, creates the branch,
/// records an oplog snapshot on success, and in ad-hoc/single-branch mode
/// checks out the new branch when it was created directly above the currently
/// checked-out local branch. For lower-level implementation details, see
/// [`but_workspace::branch::create_reference()`].
#[but_api(napi, try_from = json::BranchCreateResult)]
#[instrument(err(Debug))]
pub fn branch_create(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::MaybeLossyFullNameRef)] new_ref: Option<gix::refs::FullName>,
    placement: json::BranchCreatePlacement,
) -> anyhow::Result<BranchCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_create_with_perm(ctx, new_ref, placement, guard.write_permission())
}

/// Create a new branch named `new_ref` at `placement` under caller-held
/// exclusive repository access and record an oplog snapshot on success.
///
/// It prepares a best-effort create-branch oplog snapshot, creates the
/// reference along with its workspace metadata, and commits the snapshot only
/// if the creation succeeds. In ad-hoc/single-branch mode, if the placement is
/// `Above` the exact symbolic `HEAD` branch, this also checks out the newly
/// created branch after metadata has been persisted. The returned
/// [`BranchCreateResult`] contains the post-operation workspace view. For
/// lower-level implementation details, see
/// [`but_workspace::branch::create_reference()`].
pub fn branch_create_with_perm(
    ctx: &mut but_ctx::Context,
    new_ref: Option<gix::refs::FullName>,
    placement: json::BranchCreatePlacement,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCreateResult> {
    use but_workspace::branch::create_reference::{Anchor, Position};

    let anchor = match placement {
        json::BranchCreatePlacement::Independent => None,
        json::BranchCreatePlacement::Dependent { relative_to, side } => {
            let position = match side {
                InsertSide::Above => Position::Above,
                InsertSide::Below => Position::Below,
            };
            Some(match relative_to {
                crate::commit::json::RelativeTo::Commit(commit_id) => Anchor::AtCommit {
                    commit_id,
                    position,
                },
                crate::commit::json::RelativeTo::Reference(ref_name)
                | crate::commit::json::RelativeTo::ReferenceBytes(ref_name) => {
                    Anchor::AtReference {
                        ref_name: Cow::Owned(ref_name),
                        position,
                    }
                }
            })
        }
    };

    let checkout_anchor_ref = match &anchor {
        Some(Anchor::AtReference { ref_name, position })
            if matches!(position, Position::Above)
                && ref_name.category() == Some(gix::refs::Category::LocalBranch) =>
        {
            Some(ref_name.as_ref().to_owned())
        }
        _ => None,
    };

    let new_ref = if let Some(new_ref) = new_ref {
        new_ref
    } else {
        let repo = ctx.repo.get()?;
        unique_canned_refname(&repo)?
    };

    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateBranch)
            .with_trailers([Trailer::Name(new_ref.to_string())]),
        perm.read_permission(),
        DryRun::No,
    );
    let mut meta = ctx.meta()?;
    let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let checkout_after_create = checkout_anchor_ref.as_ref().is_some_and(|anchor_ref| {
        repo.head_name()
            .ok()
            .flatten()
            .as_ref()
            .is_some_and(|head_ref| head_ref.as_ref() == anchor_ref.as_ref())
    });
    let new_ws = but_workspace::branch::create_reference(
        new_ref.as_ref(),
        anchor,
        &repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None,
    )?;
    if let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }

    let workspace =
        WorkspaceState::from_workspace_with_db(&new_ws, &mut meta, &repo, BTreeMap::new(), &db)?;
    *ws = new_ws.into_owned();
    drop(ws);
    drop(repo);
    drop(db);
    drop(meta);
    if checkout_after_create {
        let checkout = branch_checkout_with_perm(ctx, new_ref.clone(), perm)?;
        return Ok(BranchCreateResult {
            workspace: checkout.workspace,
            new_ref,
        });
    }
    Ok(BranchCreateResult { workspace, new_ref })
}

/// Removes the local branch `ref_name` from the workspace, deleting its git
/// reference along with its metadata (including its `branch_order` entry).
///
/// This acquires exclusive worktree access from `ctx`, records an oplog snapshot
/// on success, and returns the post-operation workspace view. In an
/// ad-hoc/single-branch workspace it can also remove the currently checked-out
/// reference: when that reference owns no commits and has another named
/// reference underneath it, `HEAD` is first moved onto the reference below (the
/// reverse of creating an empty branch above the checked-out one). For
/// lower-level implementation details, see
/// [`but_workspace::branch::remove_reference()`].
#[but_api(napi, try_from = json::BranchRemoveResult)]
#[instrument(err(Debug))]
pub fn branch_remove(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::FullNameBytes)] ref_name: gix::refs::FullName,
) -> anyhow::Result<BranchRemoveResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_remove_with_perm(ctx, ref_name, guard.write_permission())
}

/// Remove the local branch `ref_name` under caller-held exclusive repository
/// access and record an oplog snapshot on success.
///
/// See [`branch_remove()`] for the checked-out-reference behaviour and
/// [`but_workspace::branch::remove_reference()`] for the lower-level deletion.
pub fn branch_remove_with_perm(
    ctx: &mut but_ctx::Context,
    ref_name: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchRemoveResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::DeleteBranch)
            .with_trailers([Trailer::Name(ref_name.to_string())]),
        perm.read_permission(),
        DryRun::No,
    );

    // Decide whether we must move `HEAD` off `ref_name` before deleting it. In an
    // ad-hoc workspace the checked-out reference is the projection tip; we only
    // allow removing it when it owns no commits and has another named reference
    // underneath to land `HEAD` on. This reverses the "create an empty branch
    // above the checked-out reference" flow. We look at the projection rather
    // than the branch-order metadata on purpose: the metadata is best-effort and
    // may drift, whereas the projection reflects the real segments.
    let move_head_to = {
        let (repo, ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        let is_checked_out = repo
            .head_name()
            .ok()
            .flatten()
            .is_some_and(|head| head.as_ref() == ref_name.as_ref());
        if is_checked_out {
            let (stack, _segment) = ws
                .find_segment_and_stack_by_refname(ref_name.as_ref())
                .context("the checked-out branch is not part of the workspace")?;
            let idx = stack
                .segments
                .iter()
                .position(|s| s.ref_name() == Some(ref_name.as_ref()))
                .expect("segment we just matched by ref name");
            let is_empty = stack.segments[idx].commits.is_empty();
            let below = stack.segments[idx + 1..]
                .iter()
                .find_map(|s| s.ref_name().map(|r| r.to_owned()));
            match (is_empty, below) {
                (true, Some(below)) => Some(below),
                (true, None) => bail_precondition!(
                    "Cannot remove '{}': it is the only branch in the workspace",
                    ref_name.shorten()
                ),
                (false, _) => bail_precondition!(
                    "Cannot remove the checked-out branch '{}' because it contains commits",
                    ref_name.shorten()
                ),
            }
        } else {
            None
        }
    };

    let changed = if let Some(below) = move_head_to {
        // Land `HEAD` on the reference underneath, then delete the now-detached
        // tip and its metadata directly: after the checkout the tip sits above
        // the entrypoint and is no longer part of the downward projection, so
        // `remove_reference` would not find it there.
        branch_checkout_with_perm(ctx, below, perm)?;

        let mut meta = ctx.meta()?;
        let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        let deleted_ref = if let Some(reference) = repo.try_find_reference(ref_name.as_ref())? {
            let safe_delete = but_core::branch::SafeDelete::new(&repo)?;
            let out = safe_delete.delete_reference(&reference)?;
            if let Some(paths) = out.checked_out_in_worktree_dirs {
                bail_precondition!(
                    "Refusing to delete a branch that is checked out. Worktrees are: {paths:?}"
                );
            }
            true
        } else {
            false
        };
        let deleted_meta = meta.remove(ref_name.as_ref())?;
        if deleted_ref || deleted_meta {
            let new_ws = ws
                .graph
                .redo_traversal_with_overlay(&repo, &meta, Default::default())?
                .into_workspace()?;
            *ws = new_ws;
            true
        } else {
            false
        }
    } else {
        let mut meta = ctx.meta()?;
        let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        let new_ws = but_workspace::branch::remove_reference(
            ref_name.as_ref(),
            &repo,
            &ws,
            &mut meta,
            but_workspace::branch::remove_reference::Options {
                avoid_anonymous_stacks: true,
                keep_metadata: false,
            },
        )?;
        let changed = new_ws.is_some();
        if let Some(new_ws) = new_ws {
            *ws = new_ws;
        }
        changed
    };

    if changed && let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }
    let mut meta = ctx.meta()?;
    let (repo, ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let workspace =
        WorkspaceState::from_workspace_with_db(&ws, &mut meta, &repo, BTreeMap::new(), &db)?;
    Ok(BranchRemoveResult { workspace })
}

/// Renames the local branch `ref_name` to `new_name`, moving its git reference and
/// its metadata (including its `branch_order` entry) and, when it is the
/// checked-out branch, re-pointing `HEAD` at the new name.
///
/// `new_name` is a short branch name that is normalized into a valid
/// `refs/heads/<name>` reference before the rename, so callers don't have to
/// pre-normalize it. This acquires exclusive worktree access from `ctx`, records
/// an oplog snapshot on success, and returns the post-operation workspace view.
/// It requires no stack id and works in both managed and ad-hoc/single-branch
/// workspaces.
#[but_api(napi, try_from = json::BranchRenameResult)]
#[instrument(err(Debug))]
pub fn branch_rename(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::FullNameBytes)] ref_name: gix::refs::FullName,
    new_name: String,
) -> anyhow::Result<BranchRenameResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_rename_with_perm(ctx, ref_name, new_name, guard.write_permission())
}

/// Rename the local branch `ref_name` to `new_name` under caller-held exclusive
/// repository access and record an oplog snapshot on success.
///
/// See [`branch_rename()`] for the higher-level behaviour.
pub fn branch_rename_with_perm(
    ctx: &mut but_ctx::Context,
    ref_name: gix::refs::FullName,
    new_name: String,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchRenameResult> {
    // This only renames local branches. Reject anything else up front (mirroring
    // `branch_checkout_with_perm`) so an SDK/server caller can't pass e.g.
    // `refs/remotes/origin/foo` and have us create a local branch while deleting the
    // remote-tracking ref.
    if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
        bail!(
            "Can only rename local branches under refs/heads, got '{}'",
            ref_name.as_bstr()
        );
    }

    // Normalize the requested name into a valid local branch reference. This is the non-legacy
    // counterpart to the old `normalize_branch_name`, so any caller can pass a raw, human-entered
    // name and get a valid ref.
    let normalized = but_core::branch::normalize_short_name(new_name.as_str())?;
    let new_ref = gix::refs::Category::LocalBranch.to_full_name(normalized.as_bstr())?;

    // Renaming onto the same name is a no-op that still returns the current view.
    if ref_name.as_ref() == new_ref.as_ref() {
        let mut meta = ctx.meta()?;
        let (repo, ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
        repo.find_reference(ref_name.as_ref())
            .with_context(|| format!("Branch '{}' does not exist", ref_name.shorten()))?;
        let workspace =
            WorkspaceState::from_workspace_with_db(&ws, &mut meta, &repo, BTreeMap::new(), &db)?;
        return Ok(BranchRenameResult { workspace, new_ref });
    }

    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::UpdateBranchName).with_trailers([
            // Distinct keys (`previous_name` / `name`) so both survive `.find(key)` lookups on the
            // consumer side; two `Name` trailers would collide and drop the second.
            Trailer::PreviousName(ref_name.to_string()),
            Trailer::Name(new_ref.to_string()),
        ]),
        perm.read_permission(),
        DryRun::No,
    );

    // --- git reference + metadata mutation (borrows dropped before the reload below) ---
    {
        let mut meta = ctx.meta()?;
        let (repo, _ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;

        let old_reference = repo
            .find_reference(ref_name.as_ref())
            .with_context(|| format!("Branch '{}' does not exist", ref_name.shorten()))?;
        let target_id = old_reference.clone().peel_to_id()?.detach();

        if repo.try_find_reference(new_ref.as_ref())?.is_some() {
            bail_precondition!("A branch named '{}' already exists", new_ref.shorten());
        }

        // Also reject a destination that is free at the git level but still occupied in workspace
        // metadata (e.g. a stale managed head or `branch_order` entry left behind after an external
        // branch deletion). `meta.rename()` performs these same checks, but only *after* we would
        // have moved the refs — bailing there would leave the repository renamed while the metadata
        // stayed keyed by the old name. Preflighting here keeps the ref and metadata steps from
        // diverging.
        if meta.branch_opt(new_ref.as_ref())?.is_some()
            || meta.branch_stack_order(new_ref.as_ref())?.is_some()
        {
            bail_precondition!("A branch named '{}' already exists", new_ref.shorten());
        }

        // Bail *before* mutating any refs if the branch is checked out in another worktree:
        // otherwise the delete below would silently no-op and leave a partially-applied rename
        // (new ref created, old ref still present). Unlike `branch_remove`, which performs no ref
        // mutation before `SafeDelete` checks this condition, rename needs this explicit preflight.
        // The current worktree is excluded here — if its HEAD is on the old branch it gets
        // repointed below rather than blocking the rename.
        let checkout_probe = but_core::branch::SafeDelete::new(&repo)?;
        if let Some(dirs) = checkout_probe.worktree_dirs_with_ref(&old_reference) {
            let current_workdir = repo.workdir();
            let elsewhere: Vec<_> = dirs
                .iter()
                .filter(|dir| Some(dir.as_path()) != current_workdir)
                .collect();
            if !elsewhere.is_empty() {
                bail_precondition!(
                    "Refusing to rename a branch that is checked out elsewhere. Worktrees are: {elsewhere:?}"
                );
            }
        }

        // Whether HEAD (in this worktree) points at the old branch. If so it has to follow the
        // rename to the new name; the commit is unchanged, so there is no worktree/index update to
        // do — only the symbolic ref moves.
        let head_on_old = repo
            .head_name()
            .ok()
            .flatten()
            .is_some_and(|head| head.as_ref() == ref_name.as_ref());

        let prefix_related = refs_are_prefix_related(ref_name.as_ref(), new_ref.as_ref());
        let mut backup_reference = None;
        if prefix_related {
            // One name is a directory prefix of the other (e.g. `foo` -> `foo/bar`), so the two refs
            // cannot exist at the same time on disk. The usual create-then-delete order hits a
            // directory/file conflict. Protect the commit with an internal backup ref before
            // deleting the old branch. If destination creation fails, restore the source from this
            // backup; on a later failure the backup remains available for recovery.
            let backup_ref: gix::refs::FullName =
                format!("refs/gitbutler/rename-backup/{}", uuid::Uuid::new_v4()).try_into()?;
            backup_reference = Some(
                repo.reference(
                    backup_ref.as_ref(),
                    target_id,
                    PreviousValue::MustNotExist,
                    "back up branch before rename",
                )
                .with_context(|| {
                    format!(
                        "Could not create recovery ref before renaming '{}'",
                        ref_name.as_bstr()
                    )
                })?,
            );

            if let Err(delete_err) = old_reference.delete() {
                if let Some(backup) = backup_reference.take()
                    && let Err(err) = backup.delete()
                {
                    warn!(
                        ?err,
                        "failed to remove branch-rename recovery ref after source deletion failed"
                    );
                }
                return Err(anyhow::Error::new(delete_err)
                    .context(format!("Could not delete branch '{}'", ref_name.as_bstr())));
            }
            if let Err(create_err) = repo.reference(
                new_ref.as_ref(),
                target_id,
                PreviousValue::MustNotExist,
                "rename branch",
            ) {
                let create_err = anyhow::Error::new(create_err)
                    .context(format!("Could not create branch '{}'", new_ref.as_bstr()));
                match repo.reference(
                    ref_name.as_ref(),
                    target_id,
                    PreviousValue::MustNotExist,
                    "restore branch after failed rename",
                ) {
                    Ok(_) => {
                        if let Some(backup) = backup_reference.take()
                            && let Err(err) = backup.delete()
                        {
                            warn!(
                                ?err,
                                "failed to remove branch-rename recovery ref after restoring source"
                            );
                        }
                        return Err(create_err);
                    }
                    Err(restore_err) => {
                        let backup_name = backup_reference
                            .as_ref()
                            .map(|reference| reference.name().to_string())
                            .unwrap_or_else(|| "<unknown>".into());
                        return Err(create_err.context(format!(
                            "Restoring source branch '{}' also failed: {restore_err}. The commit remains protected by recovery ref '{backup_name}'",
                            ref_name.as_bstr()
                        )));
                    }
                }
            }
            if head_on_old {
                update_head_reference(
                    &repo,
                    Target::Symbolic(new_ref.clone()),
                    false,
                    "rename",
                    new_ref.as_bstr(),
                    repo.find_commit(target_id)?.parent_ids().count(),
                )
                .with_context(|| {
                    let backup_name = backup_reference
                        .as_ref()
                        .map(|reference| reference.name().to_string())
                        .unwrap_or_else(|| "<unknown>".into());
                    format!(
                        "Could not update HEAD to '{}'. Recovery ref '{backup_name}' was retained",
                        new_ref.as_bstr()
                    )
                })?;
            }
        } else {
            // Create the new reference at the same commit as the old one.
            repo.reference(
                new_ref.as_ref(),
                target_id,
                PreviousValue::MustNotExist,
                "rename branch",
            )
            .with_context(|| format!("Could not create branch '{}'", new_ref.as_bstr()))?;

            if head_on_old {
                update_head_reference(
                    &repo,
                    Target::Symbolic(new_ref.clone()),
                    false,
                    "rename",
                    new_ref.as_bstr(),
                    repo.find_commit(target_id)?.parent_ids().count(),
                )
                .with_context(|| format!("Could not update HEAD to '{}'", new_ref.as_bstr()))?;
            }

            // Delete the old reference (HEAD has already moved off it, so this is allowed).
            let safe_delete = but_core::branch::SafeDelete::new(&repo)?;
            let out = safe_delete.delete_reference(&old_reference)?;
            if let Some(paths) = out.checked_out_in_worktree_dirs {
                bail_precondition!(
                    "Refusing to rename a branch that is checked out elsewhere. Worktrees are: {paths:?}"
                );
            }
        }

        // Move all metadata (per-branch blob + branch-order entry) to the new name.
        if let Err(err) = meta.rename(ref_name.as_ref(), new_ref.as_ref()) {
            if let Some(backup) = backup_reference.as_ref() {
                return Err(err).context(format!(
                    "Could not rename branch metadata. Recovery ref '{}' was retained",
                    backup.name()
                ));
            }
            return Err(err);
        }

        if let Some(backup) = backup_reference
            && let Err(err) = backup.delete()
        {
            warn!(?err, "failed to remove branch-rename recovery ref");
        }
    }

    // Rebuild the workspace from scratch: this re-reads HEAD, so the moved-HEAD case needs no
    // stale-entrypoint handling.
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    if let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }
    let mut meta = ctx.meta()?;
    let (repo, ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let workspace =
        WorkspaceState::from_workspace_with_db(&ws, &mut meta, &repo, BTreeMap::new(), &db)?;
    Ok(BranchRenameResult { workspace, new_ref })
}

/// Whether `a` and `b` are in a directory/file prefix relationship, i.e. one full ref name is a
/// path-component prefix of the other (`refs/heads/foo` vs `refs/heads/foo/bar`). Such refs cannot
/// coexist on disk, so a rename between them must delete the old ref *before* creating the new one,
/// rather than the usual create-then-delete order. A shared prefix that is not on a `/` boundary
/// (`foo` vs `foobar`) is not prefix-related.
fn refs_are_prefix_related(a: &gix::refs::FullNameRef, b: &gix::refs::FullNameRef) -> bool {
    fn is_dir_prefix(prefix: &bstr::BStr, full: &bstr::BStr) -> bool {
        full.len() > prefix.len() && full.starts_with(prefix) && full[prefix.len()] == b'/'
    }
    let (a, b) = (a.as_bstr(), b.as_bstr());
    is_dir_prefix(a, b) || is_dir_prefix(b, a)
}

/// Checks out an existing local branch and returns the resulting workspace state.
///
/// This acquires exclusive worktree access from `ctx`, updates the worktree and
/// index through [`but_core::worktree::safe_checkout_from_head()`], then points `HEAD`
/// symbolically at `branch`. The branch must be an existing full local branch
/// name under `refs/heads/`.
#[but_api(napi, try_from = json::BranchCheckoutResult)]
#[instrument(err(Debug))]
pub fn branch_checkout(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::FullNameBytes)] branch: gix::refs::FullName,
) -> anyhow::Result<BranchCheckoutResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_checkout_with_perm(ctx, branch, guard.write_permission())
}

/// Creates a new local branch at the project target SHA, checks it out, and
/// returns the resulting workspace state.
///
/// If `name` is provided, it is treated as a short branch name and normalized
/// before creating `refs/heads/<name>`. If omitted, a unique canned branch name
/// is generated. The resulting branch must not already exist.
#[but_api(napi, try_from = json::BranchCheckoutResult)]
#[instrument(err(Debug))]
pub fn branch_checkout_new(
    ctx: &mut but_ctx::Context,
    name: Option<String>,
) -> anyhow::Result<BranchCheckoutResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_checkout_new_with_perm(ctx, name, guard.write_permission())
}

/// Creates a new local branch at the project target SHA and checks it out under
/// caller-held exclusive repository access.
pub fn branch_checkout_new_with_perm(
    ctx: &mut but_ctx::Context,
    name: Option<String>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    let target_commit_id = ctx.project_meta()?.target_commit_id_or_err()?;
    let branch = {
        let repo = ctx.repo.get()?;
        let branch = match name {
            Some(name) => {
                let normalized = but_core::branch::normalize_short_name(name.as_str())?;
                let branch = gix::refs::Category::LocalBranch.to_full_name(normalized.as_bstr())?;
                if repo.try_find_reference(branch.as_ref())?.is_some() {
                    bail!("Branch '{}' already exists", branch.as_bstr());
                }
                branch
            }
            None => unique_canned_refname(&repo)?,
        };

        repo.reference(
            branch.as_ref(),
            target_commit_id,
            PreviousValue::MustNotExist,
            "branch checkout new",
        )
        .with_context(|| format!("Could not create branch '{}'", branch.as_bstr()))?;
        branch
    };

    branch_checkout_with_perm(ctx, branch, perm)
}

/// Switch to the workspace reference
#[but_api(napi, try_from = json::BranchCheckoutResult)]
#[instrument(err(Debug))]
pub fn workspace_checkout(ctx: &mut but_ctx::Context) -> anyhow::Result<BranchCheckoutResult> {
    let mut guard = ctx.exclusive_worktree_access();
    workspace_checkout_with_perm(ctx, guard.write_permission())
}

/// Checks out the GitButler workspace reference under caller-held exclusive repository access.
pub fn workspace_checkout_with_perm(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    let workspace_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
    checkout_ref_with_perm(ctx, workspace_ref, perm)
}

/// Checks out an existing local branch under caller-held exclusive repository
/// access.
///
/// TODO: Decide whether branch checkout should record an oplog snapshot. For
/// now this deliberately performs only the Git checkout and workspace
/// projection rebuild.
pub fn branch_checkout_with_perm(
    ctx: &mut but_ctx::Context,
    branch: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    if !branch.as_bstr().starts_with_str("refs/heads/") {
        bail!(
            "Can only check out local branches under refs/heads, got '{}'",
            branch.as_bstr()
        );
    }

    checkout_ref_with_perm(ctx, branch, perm)
}

fn checkout_ref_with_perm(
    ctx: &mut but_ctx::Context,
    reference_name: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    {
        let repo = ctx.repo.get()?;
        let current_head = repo
            .head_id()
            .context("Cannot check out a branch while HEAD is unborn")?
            .detach();
        let mut reference = repo
            .find_reference(reference_name.as_ref())
            .with_context(|| format!("Could not find ref '{}'", reference_name.as_bstr()))?;
        let target = reference
            .peel_to_id()
            .with_context(|| format!("Could not resolve ref '{}'", reference_name.as_bstr()))?
            .detach();
        let target_commit = repo.find_commit(target).with_context(|| {
            format!(
                "Ref '{}' does not point to a commit",
                reference_name.as_bstr()
            )
        })?;

        safe_checkout_from_head(
            target,
            &repo,
            checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )
        .with_context(|| {
            format!(
                "Could not safely check out '{}' from {current_head} to {target}",
                reference_name.as_bstr()
            )
        })?;
        update_head_reference(
            &repo,
            gix::refs::Target::Symbolic(reference_name.clone()),
            false,
            "checkout",
            reference_name.as_bstr(),
            target_commit.parent_ids().count(),
        )
        .with_context(|| format!("Could not update HEAD to '{}'", reference_name.as_bstr()))?;
    }

    ctx.reload_repo_and_invalidate_workspace(perm)?;
    let mut meta = ctx.meta()?;
    let (repo, ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let workspace =
        WorkspaceState::from_workspace_with_db(&ws, &mut meta, &repo, BTreeMap::new(), &db)?;
    Ok(BranchCheckoutResult { workspace })
}

/// Computes the worktree-visible diff for `branch` in the current workspace.
///
/// `branch` is resolved by name in the repository referenced by `ctx`, and the
/// diff is computed against the current workspace state. For lower-level
/// implementation details, see [`but_workspace::ui::diff::changes_in_branch()`].
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn branch_diff(ctx: &Context, branch: String) -> anyhow::Result<TreeChanges> {
    let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
    let branch = repo.find_reference(&branch)?;
    but_workspace::ui::diff::changes_in_branch(&repo, &ws, branch.name())
}

/// Get the initial upstream integration script for `branch`.
#[but_api(napi, try_from = json::InitialBranchIntegration)]
#[instrument(err(Debug))]
pub fn get_initial_branch_integration(
    ctx: &Context,
    branch: &gix::refs::FullNameRef,
    strategy: Option<json::BranchIntegrationStrategy>,
) -> anyhow::Result<InitialBranchIntegration> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
    let mut ws = ws.clone();
    let strategy = strategy
        .map(BranchIntegrationStrategy::from)
        .unwrap_or_default();
    but_workspace::branch::integrate_branch_upstream::get_initial_integration_steps_for_branch(
        branch, strategy, &mut ws, &mut meta, &repo,
    )
}

/// Apply `integration` to `branch`.
///
/// This acquires exclusive worktree access from `ctx`, applies the integration
/// steps to the branch, and records an oplog snapshot on success. When
/// `dry_run` is enabled, the returned workspace previews the integration
/// result and no oplog entry is persisted.
#[but_api(napi, try_from = json::IntegrateBranchResult)]
#[instrument(err(Debug))]
pub fn apply_branch_integration(
    ctx: &mut but_ctx::Context,
    branch: &gix::refs::FullNameRef,
    integration: json::InteractiveIntegration,
    dry_run: DryRun,
) -> anyhow::Result<IntegrateBranchResult> {
    let integration: InteractiveIntegration = integration.try_into()?;
    let mut guard = ctx.exclusive_worktree_access();
    apply_branch_integration_with_perm(ctx, branch, integration, dry_run, guard.write_permission())
}

/// Apply `integration` to `branch` under caller-held exclusive repository access.
///
/// It prepares a best-effort oplog snapshot, runs the interactive branch
/// integration, and commits the snapshot only if the operation succeeds. The
/// returned [`IntegrateBranchResult`] contains the post-operation workspace
/// view. When `dry_run` is enabled, it returns a preview of the resulting
/// workspace state and skips oplog persistence.
pub fn apply_branch_integration_with_perm(
    ctx: &mut but_ctx::Context,
    branch: &gix::refs::FullNameRef,
    integration: InteractiveIntegration,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<IntegrateBranchResult> {
    branch_mutation_with_snapshot(
        ctx,
        perm,
        OperationKind::GenericBranchUpdate,
        dry_run,
        |ctx, perm| {
            let mut meta = ctx.meta()?;
            let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
            let rebase = but_workspace::branch::integrate_branch_with_steps(
                branch,
                integration,
                &mut ws,
                &mut meta,
                &repo,
            )?;

            Ok(IntegrateBranchResult {
                workspace: WorkspaceState::from_successful_rebase_with_db(
                    rebase, &repo, dry_run, &db,
                )?,
            })
        },
    )
}

/// Moves a branch using the behavior described by [`move_branch_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx`, moves `subject_branch`
/// on top of `target_branch`, and records an oplog snapshot on success. When
/// `dry_run` is enabled, the returned workspace previews the move and no oplog
/// entry is persisted.
#[but_api(napi, try_from = json::MoveBranchResult)]
#[instrument(err(Debug))]
pub fn move_branch(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    target_branch: &gix::refs::FullNameRef,
    dry_run: DryRun,
) -> anyhow::Result<MoveBranchResult> {
    let mut guard = ctx.exclusive_worktree_access();
    move_branch_with_perm(
        ctx,
        subject_branch,
        target_branch,
        dry_run,
        guard.write_permission(),
    )
}

/// Move `subject_branch` on top of `target_branch` under caller-held
/// exclusive repository access and record an oplog snapshot on success.
///
/// It prepares a best-effort move-branch oplog snapshot, rebases the subject
/// branch onto the target branch, updates workspace metadata, and commits the
/// snapshot only if the move succeeds. The returned [`MoveBranchResult`]
/// contains the post-operation workspace view. When `dry_run` is enabled, it
/// returns a preview of the resulting workspace state and skips oplog
/// persistence. For lower-level implementation details, see
/// [`but_workspace::branch::move_branch()`].
pub fn move_branch_with_perm(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    target_branch: &gix::refs::FullNameRef,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveBranchResult> {
    let (result, new_tip) = branch_mutation_with_snapshot(
        ctx,
        perm,
        OperationKind::MoveBranch,
        dry_run,
        |ctx, perm| {
            let mut meta = ctx.meta()?;
            let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo)?;
            let but_workspace::branch::move_branch::Outcome {
                rebase,
                ws_meta,
                new_tip,
                branch_stack_order,
            } = but_workspace::branch::move_branch(editor, subject_branch, target_branch)?;

            let result = MoveBranchResult {
                workspace: branch_workspace_from_rebase(
                    rebase,
                    ws_meta,
                    new_tip.as_ref(),
                    branch_stack_order.as_deref(),
                    &repo,
                    dry_run,
                    &db,
                )?,
            };
            Ok((result, new_tip))
        },
    )?;

    // In single-branch (ad-hoc) mode a reorder can change which branch is at the top of the visible
    // stack. The operation doesn't move `HEAD`, so check out that tip here to keep the whole stack
    // projected (mirroring `create_reference`). Skipped on dry runs.
    let is_dry_run: bool = dry_run.into();
    if let Some(new_tip) = new_tip
        && !is_dry_run
    {
        let checkout = branch_checkout_with_perm(ctx, new_tip, perm)?;
        return Ok(MoveBranchResult {
            workspace: checkout.workspace,
        });
    }

    Ok(result)
}

/// Tears off a branch using the behavior described by [`tear_off_branch_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx`, tears `subject_branch`
/// out of its current stack, and records an oplog snapshot on success. When
/// `dry_run` is enabled, the returned workspace previews the tear-off and no
/// oplog entry is persisted.
#[but_api(napi, try_from = json::MoveBranchResult)]
#[instrument(err(Debug))]
pub fn tear_off_branch(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    dry_run: DryRun,
) -> anyhow::Result<MoveBranchResult> {
    let mut guard = ctx.exclusive_worktree_access();
    tear_off_branch_with_perm(ctx, subject_branch, dry_run, guard.write_permission())
}

/// Removes `subject_branch` from its current stack, creating a new stack for
/// it, under caller-held exclusive repository access.
///
/// It prepares a best-effort tear-off oplog snapshot, performs the tear-off
/// rebase and workspace metadata update under `perm`, and commits the snapshot
/// only if the mutation succeeds. The returned [`MoveBranchResult`] contains
/// the post-operation workspace view. When `dry_run` is enabled, it returns a
/// preview of the resulting workspace state and skips oplog persistence. For
/// lower-level implementation details, see
/// [`but_workspace::branch::tear_off_branch()`].
pub fn tear_off_branch_with_perm(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveBranchResult> {
    branch_mutation_with_snapshot(
        ctx,
        perm,
        OperationKind::TearOffBranch,
        dry_run,
        |ctx, perm| {
            let mut meta = ctx.meta()?;
            let (repo, mut ws, db) = ctx.workspace_mut_and_db_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo)?;
            let but_workspace::branch::move_branch::Outcome {
                rebase, ws_meta, ..
            } = but_workspace::branch::tear_off_branch(editor, subject_branch, None)?;

            Ok(MoveBranchResult {
                workspace: branch_workspace_from_rebase(
                    rebase, ws_meta, None, None, &repo, dry_run, &db,
                )?,
            })
        },
    )
}

fn branch_mutation_with_snapshot<T, F>(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
    operation_kind: OperationKind,
    dry_run: DryRun,
    operation: F,
) -> anyhow::Result<T>
where
    F: FnOnce(&mut but_ctx::Context, &mut RepoExclusive) -> anyhow::Result<T>,
{
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(operation_kind),
        perm.read_permission(),
        dry_run,
    );

    let result = operation(ctx, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && result.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }

    result
}

fn branch_workspace_from_rebase<M: but_core::RefMetadata>(
    mut rebase: SuccessfulRebase<'_, '_, M>,
    ws_meta: Option<but_core::ref_metadata::Workspace>,
    new_tip: Option<&gix::refs::FullName>,
    branch_stack_order: Option<&[gix::refs::FullName]>,
    repo: &gix::Repository,
    dry_run: DryRun,
    db: &but_db::DbHandle,
) -> anyhow::Result<WorkspaceState> {
    if dry_run.into() {
        let entrypoint = new_tip
            .map(|new_tip| -> anyhow::Result<_> {
                Ok((rebase.reference_target(new_tip.as_ref())?, new_tip.clone()))
            })
            .transpose()?;
        let replaced_commits = rebase.history.commit_mappings();
        let workspace = rebase
            .overlayed_graph_with_workspace_overrides(entrypoint, branch_stack_order)?
            .into_workspace()?;
        let (repo, meta) = rebase.repo_and_meta_mut();
        return WorkspaceState::from_workspace_with_db(
            &workspace,
            meta,
            repo,
            replaced_commits,
            db,
        );
    }

    let materialized = rebase.materialize()?;
    if let Some(order) = branch_stack_order {
        materialized.meta.set_branch_stack_order(order)?;
        let project_meta = materialized.workspace.graph.project_meta.clone();
        materialized
            .workspace
            .refresh_from_head(repo, &*materialized.meta, project_meta)?;
    }
    if let Some((ws_meta, ref_name)) = ws_meta.zip(materialized.workspace.ref_name()) {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        materialized.meta.set_workspace(&md)?;
    }

    WorkspaceState::from_workspace_with_db(
        materialized.workspace,
        materialized.meta,
        repo,
        materialized.history.commit_mappings(),
        db,
    )
}
