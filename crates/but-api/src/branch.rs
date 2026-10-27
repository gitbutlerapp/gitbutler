use std::{borrow::Cow, collections::BTreeMap};

use crate::WorkspaceState;
use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{
    DryRun, RefMetadata, WORKSPACE_REF_NAME,
    branch::{refuse_colliding_refname, unique_canned_refname},
    ref_metadata::{ProjectMeta, UnresolvedLegacyTargetRefError},
    sync::{RepoExclusive, RepoShared},
    ui::TreeChanges,
    update_head_reference,
    worktree::{checkout, safe_checkout_from_head},
};
use but_ctx::Context;
use but_error::bail_precondition;
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::{
    Editor, GraphEditorOptions,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::branch::{
    BranchIntegrationStrategy, InitialBranchIntegration,
    integrate_branch_upstream::InteractiveIntegration,
};
use gix::refs::transaction::PreviousValue;
use tracing::instrument;

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
    use bstr::ByteSlice;
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
                conflicting_stacks,
            } = value;

            ApplyOutcome {
                status,
                workspace_changed,
                applied_branches: applied_branches.into_iter().map(Into::into).collect(),
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
        Independent {
            /// Zero-based insertion index, clamped to the stack count. `None` appends.
            order: Option<usize>,
        },
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

    /// JSON transport type for how a branch stack relates to the workspace.
    #[derive(Debug, Clone, Copy, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub enum ListedStackStatus {
        /// The branches are applied to the workspace as a stack.
        Applied,
        /// The branches form a stack known to workspace metadata, but it is not applied.
        Unapplied,
        /// The stack represents the configured target branch.
        Target,
        /// The branches are not related to the workspace.
        Standalone,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ListedStackStatus);

    impl From<but_branches::ListedStackStatus> for ListedStackStatus {
        fn from(value: but_branches::ListedStackStatus) -> Self {
            match value {
                but_branches::ListedStackStatus::Applied => Self::Applied,
                but_branches::ListedStackStatus::Unapplied => Self::Unapplied,
                but_branches::ListedStackStatus::Target => Self::Target,
                but_branches::ListedStackStatus::Standalone => Self::Standalone,
            }
        }
    }

    /// JSON transport type for one or more stacked branches displayed as a unit.
    ///
    /// A branch belongs to exactly one stack, so the tip branch's `refName`
    /// uniquely identifies the stack.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ListedStack {
        /// How the stack relates to the workspace.
        pub status: ListedStackStatus,
        /// The branches of the stack, from the tip (newest) to the base (oldest).
        ///
        /// When flattening branches across stacks, carry `status` along: facts like
        /// "this is the target branch" exist only at the stack level.
        pub branches: Vec<ListedBranch>,
        /// The committer timestamp of the newest branch tip in milliseconds since the epoch.
        pub updated_at_ms: Option<i64>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ListedStack);

    /// JSON transport type for a single listed branch, unifying its local ref and all
    /// remote-tracking refs of the same name.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ListedBranch {
        /// The full name of the primary ref: the local branch if it exists,
        /// otherwise the first remote-tracking branch.
        pub ref_name: crate::json::FullRefName,
        /// The branch name without ref prefix or remote name, for display.
        pub display_name: String,
        /// The commit at the tip of the primary ref.
        pub tip: crate::json::HexHashString,
        /// Whether a local branch of this name exists.
        pub has_local: bool,
        /// All remote-tracking refs that share the branch name.
        pub remote_refs: Vec<crate::json::FullRefName>,
        /// The number of commits this branch adds on its own — from its tip down to
        /// the next listed branch below it in the stack, or to the fork point when
        /// it stands alone. This is the count to show as the branch's own commits.
        ///
        /// A per-branch commit history that is not stack-aware (such as the one
        /// from `branchDetails`) lists every commit down to the target, so a stacked
        /// branch's own commits are the first `commitCount` of it. For the same
        /// reason `commitCount` is smaller than `commitsAheadOfTarget`, which also
        /// counts the commits of the branches below.
        ///
        /// `null` if the commit graph ends before the fork point, as in a shallow
        /// clone; an exact-looking number would be wrong.
        pub commit_count: Option<usize>,
        /// The number of commits reachable from this branch but not from the target
        /// branch. For a branch stacked on top of others this includes the commits
        /// of those lower branches, so it measures the branch's distance from the
        /// target rather than its own contribution — use `commitCount` for that.
        ///
        /// `null` when no target is configured, or when the connection to the
        /// target lies beyond the end of the traversed graph, as in a shallow clone.
        pub commits_ahead_of_target: Option<usize>,
        /// The author of the tip commit.
        pub last_author: Option<but_workspace::ui::Author>,
        /// The committer timestamp of the tip commit in milliseconds since the epoch.
        pub updated_at_ms: Option<i64>,
        /// The cached review (PR/MR) published for this branch, if any is known.
        pub review: Option<ListedForgeReview>,
        /// The lifecycle state of `review`, derived server-side so every client
        /// agrees on the precedence: merged wins over closed, closed over draft,
        /// draft over open. `null` exactly when `review` is `null`.
        pub review_status: Option<BranchReviewStatus>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ListedBranch);

    /// JSON transport type for the cached review (PR/MR) of a listed branch.
    ///
    /// Only what a branch listing displays. The full review is available from the
    /// per-review APIs; carrying it here would put every review body in a response
    /// that lists the whole repository. Lifecycle state lives in `reviewStatus`
    /// rather than being re-derived from timestamps.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ListedForgeReview {
        /// The number identifying the review within its repository, e.g. `123`.
        pub number: i64,
        /// The title of the review.
        pub title: String,
        /// The URL to view the review in a web browser.
        pub html_url: String,
        /// The forge's symbol for this review type, e.g. `#` for GitHub pull
        /// requests and `!` for GitLab merge requests. Precedes `number` when
        /// displayed.
        pub unit_symbol: String,
        /// Labels used to categorize the review.
        pub labels: Vec<but_forge::ForgeReviewLabel>,
        /// The review author, which may differ from the latest commit author.
        pub author: Option<ListedForgeReviewAuthor>,
        /// ISO 8601 timestamp of when the review was opened.
        pub created_at: Option<String>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ListedForgeReview);

    /// Author identity used to display and search listed reviews.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ListedForgeReviewAuthor {
        /// The author's username on the forge.
        pub login: String,
        /// The author's display name, if available.
        pub name: Option<String>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ListedForgeReviewAuthor);

    impl From<&but_forge::ForgeReview> for ListedForgeReview {
        fn from(value: &but_forge::ForgeReview) -> Self {
            Self {
                number: value.number,
                title: value.title.clone(),
                html_url: value.html_url.clone(),
                unit_symbol: value.unit_symbol.clone(),
                labels: value.labels.clone(),
                author: value.author.as_ref().map(|author| ListedForgeReviewAuthor {
                    login: author.login.clone(),
                    name: author.name.clone(),
                }),
                created_at: value.created_at.clone(),
            }
        }
    }

    /// JSON transport type for the lifecycle state of a branch's cached review.
    #[derive(Debug, Clone, Copy, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub enum BranchReviewStatus {
        /// The review is open and ready for review.
        Open,
        /// The review is open but marked as a draft.
        Draft,
        /// The review was merged.
        Merged,
        /// The review was closed without merging.
        Closed,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchReviewStatus);

    impl From<super::ListedStack> for ListedStack {
        fn from(value: super::ListedStack) -> Self {
            Self {
                status: value.status.into(),
                branches: value.branches.into_iter().map(Into::into).collect(),
                updated_at_ms: value.updated_at_ms,
            }
        }
    }

    impl From<super::ListedBranch> for ListedBranch {
        fn from(value: super::ListedBranch) -> Self {
            let super::ListedBranch { branch, review } = value;
            let review_status = review.as_ref().map(|review| {
                if review.is_merged() {
                    BranchReviewStatus::Merged
                } else if review.closed_at.is_some() {
                    BranchReviewStatus::Closed
                } else if review.draft {
                    BranchReviewStatus::Draft
                } else {
                    BranchReviewStatus::Open
                }
            });
            Self {
                ref_name: branch.ref_name.into(),
                display_name: branch.display_name.to_str_lossy().into_owned(),
                tip: branch.tip.into(),
                has_local: branch.has_local,
                remote_refs: branch.remote_refs.into_iter().map(Into::into).collect(),
                commit_count: branch.commit_count,
                commits_ahead_of_target: branch.commits_ahead_of_target,
                last_author: branch.last_author.map(Into::into),
                updated_at_ms: branch.updated_at_ms,
                review: review.as_ref().map(Into::into),
                review_status,
            }
        }
    }

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
    let (repo, ws, _) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let out = but_workspace::branch::apply(
        existing_branch,
        ws.clone(),
        &repo,
        &mut meta,
        // NOTE: Options can later be passed as parameter, or we have a separate function for that.
        //       Showing them off here while leaving defaults.
        but_workspace::branch::apply::Options {
            new_stack_id: None,
            allow_applying_already_applied_branch_when_outside_workspace: false,
        },
    )?;

    if out.status.persisted_mutation() {
        drop(repo);
        drop(ws);
        ctx.update_workspace_cache(out.workspace.clone());
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

/// Returns an unused short branch name derived from the configured Git author,
/// like `jd-branch-1`.
///
/// This is the very name [`branch_create()`] falls back to when `new_ref` is
/// omitted, so callers can show the branch they are about to create before it
/// exists. Uniqueness only holds at the time of the call: the name is
/// deduplicated against local branches and the short names of remote-tracking
/// branches, both of which can change afterwards.
#[but_api(napi, provides = [Branches])]
#[instrument(err(Debug))]
pub fn branch_canned_name(ctx: &Context) -> anyhow::Result<String> {
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    Ok(unique_canned_refname(&repo)?.shorten().to_string())
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

    let (anchor, order) = match placement {
        json::BranchCreatePlacement::Independent { order } => (None, order),
        json::BranchCreatePlacement::Dependent { relative_to, side } => {
            let position = match side {
                InsertSide::Above => Position::Above,
                InsertSide::Below => Position::Below,
            };
            let anchor = match relative_to {
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
            };
            (Some(anchor), None)
        }
    };

    let new_ref = if let Some(new_ref) = new_ref {
        new_ref
    } else {
        let repo = ctx.repo.get()?;
        unique_canned_refname(&repo)?
    };

    // The snapshot lists `new_ref` so that undo deletes it again: left behind, it would join
    // a stack on the next change.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateBranch)
            .with_trailers([Trailer::Name(new_ref.to_string())]),
        &[new_ref.as_ref()],
        perm.read_permission(),
        DryRun::No,
    );
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    but_workspace::branch::create_reference(editor, new_ref.as_ref(), anchor, order)?
        .materialize(Default::default())?;
    drop((repo, db, meta));
    ctx.update_workspace_cache(ws);

    if let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }

    let mut meta = ctx.meta()?;
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let workspace = WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        BTreeMap::new(),
        Vec::new(),
        &mut db,
    )?;
    drop((ws, repo, db, meta));
    Ok(BranchCreateResult { workspace, new_ref })
}

