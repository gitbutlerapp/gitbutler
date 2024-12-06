use crate::stack::branch_integrated;
use crate::{r#virtual::IsCommitIntegrated, BranchManagerExt, VirtualBranchesExt as _};
use anyhow::{anyhow, bail, Result};
use gitbutler_cherry_pick::RepositoryExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_oxidize::{git2_to_gix_object_id, gix_to_git2_oid};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo::{
    rebase::{cherry_rebase_group, gitbutler_merge_commits},
    GixRepositoryExt, LogUntil,
};
use gitbutler_repo_actions::RepoActionsExt as _;
use gitbutler_stack::stack_context::StackContext;
use gitbutler_stack::{Stack, StackId, Target, VirtualBranchesHandle};
use gitbutler_workspace::{checkout_branch_trees, compute_updated_branch_head, BranchHeadAndTree};
use serde::{Deserialize, Serialize};

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NameAndStatus {
    name: String,
    status: BranchStatus,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    tree_status: TreeStatus,
    branch_statuses: Vec<NameAndStatus>,
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
    UpdatesRequired(Vec<(StackId, StackStatus)>),
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
    // TODO(CTO): Rename to stack_id
    pub branch_id: StackId,
    /// Used to ensure a given branch hasn't changed since the UI issued the command.
    #[serde(with = "gitbutler_serde::oid")]
    pub branch_tree: git2::Oid,
    pub approach: ResolutionApproach,
}

enum IntegrationResult {
    UpdatedObjects { head: git2::Oid, tree: git2::Oid },
    UnapplyBranch,
    DeleteBranch,
}

pub struct UpstreamIntegrationContext<'a> {
    _permission: Option<&'a mut WorktreeWritePermission>,
    repository: &'a git2::Repository,
    stacks_in_workspace: Vec<Stack>,
    new_target: git2::Commit<'a>,
    target: Target,
}

impl<'a> UpstreamIntegrationContext<'a> {
    pub(crate) fn open(
        command_context: &'a CommandContext,
        target_commit_oid: Option<git2::Oid>,
        permission: &'a mut WorktreeWritePermission,
    ) -> Result<Self> {
        let virtual_branches_handle = command_context.project().virtual_branches();
        let target = virtual_branches_handle.get_default_target()?;
        let repository = command_context.repo();
        let target_branch = repository
            .maybe_find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("Branch not found"))?;

        let new_target = target_commit_oid.map_or_else(
            || target_branch.get().peel_to_commit(),
            |oid| repository.find_commit(oid),
        )?;

        let stacks_in_workspace = virtual_branches_handle.list_stacks_in_workspace()?;

        Ok(Self {
            _permission: Some(permission),
            repository,
            new_target,
            target: target.clone(),
            stacks_in_workspace,
        })
    }
}

