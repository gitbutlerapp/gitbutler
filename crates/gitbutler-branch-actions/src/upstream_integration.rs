use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_core::{Reference, RepositoryExt};
use but_ctx::{Context, access::RepoExclusive};
use but_rebase::{RebaseOutput, RebaseStep};
use but_serde::BStringForFrontend;
use but_workspace::{legacy::stack_ext::StackDetailsExt, ref_info::Options};
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_repo::{first_parent_commit_ids_until, rebase::merge_commits};
use gitbutler_stack::{StackId, Target, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes};
use gix::merge::tree::TreatAsUnresolved;
use serde::{Deserialize, Serialize};

use crate::{BranchManagerExt, VirtualBranchesExt as _};

#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NameAndStatus {
    pub name: String,
    pub status: BranchStatus,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NameAndStatus);

#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    pub tree_status: UpstreamTreeStatus,
    pub branch_statuses: Vec<NameAndStatus>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackStatus);

#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum UpstreamTreeStatus {
    SafelyUpdatable,
    Conflicted,
    Empty,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UpstreamTreeStatus);

#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BranchStatus {
    SafelyUpdatable,
    Integrated,
    Conflicted {
        /// If the branch can be rebased onto the target without conflicts
        rebasable: bool,
    },
    Empty,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BranchStatus);

#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum StackStatuses {
    UpToDate,
    UpdatesRequired {
        #[serde(rename = "worktreeConflicts")]
        #[cfg_attr(feature = "export-schema", schemars(with = "Vec<String>"))]
        worktree_conflicts: Vec<BStringForFrontend>,
        #[cfg_attr(
            feature = "export-schema",
            schemars(with = "Vec<(Option<String>, StackStatus)>")
        )]
        statuses: Vec<(Option<StackId>, StackStatus)>,
    },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackStatuses);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BaseBranchResolutionApproach {
    Rebase,
    Merge,
    HardReset,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BaseBranchResolutionApproach);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum ResolutionApproach {
    Rebase,
    Merge,
    Unapply,
    Delete,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ResolutionApproach);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct BaseBranchResolution {
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    target_commit_oid: gix::ObjectId,
    approach: BaseBranchResolutionApproach,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BaseBranchResolution);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct IntegrationOutcome {
    /// The list of branches that have been deleted as a result of the upstream integration
    deleted_branches: Vec<String>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(IntegrationOutcome);

impl StackStatus {
    fn create(
        tree_status: UpstreamTreeStatus,
        branch_statuses: Vec<NameAndStatus>,
    ) -> Result<Self> {
        if branch_statuses.is_empty() {
            bail!("Branch statuses must not be empty")
        }

        Ok(Self {
            tree_status,
            branch_statuses,
        })
    }

    fn resolution_acceptable(&self, approach: &ResolutionApproach) -> bool {
        if self.tree_status == UpstreamTreeStatus::Empty
            && self
                .branch_statuses
                .iter()
                .all(|branch_status| branch_status.status == BranchStatus::Integrated)
        {
            return matches!(
                approach,
                ResolutionApproach::Unapply | ResolutionApproach::Delete
            );
        }

        if self.is_single() {
            matches!(
                approach,
                ResolutionApproach::Merge
                    | ResolutionApproach::Rebase
                    | ResolutionApproach::Unapply
            )
        } else {
            matches!(
                approach,
                ResolutionApproach::Rebase | ResolutionApproach::Unapply
            )
        }
    }

    fn is_single(&self) -> bool {
        self.branch_statuses.len() == 1
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::stack_id")
    )]
    pub stack_id: StackId,
    pub approach: ResolutionApproach,
    pub delete_integrated_branches: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(Resolution);

enum IntegrationResult {
    UpdatedObjects {
        head: gix::ObjectId,
        rebase_output: Option<RebaseOutput>,
        for_archival: Vec<Reference>,
    },
    UnapplyBranch,
    DeleteBranch,
}

pub struct UpstreamIntegrationContext<'a> {
    _permission: Option<&'a mut RepoExclusive>,
    ctx: &'a Context,
    stacks_in_workspace: Vec<but_workspace::legacy::ui::StackEntry>,
    new_target: gix::ObjectId,
    target: Target,
    gix_repo: &'a gix::Repository,
    review_map: &'a HashMap<String, but_forge::ForgeReview>,
}