/// Names the anonymous segment whose top commit is `commit_id` as `new_ref`, as
/// seen from `checkout`, or with a canned name if `new_ref` is omitted.
///
/// A checkout detached at `commit_id` is attached to the new branch; anywhere
/// else the branch is created right above `commit_id`. This acquires exclusive
/// worktree access from `ctx` and records an oplog snapshot on success. For
/// lower-level implementation details, see
/// [`but_workspace::branch::name_segment()`].
#[but_api(
    napi,
    try_from = json::BranchCreateResult,
    invalidates = [Branches, Workspace]
)]
#[instrument(err(Debug))]
pub fn branch_name_segment(
    ctx: &mut but_ctx::Context,
    checkout: crate::commit::json::ChangesSource,
    commit_id: gix::ObjectId,
    #[but_api(crate::json::MaybeLossyFullNameRef)] new_ref: Option<gix::refs::FullName>,
) -> anyhow::Result<BranchCreateResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_name_segment_with_perm(ctx, checkout, commit_id, new_ref, guard.write_permission())
}

/// Name the anonymous segment whose top commit is `commit_id` under caller-held
/// exclusive repository access and record an oplog snapshot on success.
///
/// See [`branch_name_segment()`] for the higher-level behaviour.
pub fn branch_name_segment_with_perm(
    ctx: &mut but_ctx::Context,
    checkout: crate::commit::json::ChangesSource,
    commit_id: gix::ObjectId,
    new_ref: Option<gix::refs::FullName>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCreateResult> {
    let checkout = crate::worktrees::worktree_kind(ctx, &checkout)?;
    let new_ref = match new_ref {
        Some(new_ref) => new_ref,
        None => unique_canned_refname(&*ctx.repo.get()?)?,
    };

    // The snapshot lists `new_ref` so that undo deletes it again. Not when a linked worktree
    // gets it checked out: undo can't restore that worktree, and won't delete a ref it has.
    let undone_ref = matches!(checkout, but_graph::WorktreeKind::Main).then_some(new_ref.as_ref());
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateBranch)
            .with_trailers([Trailer::Name(new_ref.to_string())]),
        undone_ref.as_slice(),
        perm.read_permission(),
        DryRun::No,
    );
    {
        let mut meta = ctx.meta()?;
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        but_workspace::branch::name_segment(editor, &checkout, commit_id, new_ref.as_ref())?
            .materialize(Default::default())?;
    }

    ctx.reload_repo_and_invalidate_workspace(perm)?;
    if let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }
    let mut meta = ctx.meta()?;
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let workspace = WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        BTreeMap::new(),
        Vec::new(),
        &mut db,
    )?;
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
/// reverse of creating an empty branch above the checked-out one). Like unapply,
/// leaving a managed workspace with one lane checks out that lane, or with none
/// the local target branch, and deletes the workspace reference. For lower-level
/// implementation details, see
/// [`but_workspace::branch::remove_reference()`].
#[but_api(
    napi,
    try_from = json::BranchRemoveResult,
    invalidates = [Branches, Workspace]
)]
#[instrument(err(Debug))]
pub fn branch_remove(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::FullNameBytes)] ref_name: gix::refs::FullName,
) -> anyhow::Result<BranchRemoveResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_remove_with_perm(ctx, ref_name, guard.write_permission())
}