/// Returns the status of a stack
/// Takes both a gix and git2 repository. The git2 repository can't be in
/// memory as the gix repository needs to be able to access those commits
fn get_stack_status(
    repository: &git2::Repository,
    gix_repository: &gix::Repository,
    target: Target,
    new_target_commit_id: gix::ObjectId,
    stack: &Stack,
) -> Result<StackStatus> {
    let cache = gix_repository.commit_graph_if_enabled()?;
    let mut graph = gix_repository.revision_graph(cache.as_ref());
    let upstream_commit_oids = repository.l(
        gix_to_git2_oid(new_target_commit_id),
        LogUntil::Commit(target.sha),
        true,
    )?;
    let new_target_tree_id = gix_repository
        .find_commit(new_target_commit_id)?
        .tree_id()?;
    let mut check_commit = IsCommitIntegrated::new_basic(
        gix_repository,
        repository,
        &mut graph,
        git2_to_gix_object_id(target.sha),
        new_target_tree_id.detach(),
        upstream_commit_oids,
    );

    let mut unintegrated_branch_found = false;

    let mut last_head: git2::Oid = gix_to_git2_oid(new_target_commit_id);

    let mut branch_statuses: Vec<NameAndStatus> = vec![];

    let stack_context = StackContext::new(repository, target);
    let branches = stack.branches();
    for branch in &branches {
        if branch.archived {
            continue;
        }

        // If an integrated branch has been found, there is no need to bother
        // with subsequent branches.
        if !unintegrated_branch_found
            && branch_integrated(&mut check_commit, branch, &stack_context, stack)?
        {
            branch_statuses.push(NameAndStatus {
                name: branch.name.clone(),
                status: BranchStatus::Integrated,
            });

            continue;
        } else {
            unintegrated_branch_found = true;
        }

        // Rebase the commits and see if any conflict
        // Rebasing is preferable to merging, as not everything that is
        // mergable is rebasable.
        // Doing both would be preferable, but we don't communicate that
        // to the frontend at the minute.
        let commits = branch.commits(&stack_context, stack)?;

        if commits.local_commits.is_empty() {
            branch_statuses.push(NameAndStatus {
                name: branch.name.clone(),
                status: BranchStatus::Empty,
            });

            continue;
        }

        let local_commit_ids = commits
            .local_commits
            .iter()
            .map(|commit| commit.id())
            .rev()
            .collect::<Vec<_>>();

        let rebase_base = last_head;

        let new_head_oid = cherry_rebase_group(repository, rebase_base, &local_commit_ids, false)?;
        let rebased_commits = repository.log(new_head_oid, LogUntil::Commit(rebase_base), false)?;

        last_head = new_head_oid;

        let any_conflicted = rebased_commits.iter().any(|commit| commit.is_conflicted());

        branch_statuses.push(NameAndStatus {
            name: branch.name.clone(),
            status: if any_conflicted {
                BranchStatus::Conflicted { rebasable: false }
            } else {
                BranchStatus::SaflyUpdatable
            },
        });
    }

    let stack_head = repository.find_commit(stack.head())?;

    let tree_status = if stack.tree
        == repository
            .find_real_tree(&stack_head, Default::default())?
            .id()
    {
        TreeStatus::Empty
    } else {
        let (merge_options_fail_fast, conflict_kind) =
            gix_repository.merge_options_no_rewrites_fail_fast()?;

        let tree_merge_base = gix_repository
            .find_commit(new_target_commit_id)?
            .tree_id()?;
        let tree_id = git2_to_gix_object_id(stack.tree);
        let new_head_commit = repository.find_commit(last_head)?;
        let tree_conflicted = gix_repository
            .merge_trees(
                tree_merge_base,
                tree_id,
                git2_to_gix_object_id(new_head_commit.tree_id()),
                gix_repository.default_merge_labels(),
                merge_options_fail_fast.clone(),
            )?
            .has_unresolved_conflicts(conflict_kind);

        if tree_conflicted {
            TreeStatus::Conflicted
        } else {
            TreeStatus::SaflyUpdatable
        }
    };

    StackStatus::create(tree_status, branch_statuses)
}

