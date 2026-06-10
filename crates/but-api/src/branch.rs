use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{
    DryRun, sync::RepoExclusive, ui::TreeChanges, worktree::checkout::UncommitedWorktreeChanges,
};
use but_ctx::Context;
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::{Editor, SuccessfulRebase};
use but_workspace::branch::{
    BranchIntegrationStrategy, InitialBranchIntegration, OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
    integrate_branch_upstream::InteractiveIntegration,
};
use tracing::instrument;

/// Outcome after moving a branch.
pub struct MoveBranchResult {
    /// Workspace state after moving or tearing off a branch.
    pub workspace: WorkspaceState,
}

/// Outcome after integrating a branch with an interactive integration plan.
pub struct IntegrateBranchResult {
    /// Workspace state after applying or previewing the integration.
    pub workspace: WorkspaceState,
}

/// JSON transport types for branch APIs.
pub mod json {
    use serde::{Deserialize, Serialize};

    use crate::branch::{
        IntegrateBranchResult as InternalIntegrateBranchResult,
        MoveBranchResult as InternalMoveBranchResult,
    };

    /// JSON sibling of [`but_workspace::branch::apply::Outcome`].
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ApplyOutcome {
        /// Whether the workspace changed while applying the branch.
        pub workspace_changed: bool,
        /// The branches that were actually applied.
        pub applied_branches: Vec<crate::json::FullRefName>,
        /// Whether the workspace reference had to be created.
        pub workspace_ref_created: bool,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ApplyOutcome);

    impl<'a> From<but_workspace::branch::apply::Outcome<'a>> for ApplyOutcome {
        fn from(value: but_workspace::branch::apply::Outcome<'a>) -> Self {
            let workspace_changed = value.workspace_changed();
            let but_workspace::branch::apply::Outcome {
                workspace: _,
                applied_branches,
                workspace_ref_created,
                workspace_merge: _,
                conflicting_stack_ids: _,
            } = value;

            ApplyOutcome {
                workspace_changed,
                applied_branches: applied_branches.into_iter().map(Into::into).collect(),
                workspace_ref_created,
            }
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
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
    let mut guard = ctx.exclusive_worktree_access();
    apply_only_with_perm(ctx, existing_branch, guard.write_permission())
}

/// Applies `existing_branch` to the current workspace under caller-held
/// exclusive repository access.
///
/// It applies the branch with the default workspace-apply options, updates the
/// in-memory workspace stored in `ctx` to the returned workspace state, and
/// returns the apply outcome. This variant does not create an oplog
/// entry. For lower-level implementation details, see
/// [`but_workspace::branch::apply()`].
pub fn apply_only_with_perm(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
    perm: &mut RepoExclusive,
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let out = but_workspace::branch::apply(
        existing_branch,
        &ws,
        &repo,
        &mut meta,
        // NOTE: Options can later be passed as parameter, or we have a separate function for that.
        //       Showing them off here while leaving defaults.
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::default(),
            on_workspace_conflict: OnWorkspaceMergeConflict::default(),
            workspace_reference_naming: WorkspaceReferenceNaming::default(),
            uncommitted_changes: UncommitedWorktreeChanges::default(),
            order: None,
            new_stack_id: None,
        },
    )?
    .into_owned();

    *ws = out.workspace.clone().into_owned();
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
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
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
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
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
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
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
            let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
            let rebase = but_workspace::branch::integrate_branch_with_steps(
                branch,
                integration,
                &mut ws,
                &mut meta,
                &repo,
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
    branch_mutation_with_snapshot(
        ctx,
        perm,
        OperationKind::MoveBranch,
        dry_run,
        |ctx, perm| {
            let mut meta = ctx.meta()?;
            let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo)?;
            let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
                but_workspace::branch::move_branch(editor, subject_branch, target_branch)?;

            Ok(MoveBranchResult {
                workspace: branch_workspace_from_rebase(rebase, ws_meta, &repo, dry_run)?,
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
            let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
            let editor = Editor::create(&mut ws, &mut meta, &repo)?;
            let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
                but_workspace::branch::tear_off_branch(editor, subject_branch, None)?;

            Ok(MoveBranchResult {
                workspace: branch_workspace_from_rebase(rebase, ws_meta, &repo, dry_run)?,
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
    rebase: SuccessfulRebase<'_, '_, M>,
    ws_meta: Option<but_core::ref_metadata::Workspace>,
    repo: &gix::Repository,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceState> {
    if dry_run.into() {
        return WorkspaceState::from_successful_rebase(rebase, repo, dry_run);
    }

    let materialized = rebase.materialize()?;
    if let Some((ws_meta, ref_name)) = ws_meta.zip(materialized.workspace.ref_name()) {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        md.set_project_meta(materialized.workspace.graph.project_meta.clone());
        materialized.meta.set_workspace(&md)?;
    }

    WorkspaceState::from_workspace(
        materialized.workspace,
        repo,
        materialized.history.commit_mappings(),
    )
}