/// Remove the local branch `ref_name`, whether or not it is part of the current
/// workspace projection, under caller-held exclusive repository access and
/// record an oplog snapshot on success.
///
/// See [`branch_remove()`] for the checked-out-reference behaviour and
/// [`but_workspace::branch::remove_reference()`] for the lower-level deletion.
pub fn branch_remove_with_perm(
    ctx: &mut but_ctx::Context,
    ref_name: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchRemoveResult> {
    if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
        bail!(
            "Can only delete local branches under refs/heads, got '{}'",
            ref_name.as_bstr()
        );
    }
    if but_branches::is_technical_branch_name(ref_name.shorten()) {
        bail!(
            "Cannot delete GitButler technical branch '{}'",
            ref_name.shorten()
        );
    }

    // Name the branch so undo recreates it even when it isn't in the workspace.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::DeleteBranch)
            .with_trailers([Trailer::Name(ref_name.to_string())]),
        &[ref_name.as_ref()],
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
    let target = ctx.project_meta().ok().and_then(|meta| meta.target_ref);
    let move_head_to = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        let is_checked_out = repo
            .head_name()
            .ok()
            .flatten()
            .is_some_and(|head| head == ref_name);
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
            // Made on top of the checked-out target, as `git checkout -b` from it, the
            // target is the workspace's base rather than a segment underneath.
            let target_at_head = target
                .map(|target| target.as_local_full_name_ref().to_owned())
                .filter(|target| *target != ref_name)
                .filter(|target| {
                    let tip = (repo.find_reference(target.as_ref()).ok())
                        .and_then(|mut r| r.peel_to_id().ok());
                    tip.is_some_and(|tip| repo.head_id().is_ok_and(|head| head == tip))
                });
            let below = stack.segments[idx + 1..]
                .iter()
                .find_map(|s| s.ref_name().map(|r| r.to_owned()))
                .or(target_at_head);
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

    let moved_head = move_head_to.is_some();
    if let Some(below) = move_head_to {
        // Land `HEAD` on the reference underneath. The old tip now sits above
        // the entrypoint and is no longer part of the downward projection.
        branch_checkout_with_perm_only(ctx, below, perm)?;
    }

    let mut meta = ctx.meta()?;
    let (mut repo, ws, _) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let new_ws = if moved_head {
        None
    } else {
        but_workspace::branch::remove_reference(
            ref_name.as_ref(),
            &mut repo,
            &ws,
            &mut meta,
            but_workspace::branch::remove_reference::Options {
                avoid_anonymous_stacks: true,
                keep_metadata: false,
            },
        )?
    };
    let changed = if let Some(new_ws) = new_ws {
        // Like unapply, leave a workspace that removing the branch left without purpose.
        let new_ws = but_workspace::branch::dissolve_unnecessary_workspace(
            &new_ws,
            &repo,
            &meta,
            Default::default(),
        )?
        .map_or(new_ws, |(dissolved, _)| dissolved);
        drop(ws);
        drop(repo);
        ctx.update_workspace_cache(new_ws);
        true
    } else {
        // Standalone branches are intentionally absent from the workspace
        // projection, as is a checked-out tip after moving HEAD below it.
        let deleted_ref = but_workspace::branch::remove_reference::delete_local_branch(
            &mut repo,
            ref_name.as_ref(),
        )?;
        let deleted_meta = meta.remove(ref_name.as_ref())?;
        if deleted_ref || deleted_meta {
            let new_ws = ws
                .graph
                .redo_traversal_with_overlay(&repo, &meta, Default::default())?
                .into_workspace()?;
            drop(ws);
            drop(repo);
            ctx.update_workspace_cache(new_ws);
            true
        } else {
            drop(ws);
            drop(repo);
            false
        }
    };
    drop(meta);

    if changed && let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }
    let mut meta = ctx.meta()?;
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let workspace = WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        BTreeMap::new(),
        Vec::new(),
        &mut db,
    )?;
    Ok(BranchRemoveResult { workspace })
}