impl<'a> UpstreamIntegrationContext<'a> {
    pub(crate) fn open(
        ctx: &'a Context,
        target_commit_oid: Option<gix::ObjectId>,
        permission: &'a mut RepoExclusive,
        gix_repo: &'a gix::Repository,
        review_map: &'a HashMap<String, but_forge::ForgeReview>,
    ) -> Result<Self> {
        {
            let meta = ctx.meta()?;
            let repo = ctx.repo.get()?;
            let mut cache = ctx.cache.get_cache_mut()?;
            let _ref_info = but_workspace::head_info(
                &repo,
                &meta,
                Options {
                    expensive_commit_info: true,
                    traversal: but_graph::init::Options::limited(),
                },
                &mut cache,
            )?;
        }

        let virtual_branches_handle = ctx.virtual_branches();
        let target = virtual_branches_handle.get_default_target()?;
        let new_target = match target_commit_oid {
            Some(oid) => oid,
            None => {
                gix_repo
                    .find_reference(&target.branch.to_string())?
                    .peel_to_commit()?
                    .id
            }
        };

        let stacks_in_workspace = stacks(ctx, gix_repo)?;

        Ok(Self {
            _permission: Some(permission),
            new_target,
            target: target.clone(),
            stacks_in_workspace,
            ctx,
            gix_repo,
            review_map,
        })
    }
}

fn stacks(
    ctx: &Context,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<but_workspace::legacy::ui::StackEntry>> {
    let meta = ctx.legacy_meta()?;
    let mut cache = ctx.cache.get_cache_mut()?;
    but_workspace::legacy::stacks_v3(
        repo,
        &meta,
        but_workspace::legacy::StacksFilter::InWorkspace,
        None,
        &mut cache,
    )
}

fn stack_details(
    ctx: &Context,
    stack_id: Option<StackId>,
) -> anyhow::Result<but_workspace::ui::StackDetails> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = ctx.legacy_meta()?;
    let mut cache = ctx.cache.get_cache_mut()?;
    but_workspace::legacy::stack_details_v3(stack_id, &repo, &meta, &mut cache)
}

/// Returns the status of a stack.
fn get_stack_status(
    gix_repo: &gix::Repository,
    new_target_commit_id: gix::ObjectId,
    stack_id: Option<StackId>,
    review_map: &HashMap<String, but_forge::ForgeReview>,
    ctx: &Context,
) -> Result<StackStatus> {
    let mut last_head = new_target_commit_id;

    let mut branch_statuses: Vec<NameAndStatus> = vec![];

    let details = stack_details(ctx, stack_id)?;

    let branches = details.branch_details;
    for branch in branches.into_iter().rev() {
        let local_commits = &branch.commits;

        let Some(branch_head) = local_commits.first() else {
            branch_statuses.push(NameAndStatus {
                name: branch.name.to_string(),
                status: BranchStatus::Empty,
            });

            continue;
        };

        let branch_head_string = branch_head.id.to_string();

        // Check if the branch has been integrated (either via review or commits)
        let is_integrated_via_review = review_map
            .get(&branch.name.to_string())
            .is_some_and(|review| review.is_merged_at_commit(&branch_head_string));
        let is_integrated_via_commits = matches!(
            branch_head.state,
            but_workspace::ui::CommitState::Integrated
        );

        if is_integrated_via_commits || is_integrated_via_review {
            branch_statuses.push(NameAndStatus {
                name: branch.name.to_string(),
                status: BranchStatus::Integrated,
            });

            continue;
        }
        // Rebase the commits and see if any conflict
        // Rebasing is preferable to merging, as not everything that is
        // mergeable is rebasable.
        // Doing both would be preferable, but we don't communicate that
        // to the frontend at the minute.
        let local_commit_ids = local_commits
            .iter()
            .filter(|c| !matches!(c.state, but_workspace::ui::CommitState::Integrated))
            .map(|commit| commit.id)
            .rev()
            .collect::<Vec<_>>();

        let rebase_base = last_head;

        let steps: Vec<RebaseStep> = local_commit_ids
            .iter()
            .map(|commit_id| RebaseStep::Pick {
                commit_id: *commit_id,
                new_message: None,
            })
            .collect();
        let mut rebase = but_rebase::Rebase::new(gix_repo, Some(rebase_base), None)?;
        rebase.rebase_noops(false);
        rebase.steps(steps)?;
        let output = rebase.rebase(&*ctx.cache.get_cache()?)?;
        let new_head_oid = output.top_commit;

        let any_conflicted = output.commit_mapping.iter().any(|(_base, _old, new)| {
            if let Ok(commit) = gix_repo.find_commit(*new) {
                commit.is_conflicted()
            } else {
                false
            }
        });

        last_head = new_head_oid;

        branch_statuses.push(NameAndStatus {
            name: branch.name.to_string(),
            status: if any_conflicted {
                BranchStatus::Conflicted { rebasable: false }
            } else {
                BranchStatus::SafelyUpdatable
            },
        });
    }

    StackStatus::create(UpstreamTreeStatus::Empty, branch_statuses)
}

