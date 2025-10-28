use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use bstr::ByteSlice;
use but_core::Reference;
use but_graph::VirtualBranchesTomlMetadata;
use but_rebase::{RebaseOutput, RebaseStep};
use but_workspace::{ref_info::Options, stack_ext::StackDetailsExt};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_oxidize::{
    GixRepositoryExt, ObjectIdExt, OidExt, git2_to_gix_object_id, gix_to_git2_oid,
};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{
    RepositoryExt as _,
    logging::{LogUntil, RepositoryExt as _},
    rebase::gitbutler_merge_commits,
};
use gitbutler_serde::BStringForFrontend;
use gitbutler_stack::{StackId, Target, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommited_changes};
use gix::merge::tree::TreatAsUnresolved;
use serde::{Deserialize, Serialize};

use crate::{BranchManagerExt, VirtualBranchesExt as _};

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NameAndStatus {
    pub name: String,
    pub status: BranchStatus,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    pub tree_status: TreeStatus,
    pub branch_statuses: Vec<NameAndStatus>,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum TreeStatus {
    SaflyUpdatable,
    Conflicted,
    Empty,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BranchStatus {
    SaflyUpdatable,
    Integrated,
    Conflicted {
        /// If the branch can be rebased onto the target without conflicts
        rebasable: bool,
    },
    Empty,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum StackStatuses {
    UpToDate,
    UpdatesRequired {
        #[serde(rename = "worktreeConflicts")]
        worktree_conflicts: Vec<BStringForFrontend>,
        statuses: Vec<(Option<StackId>, StackStatus)>,
    },
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BaseBranchResolutionApproach {
    Rebase,
    Merge,
    HardReset,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum ResolutionApproach {
    Rebase,
    Merge,
    Unapply,
    Delete,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranchResolution {
    #[serde(with = "gitbutler_serde::oid")]
    target_commit_oid: git2::Oid,
    approach: BaseBranchResolutionApproach,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationOutcome {
    /// This is the list of branch names that have become archived as a result of the upstream integration
    archived_branches: Vec<String>,
    /// This is the list of review ids that have been closed as a result of the upstream integration
    review_ids_to_close: Vec<String>,
}

impl StackStatus {
    fn create(tree_status: TreeStatus, branch_statuses: Vec<NameAndStatus>) -> Result<Self> {
        if branch_statuses.is_empty() {
            bail!("Branch statuses must not be empty")
        }

        Ok(Self {
            tree_status,
            branch_statuses,
        })
    }

    fn resolution_acceptable(&self, approach: &ResolutionApproach) -> bool {
        if self.tree_status == TreeStatus::Empty
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
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub stack_id: StackId,
    pub approach: ResolutionApproach,
    pub delete_integrated_branches: bool,
    /// A list of references that the application should consider as integrated even if they are not deteced as such.
    /// This is useful in the case of squash-merging, where GitButler can not detect the integration of branches.
    /// This signal can be provided either by the user or, even better, based on a status from GitHub.
    pub force_integrated_branches: Vec<String>,
}

enum IntegrationResult {
    UpdatedObjects {
        head: git2::Oid,
        tree: Option<git2::Oid>,
        rebase_output: Option<RebaseOutput>,
        for_archival: Vec<Reference>,
    },
    UnapplyBranch,
    DeleteBranch,
}

pub struct UpstreamIntegrationContext<'a> {
    _permission: Option<&'a mut WorktreeWritePermission>,
    repo: &'a git2::Repository,
    stacks_in_workspace: Vec<but_workspace::ui::StackEntry>,
    new_target: git2::Commit<'a>,
    target: Target,
    ctx: &'a CommandContext,
    gix_repo: &'a gix::Repository,
}

impl<'a> UpstreamIntegrationContext<'a> {
    pub(crate) fn open(
        ctx: &'a CommandContext,
        target_commit_oid: Option<git2::Oid>,
        permission: &'a mut WorktreeWritePermission,
        gix_repo: &'a gix::Repository,
    ) -> Result<Self> {
        let meta = ctx.meta(permission.read_permission())?;
        let repo = ctx.gix_repo()?;
        let _ref_info = but_workspace::head_info(
            &repo,
            &*meta,
            Options {
                expensive_commit_info: true,
                traversal: meta.graph_options(),
            },
        )?;

        let virtual_branches_handle = ctx.project().virtual_branches();
        let target = virtual_branches_handle.get_default_target()?;
        let repo = ctx.repo();
        let target_branch = repo
            .maybe_find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("Branch not found"))?;

        let new_target = target_commit_oid.map_or_else(
            || target_branch.get().peel_to_commit(),
            |oid| repo.find_commit(oid),
        )?;

        let stacks_in_workspace = stacks(ctx, gix_repo)?;

        Ok(Self {
            _permission: Some(permission),
            repo,
            new_target,
            target: target.clone(),
            stacks_in_workspace,
            ctx,
            gix_repo,
        })
    }
}

fn stacks(
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<but_workspace::ui::StackEntry>> {
    let project = ctx.project();
    if ctx.app_settings().feature_flags.ws3 {
        let meta =
            VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))?;
        but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace, None)
    } else {
        but_workspace::stacks(
            ctx,
            &project.gb_dir(),
            repo,
            but_workspace::StacksFilter::InWorkspace,
        )
    }
}

fn stack_details(
    ctx: &CommandContext,
    stack_id: Option<StackId>,
) -> anyhow::Result<but_workspace::ui::StackDetails> {
    if ctx.app_settings().feature_flags.ws3 {
        let repo = ctx.gix_repo_for_merging_non_persisting()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stack_details_v3(stack_id, &repo, &meta)
    } else {
        let Some(stack_id) = stack_id else {
            bail!("Failed to get stack details: stack ID not provided");
        };
        but_workspace::stack_details(&ctx.project().gb_dir(), stack_id, ctx)
    }
}

/// Returns the status of a stack
/// Takes both a gix and git2 repository. The git2 repository can't be in
/// memory as the gix repository needs to be able to access those commits
fn get_stack_status(
    gix_repo: &gix::Repository,
    new_target_commit_id: gix::ObjectId,
    stack_id: Option<StackId>,
    ctx: &CommandContext,
) -> Result<StackStatus> {
    let mut last_head: git2::Oid = gix_to_git2_oid(new_target_commit_id);

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

        // Check if the branch in question has already been integrated
        if matches!(
            branch_head.state,
            but_workspace::ui::CommitState::Integrated
        ) {
            branch_statuses.push(NameAndStatus {
                name: branch.name.to_string(),
                status: BranchStatus::Integrated,
            });

            continue;
        }
        // Rebase the commits and see if any conflict
        // Rebasing is preferable to merging, as not everything that is
        // mergable is rebasable.
        // Doing both would be preferable, but we don't communicate that
        // to the frontend at the minute.
        let local_commit_ids = local_commits
            .iter()
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
        let mut rebase = but_rebase::Rebase::new(gix_repo, Some(rebase_base.to_gix()), None)?;
        rebase.rebase_noops(false);
        rebase.steps(steps)?;
        let output = rebase.rebase()?;
        let new_head_oid = output.top_commit.to_git2();

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
                BranchStatus::SaflyUpdatable
            },
        });
    }

    StackStatus::create(TreeStatus::Empty, branch_statuses)
}