/// Renames the local branch `ref_name` to `new_name`, moving its git reference and
/// its metadata (including its `branch_order` entry) and re-pointing the `HEAD`
/// of every worktree that has it checked out at the new name.
///
/// `new_name` is a short branch name that is normalized into a valid
/// `refs/heads/<name>` reference before the rename, so callers don't have to
/// pre-normalize it. This acquires exclusive worktree access from `ctx`, records
/// an oplog snapshot on success, and returns the post-operation workspace view.
/// It requires no stack id and works in both managed and ad-hoc/single-branch
/// workspaces.
#[but_api(
    napi,
    try_from = json::BranchRenameResult,
    invalidates = [Branches, Workspace]
)]
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
    if ref_name == new_ref {
        let mut meta = ctx.meta()?;
        let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        repo.find_reference(ref_name.as_ref())
            .with_context(|| format!("Branch '{}' does not exist", ref_name.shorten()))?;
        let workspace = WorkspaceState::from_workspace_with_db(
            &ws,
            &mut meta,
            &repo,
            BTreeMap::new(),
            Vec::new(),
            &mut db,
        )?;
        return Ok(BranchRenameResult { workspace, new_ref });
    }

    // The snapshot lists `new_ref` so that undo deletes it again, and `ref_name` so that undo
    // brings it back even when no applied stack holds it, as with the target branch. Not when a
    // linked worktree has the branch checked out: it follows the rename, and undo won't delete a
    // ref it has. Nor when one name is the other's directory, as `foo` and `foo/bar`: undo
    // recreates the old name before it deletes the new one, which git can't hold at once, and
    // would lose both.
    let nested = |inner: &gix::refs::FullName, outer: &gix::refs::FullName| {
        let inner = inner.as_bstr();
        inner.starts_with(outer.as_bstr()) && inner.get(outer.as_bstr().len()) == Some(&b'/')
    };
    let in_linked_worktree = {
        let repo = ctx.repo.get()?;
        let worktrees = but_core::branch::SafeDelete::new(&repo)?;
        repo.try_find_reference(ref_name.as_ref())?
            .and_then(|reference| worktrees.worktree_dirs_with_ref(&reference))
            .is_some_and(|dirs| dirs.iter().any(|dir| Some(dir.as_path()) != repo.workdir()))
    };
    let names = [ref_name.as_ref(), new_ref.as_ref()];
    let listed: &[&gix::refs::FullNameRef] =
        match !in_linked_worktree && !nested(&new_ref, &ref_name) && !nested(&ref_name, &new_ref) {
            true => &names,
            false => &[],
        };
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::UpdateBranchName).with_trailers([
            // Distinct keys (`previous_name` / `name`) so both survive `.find(key)` lookups on the
            // consumer side; two `Name` trailers would collide and drop the second.
            Trailer::PreviousName(ref_name.to_string()),
            Trailer::Name(new_ref.to_string()),
        ]),
        listed,
        perm.read_permission(),
        DryRun::No,
    );

    {
        let mut meta = ctx.meta()?;
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;

        repo.find_reference(ref_name.as_ref())
            .with_context(|| format!("Branch '{}' does not exist", ref_name.shorten()))?;
        if repo.try_find_reference(new_ref.as_ref())?.is_some()
            || meta.branch_opt(new_ref.as_ref())?.is_some()
            || meta.branch_stack_order(new_ref.as_ref())?.is_some()
        {
            bail_precondition!("A branch named '{}' already exists", new_ref.shorten());
        }
        refuse_colliding_refname(&repo, new_ref.as_ref(), Some(ref_name.as_ref()))?;

        let options = GraphEditorOptions {
            extra_mutable_refs: vec![ref_name.clone()],
            ..Default::default()
        };
        let mut editor = Editor::create_with_opts(&mut ws, &mut meta, &repo, &mut db, &options)?;
        editor.replace_reference(editor.select_reference(ref_name.as_ref())?, new_ref.clone())?;
        let mut rebase = editor.rebase()?;
        let (_, meta) = rebase.repo_and_meta_mut();
        meta.rename(ref_name.as_ref(), new_ref.as_ref())?;
        rebase.materialize(Default::default())?;
    }

    // Rebuild the workspace from scratch: this re-reads HEAD, so the moved-HEAD case needs no
    // stale-entrypoint handling.
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    if let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm).ok();
    }
    let mut meta = ctx.meta()?;
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let workspace = WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        BTreeMap::new(),
        Vec::new(),
        &mut db,
    )?;
    Ok(BranchRenameResult { workspace, new_ref })
}