pub fn upstream_integration_statuses(
    context: &UpstreamIntegrationContext,
) -> Result<StackStatuses> {
    let UpstreamIntegrationContext {
        new_target,
        target,
        stacks_in_workspace,
        review_map,
        ctx,
        ..
    } = context;

    let repo = ctx.clone_repo_for_merging()?;
    let repo_in_memory = repo.clone().with_object_memory();

    if *new_target == target.sha {
        return Ok(StackStatuses::UpToDate);
    };

    let heads = stacks_in_workspace
        .iter()
        .map(|stack| stack.tip)
        .chain(std::iter::once(*new_target))
        .collect::<Vec<_>>();

    // The merge base tree of all of the applied stacks plus the new target
    let merge_base_tree = repo
        .merge_base_octopus(heads)?
        .object()?
        .into_commit()
        .tree_id()?;

    // The working directory tree
    #[expect(deprecated)]
    let workdir_tree = repo.create_wd_tree(gitbutler_project::AUTO_TRACK_LIMIT_BYTES)?;

    // The target tree
    let target_tree = repo.find_commit(*new_target)?.tree_id()?;

    let (merge_options_fail_fast, _conflict_kind) = repo.merge_options_no_rewrites_fail_fast()?;

    let merge_outcome = repo.merge_trees(
        merge_base_tree,
        repo.head()?.peel_to_commit()?.tree_id()?,
        target_tree,
        repo.default_merge_labels(),
        merge_options_fail_fast.clone(),
    )?;
    let committed_conflicts = merge_outcome
        .conflicts
        .iter()
        .filter(|c| c.is_unresolved(TreatAsUnresolved::git()))
        .collect::<Vec<_>>();

    let worktree_conflicts = repo
        .merge_trees(
            merge_base_tree,
            workdir_tree,
            target_tree,
            repo.default_merge_labels(),
            merge_options_fail_fast.clone(),
        )?
        .conflicts
        .iter()
        .filter(|c| c.is_unresolved(TreatAsUnresolved::git()))
        // only include conflicts that are not in the list committed_conflicts
        .filter(|c| !committed_conflicts.iter().any(|cc| cc.ours == c.ours))
        .map(|c| c.ours.location().into())
        .collect::<Vec<BStringForFrontend>>();

    let statuses = stacks_in_workspace
        .iter()
        .map(|stack| {
            Ok((
                stack.id,
                get_stack_status(
                    &repo_in_memory,
                    *new_target,
                    stack.id,
                    review_map,
                    context.ctx,
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(StackStatuses::UpdatesRequired {
        worktree_conflicts,
        statuses,
    })
}

pub(crate) fn integrate_upstream(
    ctx: &Context,
    resolutions: &[Resolution],
    base_branch_resolution: Option<BaseBranchResolution>,
    review_map: &HashMap<String, but_forge::ForgeReview>,
    permission: &mut RepoExclusive,
) -> Result<IntegrationOutcome> {
    let old_workspace = WorkspaceState::create(ctx, permission.read_permission())?;

    let (target_commit_oid, base_branch_resolution_approach) = base_branch_resolution
        .map(|r| (Some(r.target_commit_oid), Some(r.approach)))
        .unwrap_or((None, None));

    let repo = ctx.repo.get()?;
    let context =
        UpstreamIntegrationContext::open(ctx, target_commit_oid, permission, &repo, review_map)?;
    let mut virtual_branches_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let default_target = virtual_branches_state.get_default_target()?;

    let mut deleted_branches = vec![];

    // Ensure resolutions match current statuses
    {
        let statuses = upstream_integration_statuses(&context)?;

        let StackStatuses::UpdatesRequired { statuses, .. } = statuses else {
            bail!("Branches are all up to date")
        };

        if resolutions.len() != context.stacks_in_workspace.len() {
            bail!(
                "Chosen resolutions do not match quantity of applied virtual branches. {:?} {:?}",
                resolutions,
                context.stacks_in_workspace
            )
        }

        let all_resolutions_are_up_to_date = resolutions.iter().all(|resolution| {
            let Some(status) = statuses
                .iter()
                .find(|status| status.0 == Some(resolution.stack_id))
            else {
                return false;
            };

            status.1.resolution_acceptable(&resolution.approach)
        });

        if !all_resolutions_are_up_to_date {
            bail!("Chosen resolutions do not match current integration statuses")
        }
    }

    let integration_results =
        compute_resolutions(&context, resolutions, base_branch_resolution_approach)?;

    {
        // We perform the updates in stages. If deleting or unapplying fails, we
        // could enter a much worse state if we're simultaneously updating trees

        // Delete branches
        for (maybe_stack_id, integration_result) in &integration_results {
            if !matches!(integration_result, IntegrationResult::DeleteBranch) {
                continue;
            };

            let Some(stack_id) = maybe_stack_id else {
                // If the stack ID is not defined, we're on single-branch mode, so nothing to delete.
                continue;
            };

            let maybe_stack = context
                .stacks_in_workspace
                .iter()
                .find(|s| s.id == Some(*stack_id));

            let Some(stack) = maybe_stack else {
                // The integration results should match the stacks in the workspace,
                // so this should never happen.
                bail!("Failed to find stack while integrating upstream: {stack_id:?}");
            };

            virtual_branches_state.delete_branch_entry(stack_id)?;
            let delete_local_refs = resolutions
                .iter()
                .find(|r| r.stack_id == *stack_id)
                .map(|r| r.delete_integrated_branches)
                .unwrap_or(false);

            if delete_local_refs {
                for head in &stack.heads {
                    let branch_name = head.name.to_str().context("Invalid branch name")?;
                    match head.delete_reference(&repo) {
                        Ok(_) => {
                            deleted_branches.push(branch_name.to_string());
                        }
                        _ => {
                            // Fail silently because interrupting this is worse
                        }
                    }
                }
            }
        }

        let permission = context._permission.expect("Permission provided above");

        // Unapply branches
        for (maybe_stack_id, integration_result) in &integration_results {
            if !matches!(integration_result, IntegrationResult::UnapplyBranch) {
                continue;
            };

            let Some(stack_id) = maybe_stack_id else {
                // If the stack ID is not defined, we're on single-branch mode, so nothing to unapply.
                continue;
            };

            ctx.branch_manager().unapply(
                *stack_id,
                permission,
                false,
                Vec::new(),
                ctx.settings.feature_flags.cv3,
            )?;
        }

        let mut stacks = virtual_branches_state.list_stacks_in_workspace()?;

        virtual_branches_state.set_default_target(Target {
            sha: context.new_target,
            ..default_target
        })?;

        // Update branch trees
        for (maybe_stack_id, integration_result) in &integration_results {
            let IntegrationResult::UpdatedObjects {
                head,
                rebase_output,
                for_archival,
            } = integration_result
            else {
                continue;
            };

            let Some(stack_id) = maybe_stack_id else {
                // If the stack ID is not defined, we're on single-branch mode and there's nothing to update.
                continue;
            };

            let Some(stack) = stacks.iter_mut().find(|stack| stack.id == *stack_id) else {
                continue;
            };

            // Update the branch heads
            if let Some(output) = rebase_output {
                stack.set_heads_from_rebase_output(ctx, output.references.clone())?;
            }

            // Dissociate closed reviews
            for head in stack.clone().heads.iter() {
                let branch_name = head.name.to_string();
                if let Some(review) = review_map.get(&branch_name)
                    && !review.is_open()
                {
                    stack.set_pr_number(ctx, &branch_name, None)?;
                }
            }

            stack.set_stack_head(&mut virtual_branches_state, &repo, *head)?;

            let delete_local_refs = resolutions
                .iter()
                .find(|r| r.stack_id == *stack_id)
                .map(|r| r.delete_integrated_branches)
                .unwrap_or(false);

            let stack_branches_deleted =
                stack.archive_integrated_heads(ctx, &repo, for_archival, delete_local_refs)?;
            deleted_branches.extend(stack_branches_deleted);
        }

        {
            let new_workspace = WorkspaceState::create(ctx, permission.read_permission())?;
            update_uncommitted_changes(ctx, old_workspace, new_workspace, permission)?;
        }

        crate::integration::update_workspace_commit_with_vb_state(
            &virtual_branches_state,
            ctx,
            false,
        )?;
    }

    deleted_branches.sort();
    deleted_branches.dedup();

    Ok(IntegrationOutcome { deleted_branches })
}

pub(crate) fn resolve_upstream_integration(
    ctx: &Context,
    resolution_approach: BaseBranchResolutionApproach,
    review_map: &HashMap<String, but_forge::ForgeReview>,
    permission: &mut RepoExclusive,
) -> Result<gix::ObjectId> {
    let repo = ctx.repo.get()?;
    let context = UpstreamIntegrationContext::open(ctx, None, permission, &repo, review_map)?;
    let new_target_id = context.new_target;
    let old_target_id = context.target.sha;
    let fork_point = repo.merge_base(old_target_id, new_target_id)?.detach();

    match resolution_approach {
        BaseBranchResolutionApproach::HardReset => Ok(new_target_id),
        BaseBranchResolutionApproach::Merge => {
            let branch_name = context.target.branch.to_string();
            let new_head = merge_commits(
                &repo,
                old_target_id,
                context.new_target,
                &format!("Merge `{branch_name}` into `{branch_name}`"),
            )?;

            Ok(new_head)
        }
        BaseBranchResolutionApproach::Rebase => {
            let steps = first_parent_commit_ids_until(&repo, old_target_id, fork_point)?
                .into_iter()
                .map(|commit_id| RebaseStep::Pick {
                    commit_id,
                    new_message: None,
                })
                .collect::<Vec<_>>();
            let mut rebase = but_rebase::Rebase::new(&repo, Some(new_target_id), None)?;
            rebase.steps(steps)?;
            rebase.rebase_noops(false);
            let outcome = rebase.rebase(&*ctx.cache.get_cache()?)?;
            Ok(outcome.top_commit)
        }
    }
}

fn compute_resolutions(
    context: &UpstreamIntegrationContext,
    resolutions: &[Resolution],
    base_branch_resolution_approach: Option<BaseBranchResolutionApproach>,
) -> Result<Vec<(Option<StackId>, IntegrationResult)>> {
    let UpstreamIntegrationContext {
        new_target,
        target,
        stacks_in_workspace,
        gix_repo,
        ..
    } = context;

    let results = resolutions
        .iter()
        .map(|resolution| {
            let Some(stack) = stacks_in_workspace
                .iter()
                .find(|stack| stack.id == Some(resolution.stack_id))
            else {
                bail!("Failed to find virtual branch");
            };

            match resolution.approach {
                ResolutionApproach::Unapply => Ok((stack.id, IntegrationResult::UnapplyBranch)),
                ResolutionApproach::Delete => Ok((stack.id, IntegrationResult::DeleteBranch)),
                ResolutionApproach::Merge => {
                    // Make a merge commit. It will be set as a stack head later.
                    let top_branch = stack.heads.last().context("top branch not found")?;

                    // These two go into the merge commit message.
                    let incoming_branch_name = target.branch.fullname();
                    let target_branch_name = top_branch.name.to_str()?;

                    let new_head = merge_commits(
                        gix_repo,
                        stack.tip,
                        *new_target,
                        &format!("Merge `{incoming_branch_name}` into `{target_branch_name}`"),
                    )?;

                    Ok((
                        stack.id,
                        IntegrationResult::UpdatedObjects {
                            head: new_head,
                            rebase_output: None,
                            for_archival: vec![],
                        },
                    ))
                }
                ResolutionApproach::Rebase => {
                    // Rebase the commits, then try rebasing the tree. If
                    // the tree ends up conflicted, commit the tree.

                    // If the base branch needs to resolve its divergence
                    // pick only the commits that are ahead of the old target head
                    let lower_bound = if base_branch_resolution_approach.is_some() {
                        target.sha
                    } else {
                        *new_target
                    };

                    let details = stack_details(context.ctx, stack.id)?;
                    let mut commit_map = HashMap::new();
                    for branch in &details.branch_details {
                        for commit in &branch.commits {
                            commit_map.insert(commit.id, commit.clone());
                        }
                    }

                    let all_steps = details.as_rebase_steps(context.gix_repo)?;
                    let branches_before = as_buckets(all_steps.clone());
                    // Filter out any integrated commits
                    let steps = all_steps
                        .into_iter()
                        .filter_map(|s| match s {
                            RebaseStep::Pick {
                                commit_id,
                                new_message: _,
                            } => {
                                let is_integrated = commit_map.get(&commit_id).is_some_and(|c| {
                                    matches!(c.state, but_workspace::ui::CommitState::Integrated)
                                });
                                if is_integrated { None } else { Some(s) }
                            }
                            _ => Some(s),
                        })
                        .collect::<Vec<_>>();

                    let branches_after = as_buckets(steps.clone());

                    // Branches that used to have commits but now don't are marked for archival
                    let mut for_archival = vec![];
                    for (ref_before, steps_before) in branches_before {
                        if let Some((_, steps_after)) = branches_after
                            .iter()
                            .find(|(ref_after, _)| ref_after == &ref_before)
                        {
                            // if there were steps before and now there are none, this should be marked for archival
                            if !steps_before.is_empty() && steps_after.is_empty() {
                                for_archival.push(ref_before);
                            }
                        }
                    }

                    let mut rebase =
                        but_rebase::Rebase::new(context.gix_repo, Some(lower_bound), None)?;
                    rebase.rebase_noops(false);
                    rebase.steps(steps)?;
                    let output = rebase.rebase(&*context.ctx.cache.get_cache()?)?;
                    let new_head = output.top_commit;

                    Ok((
                        stack.id,
                        IntegrationResult::UpdatedObjects {
                            head: new_head,
                            rebase_output: Some(output),
                            for_archival,
                        },
                    ))
                }
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(results)
}

pub(crate) fn as_buckets(steps: Vec<RebaseStep>) -> Vec<(but_core::Reference, Vec<RebaseStep>)> {
    let mut buckets = vec![];
    let mut current_steps = vec![];
    for step in steps {
        match step {
            RebaseStep::Reference(reference) => {
                buckets.push((reference, std::mem::take(&mut current_steps)));
            }
            step => {
                current_steps.push(step);
            }
        }
    }
    buckets
}