pub fn upstream_integration_statuses(
    context: &UpstreamIntegrationContext,
) -> Result<StackStatuses> {
    let UpstreamIntegrationContext {
        repo,
        new_target,
        target,
        stacks_in_workspace,
        ..
    } = context;
    let old_target = repo.find_commit(target.sha)?;

    let gix_repo = gitbutler_command_context::gix_repo_for_merging(repo.path())?;
    let gix_repo_in_memory = gix_repo.clone().with_object_memory();

    if new_target.id() == old_target.id() {
        return Ok(StackStatuses::UpToDate);
    };

    let new_target_id = new_target.id().to_gix();

    let heads = stacks_in_workspace
        .iter()
        .map(|stack| stack.tip)
        .chain(std::iter::once(new_target_id))
        .collect::<Vec<_>>();

    // The merge base tree of all of the applied stacks plus the new target
    let merge_base_tree = gix_repo
        .merge_base_octopus(heads)?
        .object()?
        .into_commit()
        .tree_id()?;

    // The working directory tree
    let workdir_tree = context
        .ctx
        .repo()
        .create_wd_tree(gitbutler_project::AUTO_TRACK_LIMIT_BYTES)?
        .id()
        .to_gix();

    // The target tree
    let target_tree = gix_repo.find_commit(new_target.id().to_gix())?.tree_id()?;

    let (merge_options_fail_fast, _conflict_kind) =
        gix_repo.merge_options_no_rewrites_fail_fast()?;

    let merge_outcome = gix_repo.merge_trees(
        merge_base_tree,
        gix_repo.head()?.peel_to_commit()?.tree_id()?,
        target_tree,
        gix_repo.default_merge_labels(),
        merge_options_fail_fast.clone(),
    )?;
    let committed_conflicts = merge_outcome
        .conflicts
        .iter()
        .filter(|c| c.is_unresolved(TreatAsUnresolved::git()))
        .collect::<Vec<_>>();

    let worktree_conflicts = gix_repo
        .merge_trees(
            merge_base_tree,
            workdir_tree,
            target_tree,
            gix_repo.default_merge_labels(),
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
                    &gix_repo_in_memory,
                    git2_to_gix_object_id(new_target.id()),
                    stack.id,
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
    ctx: &CommandContext,
    resolutions: &[Resolution],
    base_branch_resolution: Option<BaseBranchResolution>,
    permission: &mut WorktreeWritePermission,
) -> Result<IntegrationOutcome> {
    let old_workspace = WorkspaceState::create(ctx, permission.read_permission())?;

    let (target_commit_oid, base_branch_resolution_approach) = base_branch_resolution
        .map(|r| (Some(r.target_commit_oid), Some(r.approach)))
        .unwrap_or((None, None));

    let gix_repo = ctx.gix_repo()?;
    let context = UpstreamIntegrationContext::open(ctx, target_commit_oid, permission, &gix_repo)?;
    let virtual_branches_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = virtual_branches_state.get_default_target()?;

    let mut newly_archived_branches = vec![];
    let mut to_be_closed_review_ids = vec![];

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
        // We preform the updates in stages. If deleting or unapplying fails, we
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
                bail!(
                    "Failed to find stack while integrating upstream: {:?}",
                    stack_id
                );
            };

            virtual_branches_state.delete_branch_entry(stack_id)?;
            let delete_local_refs = resolutions
                .iter()
                .find(|r| r.stack_id == *stack_id)
                .map(|r| r.delete_integrated_branches)
                .unwrap_or(false);

            if delete_local_refs {
                for head in &stack.heads {
                    head.delete_reference(&gix_repo).ok();
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
                ctx.app_settings().feature_flags.cv3,
            )?;
        }

        let mut stacks = virtual_branches_state.list_stacks_in_workspace()?;

        virtual_branches_state.set_default_target(Target {
            sha: context.new_target.id(),
            ..default_target
        })?;

        // Update branch trees
        for (maybe_stack_id, integration_result) in &integration_results {
            let IntegrationResult::UpdatedObjects {
                head,
                tree,
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
            stack.set_stack_head(&virtual_branches_state, &gix_repo, *head, *tree)?;

            let delete_local_refs = resolutions
                .iter()
                .find(|r| r.stack_id == *stack_id)
                .map(|r| r.delete_integrated_branches)
                .unwrap_or(false);

            let (mut archived_branches, mut review_ids_to_close) =
                stack.archive_integrated_heads(ctx, &gix_repo, for_archival, delete_local_refs)?;
            newly_archived_branches.append(&mut archived_branches);
            to_be_closed_review_ids.append(&mut review_ids_to_close);
        }

        {
            let new_workspace = WorkspaceState::create(ctx, permission.read_permission())?;
            update_uncommited_changes(ctx, old_workspace, new_workspace, permission)?;
        }

        crate::integration::update_workspace_commit(&virtual_branches_state, ctx, false)?;
    }

    Ok(IntegrationOutcome {
        archived_branches: newly_archived_branches,
        review_ids_to_close: to_be_closed_review_ids,
    })
}