/// Checks out a branch and returns the resulting workspace state.
///
/// This acquires exclusive worktree access from `ctx`, updates the worktree and
/// index through [`but_core::worktree::safe_checkout_from_head()`], then points `HEAD`
/// symbolically at `branch`. The branch is either an existing full local branch
/// name under `refs/heads/`, or a remote-tracking branch under `refs/remotes/`,
/// in which case its local tracking branch is checked out, created at the
/// remote-tracking commit first if it doesn't exist yet.
#[but_api(napi, try_from = json::BranchCheckoutResult)]
#[instrument(err(Debug))]
pub fn branch_checkout(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::json::FullNameBytes)] branch: gix::refs::FullName,
) -> anyhow::Result<BranchCheckoutResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_checkout_with_perm(ctx, branch, guard.write_permission())
}

/// Creates a new local branch at the selected project cutoff, checks it out, and
/// returns the resulting workspace state.
///
/// If `name` is provided, it is treated as a short branch name and normalized
/// before creating `refs/heads/<name>`. If omitted, a unique canned branch name
/// is generated. The resulting branch must not already exist. Records an oplog
/// snapshot on success, which undo restores by deleting the branch again.
#[but_api(napi, try_from = json::BranchCheckoutResult)]
#[instrument(err(Debug))]
pub fn branch_checkout_new(
    ctx: &mut but_ctx::Context,
    name: Option<String>,
) -> anyhow::Result<BranchCheckoutResult> {
    let mut guard = ctx.exclusive_worktree_access();
    branch_checkout_new_with_perm(ctx, name, guard.write_permission())
}