pub fn upstream_integration_statuses(
    context: &UpstreamIntegrationContext,
) -> Result<StackStatuses> {
    let UpstreamIntegrationContext {
        repository,
        new_target,
        target,
        stacks_in_workspace,
        ..
    } = context;
    let old_target = repository.find_commit(target.sha)?;

    let gix_repository = gitbutler_command_context::gix_repository_for_merging(repository.path())?;
    let gix_repository_in_memory = gix_repository.clone().with_object_memory();

    if new_target.id() == old_target.id() {
        return Ok(StackStatuses::UpToDate);
    };

    let statuses = stacks_in_workspace
        .iter()
        .map(|stack| {
            Ok((
                stack.id,
                get_stack_status(
                    repository,
                    &gix_repository_in_memory,
                    target.clone(),
                    git2_to_gix_object_id(new_target.id()),
                    stack,
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(StackStatuses::UpdatesRequired(statuses))
}

pub(crate) fn integrate_upstream(
    command_context: &CommandContext,
    resolutions: &[Resolution],
    base_branch_resolution: Option<BaseBranchResolution>,
    permission: &mut WorktreeWritePermission,
) -> Result<()> {
    let (target_commit_oid, base_branch_resolution_approach) = base_branch_resolution
        .map(|r| (Some(r.target_commit_oid), Some(r.approach)))
        .unwrap_or((None, None));

    let context = UpstreamIntegrationContext::open(command_context, target_commit_oid, permission)?;
    let virtual_branches_state = VirtualBranchesHandle::new(command_context.project().gb_dir());
    let default_target = virtual_branches_state.get_default_target()?;

    // Ensure resolutions match current statuses
    {
        let statuses = upstream_integration_statuses(&context)?;

        let StackStatuses::UpdatesRequired(statuses) = statuses else {
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
            // This is O(n^2), in reality, n is unlikly to be more than 3 or 4
            let Some(branch) = context
                .stacks_in_workspace
                .iter()
                .find(|branch| branch.id == resolution.branch_id)
            else {
                return false;
            };

            if resolution.branch_tree != branch.tree {
                return false;
            };

            let Some(status) = statuses
                .iter()
                .find(|status| status.0 == resolution.branch_id)
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
        // could enter a much worse state if we're simultaniously updating trees

        // Delete branches
        for (stack_id, integration_result) in &integration_results {
            if !matches!(integration_result, IntegrationResult::DeleteBranch) {
                continue;
            };

            let stack = virtual_branches_state.get_stack(*stack_id)?;
            virtual_branches_state.delete_branch_entry(stack_id)?;
            command_context.delete_branch_reference(&stack)?;
        }

        let permission = context._permission.expect("Permission provided above");

        // Unapply branches
        for (stack_id, integration_result) in &integration_results {
            if !matches!(integration_result, IntegrationResult::UnapplyBranch) {
                continue;
            };

            command_context
                .branch_manager()
                .save_and_unapply(*stack_id, permission)?;
        }

        let mut stacks = virtual_branches_state.list_stacks_in_workspace()?;

        virtual_branches_state.set_default_target(Target {
            sha: context.new_target.id(),
            ..default_target
        })?;

        // Update branch trees
        for (branch_id, integration_result) in &integration_results {
            let IntegrationResult::UpdatedObjects { head, tree } = integration_result else {
                continue;
            };

            let Some(stack) = stacks.iter_mut().find(|branch| branch.id == *branch_id) else {
                continue;
            };

            stack.set_stack_head(command_context, *head, Some(*tree))?;
            stack.archive_integrated_heads(command_context)?;
        }

        // checkout_branch_trees won't checkout anything if there are no
        // applied branches, and returns the current_wd_tree as its result.
        // This is very sensible, but in this case, we want to checkout the
        // new target sha.
        if stacks.is_empty() {
            context
                .repository
                .checkout_tree_builder(&context.new_target.tree()?)
                .force()
                .remove_untracked()
                .checkout()?;
        } else {
            // Now that we've potentially updated the branch trees, lets checkout
            // the result of merging them all together.
            checkout_branch_trees(command_context, permission)?;
        }

        crate::integration::update_workspace_commit(&virtual_branches_state, command_context)?;
    }

    Ok(())
}

pub(crate) fn resolve_upstream_integration(
    command_context: &CommandContext,
    resolution_approach: BaseBranchResolutionApproach,
    permission: &mut WorktreeWritePermission,
) -> Result<git2::Oid> {
    let context = UpstreamIntegrationContext::open(command_context, None, permission)?;
    let repo = command_context.repo();
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
            let new_head = cherry_rebase_group(repo, new_target_id, &commits, false)?;

            Ok(new_head)
        }
    }
}

fn compute_resolutions(
    context: &UpstreamIntegrationContext,
    resolutions: &[Resolution],
    base_branch_resolution_approach: Option<BaseBranchResolutionApproach>,
) -> Result<Vec<(StackId, IntegrationResult)>> {
    let UpstreamIntegrationContext {
        repository,
        new_target,
        target,
        stacks_in_workspace,
        ..
    } = context;

    let results = resolutions
        .iter()
        .map(|resolution| {
            let Some(virtual_branch) = stacks_in_workspace
                .iter()
                .find(|branch| branch.id == resolution.branch_id)
            else {
                bail!("Failed to find virtual branch");
            };

            match resolution.approach {
                ResolutionApproach::Unapply => {
                    Ok((virtual_branch.id, IntegrationResult::UnapplyBranch))
                }
                ResolutionApproach::Delete => {
                    Ok((virtual_branch.id, IntegrationResult::DeleteBranch))
                }
                ResolutionApproach::Merge => {
                    // Make a merge commit on top of the branch commits,
                    // then rebase the tree ontop of that. If the tree ends
                    // up conflicted, commit the tree.
                    let target_commit = repository.find_commit(virtual_branch.head())?;

                    let new_head = gitbutler_merge_commits(
                        repository,
                        target_commit,
                        new_target.clone(),
                        &virtual_branch.name,
                        &target.branch.to_string(),
                    )?;

                    // Get the updated tree oid
                    let BranchHeadAndTree {
                        head: new_head,
                        tree: new_tree,
                    } = compute_updated_branch_head(repository, virtual_branch, new_head.id())?;

                    Ok((
                        virtual_branch.id,
                        IntegrationResult::UpdatedObjects {
                            head: new_head,
                            tree: new_tree,
                        },
                    ))
                }
                ResolutionApproach::Rebase => {
                    let gix_repository =
                        gitbutler_command_context::gix_repository_for_merging(repository.path())?;
                    let cache = gix_repository.commit_graph_if_enabled()?;
                    let mut graph = gix_repository.revision_graph(cache.as_ref());
                    let upstream_commit_oids =
                        repository.l(new_target.id(), LogUntil::Commit(target.sha), true)?;
                    let mut check_commit = IsCommitIntegrated::new_basic(
                        &gix_repository,
                        repository,
                        &mut graph,
                        git2_to_gix_object_id(target.sha),
                        git2_to_gix_object_id(new_target.tree_id()),
                        upstream_commit_oids,
                    );

                    // Rebase the commits, then try rebasing the tree. If
                    // the tree ends up conflicted, commit the tree.

                    // If the base branch needs to resolve its divergence
                    // pick only the commits that are ahead of the old target head
                    let lower_bound = if base_branch_resolution_approach.is_some() {
                        target.sha
                    } else {
                        new_target.id()
                    };

                    // Rebase virtual branches' commits
                    let virtual_branch_commits = repository.log(
                        virtual_branch.head(),
                        LogUntil::Commit(lower_bound),
                        false,
                    )?;

                    // Filter out any integrated commits
                    let virtual_branch_commits = virtual_branch_commits
                        .into_iter()
                        .filter_map(|commit| {
                            let is_integrated = check_commit.is_integrated(&commit).ok()?;
                            if is_integrated {
                                None
                            } else {
                                Some(commit.id())
                            }
                        })
                        .collect::<Vec<_>>();

                    let new_head = cherry_rebase_group(
                        repository,
                        new_target.id(),
                        &virtual_branch_commits,
                        false,
                    )?;

                    // Get the updated tree oid
                    let BranchHeadAndTree {
                        head: new_head,
                        tree: new_tree,
                    } = compute_updated_branch_head(repository, virtual_branch, new_head)?;

                    Ok((
                        virtual_branch.id,
                        IntegrationResult::UpdatedObjects {
                            head: new_head,
                            tree: new_tree,
                        },
                    ))
                }
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(results)
}