pub(crate) fn resolve_upstream_integration(
    ctx: &CommandContext,
    resolution_approach: BaseBranchResolutionApproach,
    permission: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    let gix_repo = ctx.gix_repo()?;
    let context = UpstreamIntegrationContext::open(ctx, None, permission, &gix_repo)?;
    let repo = ctx.repo();
    let new_target_id = context.new_target.id();
    let old_target_id = context.target.sha;
    let fork_point = repo.merge_base(old_target_id, new_target_id)?;

    match resolution_approach {
        BaseBranchResolutionApproach::HardReset => Ok(new_target_id),
        BaseBranchResolutionApproach::Merge => {
            let branch_name = context.target.branch.to_string();
            let old_target = repo.find_commit(context.target.sha)?;
            let new_head = gitbutler_merge_commits(
                repo,
                old_target,
                context.new_target,
                &branch_name,
                &branch_name,
            )?;

            Ok(new_head.id())
        }
        BaseBranchResolutionApproach::Rebase => {
            let commits = repo.l(old_target_id, LogUntil::Commit(fork_point), false)?;
            let steps = commits
                .iter()
                .map(|commit| RebaseStep::Pick {
                    commit_id: commit.to_gix(),
                    new_message: None,
                })
                .collect::<Vec<_>>();
            let mut rebase =
                but_rebase::Rebase::new(&gix_repo, Some(new_target_id.to_gix()), None)?;
            rebase.steps(steps)?;
            rebase.rebase_noops(false);
            let outcome = rebase.rebase()?;
            let new_head = outcome.top_commit.to_git2();

            Ok(new_head)
        }
    }
}