/// Creates a new local branch at the selected project cutoff and checks it out under
/// caller-held exclusive repository access, recording an oplog snapshot on success.
pub fn branch_checkout_new_with_perm(
    ctx: &mut but_ctx::Context,
    name: Option<String>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    let project_meta = ctx.project_meta()?;
    let (branch, target_commit_id) = {
        let repo = ctx.repo.get()?;
        let target_commit_id = project_meta.resolve_target_commit(&repo)?;
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
        (branch, target_commit_id)
    };
    // The snapshot lists the new branch so that undo deletes it again, as `branch_create` does.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::SwitchBranch),
        &[branch.as_ref()],
        perm.read_permission(),
        but_core::DryRun::No,
    );
    ctx.repo
        .get()?
        .reference(
            branch.as_ref(),
            target_commit_id,
            PreviousValue::MustNotExist,
            "branch checkout new",
        )
        .with_context(|| format!("Could not create branch '{}'", branch.as_bstr()))?;

    let result = branch_checkout_with_perm_only(ctx, branch, perm)?;
    if let Some(snapshot) = maybe_oplog_entry {
        _ = snapshot.commit(ctx, perm);
    }
    Ok(result)
}

/// Switch to the workspace reference
#[but_api(napi, try_from = json::BranchCheckoutResult)]
#[instrument(err(Debug))]
pub fn workspace_checkout(ctx: &mut but_ctx::Context) -> anyhow::Result<BranchCheckoutResult> {
    let mut guard = ctx.exclusive_worktree_access();
    workspace_checkout_with_perm(ctx, guard.write_permission())
}

/// Switch to the workspace reference under caller-held exclusive repository access.
pub fn workspace_checkout_with_perm(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    let snapshot_details = SnapshotDetails::new(OperationKind::SwitchToWorkspace);
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        snapshot_details,
        perm.read_permission(),
        but_core::DryRun::No,
    );

    let result = workspace_checkout_with_perm_only(ctx, perm)?;

    if let Some(snapshot) = maybe_oplog_entry {
        _ = snapshot.commit(ctx, perm);
    }

    Ok(result)
}

/// Checks out the GitButler workspace reference under caller-held exclusive repository access.
pub fn workspace_checkout_with_perm_only(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    let workspace_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
    branch_checkout_with_perm_only(ctx, workspace_ref, perm)
}

/// Checks out a branch under caller-held exclusive repository access.
///
/// See [`branch_checkout()`] for the accepted branch names.
pub fn branch_checkout_with_perm(
    ctx: &mut but_ctx::Context,
    branch: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    let snapshot_details = SnapshotDetails::new(OperationKind::SwitchBranch);
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        snapshot_details,
        perm.read_permission(),
        but_core::DryRun::No,
    );

    let result = branch_checkout_with_perm_only(ctx, branch, perm)?;

    if let Some(snapshot) = maybe_oplog_entry {
        _ = snapshot.commit(ctx, perm);
    }

    Ok(result)
}

/// Checks out a branch under caller-held exclusive repository access without
/// creating an oplog entry.
///
/// See [`branch_checkout()`] for the accepted branch names.
pub fn branch_checkout_with_perm_only(
    ctx: &mut but_ctx::Context,
    reference_name: gix::refs::FullName,
    perm: &mut RepoExclusive,
) -> anyhow::Result<BranchCheckoutResult> {
    {
        let repo = ctx.repo.get()?;
        let reference_name = match reference_name.category() {
            Some(gix::refs::Category::LocalBranch) => reference_name,
            Some(gix::refs::Category::RemoteBranch) => {
                but_workspace::branch::local_tracking_branch(&repo, reference_name.as_ref())?
            }
            _ => bail!(
                "Can only check out local branches under refs/heads or remote-tracking branches under refs/remotes, got '{}'",
                reference_name.as_bstr()
            ),
        };
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
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let workspace = WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        BTreeMap::new(),
        Vec::new(),
        &mut db,
    )?;
    Ok(BranchCheckoutResult { workspace })
}

/// Computes the worktree-visible diff for `branch` in the current workspace.
///
/// `branch` is resolved by name in the repository referenced by `ctx`, and the
/// diff is computed against the current workspace state. For lower-level
/// implementation details, see [`but_workspace::ui::diff::changes_in_branch()`].
#[but_api(napi, provides = [Branches])]
#[instrument(err(Debug))]
pub fn branch_diff(ctx: &Context, branch: String) -> anyhow::Result<TreeChanges> {
    let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
    let branch = repo.find_reference(&branch)?;
    but_workspace::ui::diff::changes_in_branch(&repo, &ws, branch.name())
}

/// Lists all local and remote branches of the project, grouped into stacks where
/// known, and enriched with cached review (PR/MR) information.
///
/// One ref enumeration, one graph traversal, and one forge-cache read produce the
/// entire response: no network requests and no diffs are computed. Branches applied
/// to the workspace are included and classified rather than dropped. Stacks are
/// ordered most recently updated first; group by `status` to lead with the
/// workspace-related ones. Ahead-counts are relative to the
/// project's configured target branch, which clients know from the project APIs.
#[but_api(napi, json::ListedStack, provides = [Branches])]
#[instrument(err(Debug))]
pub fn branch_list(ctx: &Context) -> anyhow::Result<Vec<ListedStack>> {
    let guard = ctx.shared_worktree_access();
    branch_list_with_perm(ctx, guard.read_permission())
}