fn compute_resolutions(
    context: &UpstreamIntegrationContext,
    resolutions: &[Resolution],
    base_branch_resolution_approach: Option<BaseBranchResolutionApproach>,
) -> Result<Vec<(Option<StackId>, IntegrationResult)>> {
    let UpstreamIntegrationContext {
        repo,
        new_target,
        target,
        stacks_in_workspace,
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
                    // Make a merge commit on top of the branch commits,
                    // then rebase the tree ontop of that. If the tree ends
                    // up conflicted, commit the tree.
                    let target_commit = repo.find_commit(stack.tip.to_git2())?;
                    let top_branch = stack.heads.last().context("top branch not found")?;

                    // These two go into the merge commit message.
                    let incoming_branch_name = target.branch.fullname();
                    let target_branch_name = top_branch.name.to_str()?;

                    let new_head = gitbutler_merge_commits(
                        repo,
                        target_commit,
                        new_target.clone(),
                        target_branch_name,
                        &incoming_branch_name,
                    )?;

                    Ok((
                        stack.id,
                        IntegrationResult::UpdatedObjects {
                            head: new_head.id(),
                            tree: None,
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
                        new_target.id()
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
                                let commit = repo.find_commit(commit_id.to_git2()).ok()?;
                                let is_integrated = commit_map.get(&commit_id).is_some_and(|c| {
                                    matches!(c.state, but_workspace::ui::CommitState::Integrated)
                                });
                                let forced = forced_integrated(
                                    &resolution.force_integrated_branches,
                                    &branches_before,
                                    &commit.id().to_gix(),
                                );
                                if is_integrated || forced {
                                    None
                                } else {
                                    Some(s)
                                }
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

                    let mut rebase = but_rebase::Rebase::new(
                        context.gix_repo,
                        Some(lower_bound.to_gix()),
                        None,
                    )?;
                    rebase.rebase_noops(false);
                    rebase.steps(steps)?;
                    let output = rebase.rebase()?;
                    let new_head = output.top_commit.to_git2();

                    Ok((
                        stack.id,
                        IntegrationResult::UpdatedObjects {
                            head: new_head,
                            tree: None,
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

// If the commit is in a bucket (branches_before) where the reference matches any of the
// resolution.force_integrated_branches then we consider it integrated.
fn forced_integrated(
    force_integrated_branches: &[String],
    branches: &[(Reference, Vec<RebaseStep>)],
    target_commit_id: &gix::ObjectId,
) -> bool {
    force_integrated_branches.iter().any(|ref_name| {
        // The reference this commit is under (from branches_before)
        let commit_ref = branches.iter().find_map(|(ref_name, steps)| {
            steps.iter().find_map(|step| {
                if let RebaseStep::Pick {
                    commit_id,
                    new_message: _,
                } = step
                {
                    if commit_id == target_commit_id {
                        Some(ref_name.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        });

        if let Some(commit_ref) = &commit_ref {
            &commit_ref.to_string() == ref_name
        } else {
            false
        }
    })
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