/// See [`branch_list()`]; this variant uses the caller's existing worktree access.
pub fn branch_list_with_perm(
    ctx: &Context,
    _perm: &RepoShared,
) -> anyhow::Result<Vec<ListedStack>> {
    let meta = ctx.meta()?;
    let project_meta = match ctx.project_meta() {
        Ok(project_meta) => project_meta,
        // Listing needs no target, and the listed branches are what one is chosen from to
        // replace a legacy target that doesn't resolve.
        Err(err) if err.is::<UnresolvedLegacyTargetRefError>() => ProjectMeta::default(),
        Err(err) => return Err(err),
    };
    let listing = {
        let repo = ctx.repo.get()?;
        let mut db = ctx.db.get_cache_mut()?;
        but_branches::list(
            &repo,
            &meta,
            &mut db,
            but_branches::Options {
                project_meta,
                hard_limit: None,
            },
        )?
    };

    let db = ctx.db.get_cache()?;
    let mut reviews_by_head = but_forge::reviews_by_head(&db)?;
    Ok(listing
        .stacks
        .into_iter()
        .map(|stack| ListedStack {
            status: stack.status,
            updated_at_ms: stack.updated_at_ms,
            branches: stack
                .branches
                .into_iter()
                .map(|branch| {
                    let review =
                        reviews_by_head.remove(branch.display_name.to_str_lossy().as_ref());
                    ListedBranch { branch, review }
                })
                .collect(),
        })
        .collect())
}

/// A listed stack enriched with cached forge review data.
#[derive(Debug)]
pub struct ListedStack {
    /// How the stack relates to the workspace.
    pub status: but_branches::ListedStackStatus,
    /// The branches in the stack, from tip to base.
    pub branches: Vec<ListedBranch>,
    /// The committer timestamp of the newest branch tip.
    pub updated_at_ms: Option<i64>,
}

/// A listed branch and its cached forge review, if present.
#[derive(Debug)]
pub struct ListedBranch {
    /// The branch listing data.
    pub branch: but_branches::ListedBranch,
    /// The cached review associated with the branch.
    pub review: Option<but_forge::ForgeReview>,
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
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let mut ws = ws.clone();
    let strategy = strategy
        .map(BranchIntegrationStrategy::from)
        .unwrap_or_default();
    but_workspace::branch::integrate_branch_upstream::get_initial_integration_steps_for_branch(
        branch, strategy, &mut ws, &mut meta, &repo, &mut db,
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
            let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
            let rebase = but_workspace::branch::integrate_branch_with_steps(
                branch,
                integration,
                &mut ws,
                &mut meta,
                &repo,
                &mut db,
            )?;

            Ok(IntegrateBranchResult {
                workspace: WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?,
            })
        },
    )
}

/// Moves a branch using the behavior described by [`move_branch_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx`, moves `subject_branch`
/// to `side` of `relative_to`, and records an oplog snapshot on success. When
/// `dry_run` is enabled, the returned workspace previews the move and no oplog
/// entry is persisted.
#[but_api(napi, try_from = json::MoveBranchResult)]
#[instrument(err(Debug))]
pub fn move_branch(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
) -> anyhow::Result<MoveBranchResult> {
    let mut guard = ctx.exclusive_worktree_access();
    move_branch_with_perm(
        ctx,
        subject_branch,
        relative_to,
        side,
        dry_run,
        guard.write_permission(),
    )
}

/// Move `subject_branch` to `side` of `relative_to` under caller-held
/// exclusive repository access and record an oplog snapshot on success.
///
/// The branch may come from a stack of the workspace or from a linked
/// worktree, and `relative_to` may name a branch or a commit of either, so
/// the same call moves a branch within the workspace, into a worktree, or
/// out of one.
///
/// It prepares a best-effort move-branch oplog snapshot, rebases the subject
/// branch to its new place, updates workspace metadata, and commits the
/// snapshot only if the move succeeds. The returned [`MoveBranchResult`]
/// contains the post-operation workspace view. When `dry_run` is enabled, it
/// returns a preview of the resulting workspace state and skips oplog
/// persistence. For lower-level implementation details, see
/// [`but_workspace::branch::move_branch()`].
pub fn move_branch_with_perm(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    relative_to: RelativeTo,
    side: InsertSide,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<MoveBranchResult> {
    branch_mutation_with_snapshot(
        ctx,
        perm,
        OperationKind::MoveBranch,
        dry_run,
        |ctx, perm| {
            let mut meta = ctx.meta()?;
            let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
            let rebase =
                but_workspace::branch::move_branch(editor, subject_branch, relative_to, side)?;
            Ok(MoveBranchResult {
                workspace: WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?,
            })
        },
    )
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
            let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
            let rebase = but_workspace::branch::tear_off_branch(editor, subject_branch)?;
            Ok(MoveBranchResult {
                workspace: WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?,
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
