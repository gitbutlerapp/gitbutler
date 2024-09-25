use anyhow::{anyhow, bail, Context, Result};
use gitbutler_branch::{
    signature, Branch, BranchId, SignaturePurpose, Target, VirtualBranchesHandle,
};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{
    rebase::{cherry_rebase_group, gitbutler_merge_commits},
    LogUntil, RepoActionsExt as _, RepositoryExt as _,
};
use serde::{Deserialize, Serialize};

use crate::{branch_trees::checkout_branch_trees, BranchManagerExt, VirtualBranchesExt as _};

#[derive(Serialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BranchStatus {
    Empty,
    FullyIntegrated,
    Conflicted {
        potentially_conflicted_uncommited_changes: bool,
    },
    SaflyUpdatable,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BranchStatuses {
    UpToDate,
    UpdatesRequired(Vec<(BranchId, BranchStatus)>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BaseBranchResolutionApproach {
    Rebase,
    Merge,
    HardReset,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
enum ResolutionApproach {
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

impl BranchStatus {
    fn resolution_acceptable(&self, approach: &ResolutionApproach) -> bool {
        match self {
            Self::Empty | Self::SaflyUpdatable | Self::Conflicted { .. } => matches!(
                approach,
                ResolutionApproach::Rebase
                    | ResolutionApproach::Merge
                    | ResolutionApproach::Unapply
            ),
            Self::FullyIntegrated => matches!(approach, ResolutionApproach::Delete),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    branch_id: BranchId,
    /// Used to ensure a given branch hasn't changed since the UI issued the command.
    #[serde(with = "gitbutler_serde::oid")]
    branch_tree: git2::Oid,
    approach: ResolutionApproach,
}

enum IntegrationResult {
    UpdatedObjects { head: git2::Oid, tree: git2::Oid },
    UnapplyBranch,
    DeleteBranch,
}

pub struct UpstreamIntegrationContext<'a> {
    _permission: Option<&'a mut WorktreeWritePermission>,
    repository: &'a git2::Repository,
    virtual_branches_in_workspace: Vec<Branch>,
    new_target: git2::Commit<'a>,
    old_target: git2::Commit<'a>,
    target_branch_name: String,
}

impl<'a> UpstreamIntegrationContext<'a> {
    pub(crate) fn open(
        command_context: &'a CommandContext,
        target_commit_oid: Option<git2::Oid>,
        permission: &'a mut WorktreeWritePermission,
    ) -> Result<Self> {
        let virtual_branches_handle = command_context.project().virtual_branches();
        let target = virtual_branches_handle.get_default_target()?;
        let repository = command_context.repository();
        let target_branch = repository
            .find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("Branch not found"))?;

        let new_target = target_commit_oid.map_or_else(
            || target_branch.get().peel_to_commit(),
            |oid| repository.find_commit(oid),
        )?;

        let old_target = repository.find_commit(target.sha)?;
        let virtual_branches_in_workspace = virtual_branches_handle.list_branches_in_workspace()?;

        Ok(Self {
            _permission: Some(permission),
            repository,
            new_target,
            old_target,
            virtual_branches_in_workspace,
            target_branch_name: target.branch.branch().to_string(),
        })
    }
}

pub fn upstream_integration_statuses(
    context: &UpstreamIntegrationContext,
) -> Result<BranchStatuses> {
    let UpstreamIntegrationContext {
        repository,
        new_target,
        old_target,
        virtual_branches_in_workspace,
        ..
    } = context;
    // look up the target and see if there is a new oid
    let old_target_tree = repository.find_real_tree(old_target, Default::default())?;
    let new_target_tree = repository.find_real_tree(new_target, Default::default())?;

    if new_target.id() == old_target.id() {
        return Ok(BranchStatuses::UpToDate);
    };

    let statuses = virtual_branches_in_workspace
        .iter()
        .map(|virtual_branch| {
            let tree = repository.find_tree(virtual_branch.tree)?;
            let head = repository.find_commit(virtual_branch.head)?;
            let head_tree = repository.find_real_tree(&head, Default::default())?;

            // Try cherry pick the branch's head commit onto the target to
            // see if it conflics. This is equivalent to doing a merge
            // but accounts for the commit being conflicted.

            let has_commits = virtual_branch.head != old_target.id();
            let has_uncommited_changes = head_tree.id() != tree.id();

            // Is the branch completly empty?
            {
                if !has_commits && !has_uncommited_changes {
                    return Ok((virtual_branch.id, BranchStatus::Empty));
                };
            }

            let head_merge_index =
                repository.merge_trees(&old_target_tree, &new_target_tree, &head_tree, None)?;
            let mut tree_merge_index =
                repository.merge_trees(&old_target_tree, &new_target_tree, &tree, None)?;

            // Is the branch conflicted?
            // A branch can't be integrated if its conflicted
            {
                let commits_conflicted = head_merge_index.has_conflicts();

                // See whether uncommited changes are potentially conflicted
                let potentially_conflicted_uncommited_changes = if has_uncommited_changes {
                    // If the commits are conflicted, we can guarentee that the
                    // tree will be conflicted.
                    if commits_conflicted {
                        true
                    } else {
                        tree_merge_index.has_conflicts()
                    }
                } else {
                    // If there are no uncommited changes, then there can't be
                    // any conflicts.
                    false
                };

                if commits_conflicted || potentially_conflicted_uncommited_changes {
                    return Ok((
                        virtual_branch.id,
                        BranchStatus::Conflicted {
                            potentially_conflicted_uncommited_changes,
                        },
                    ));
                }
            }

            // Is the branch fully integrated?
            {
                // We're safe to write the tree as we've ensured it's
                // unconflicted in the previous test.
                let tree_merge_index_tree = tree_merge_index.write_tree_to(repository)?;

                // Identical trees will have the same Oid so we can compare
                // the two
                if tree_merge_index_tree == new_target_tree.id() {
                    return Ok((virtual_branch.id, BranchStatus::FullyIntegrated));
                }
            }

            Ok((virtual_branch.id, BranchStatus::SaflyUpdatable))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(BranchStatuses::UpdatesRequired(statuses))
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

        let BranchStatuses::UpdatesRequired(statuses) = statuses else {
            bail!("Branches are all up to date")
        };

        if resolutions.len() != context.virtual_branches_in_workspace.len() {
            bail!("Chosen resolutions do not match quantity of applied virtual branches")
        }

        let all_resolutions_are_up_to_date = resolutions.iter().all(|resolution| {
            // This is O(n^2), in reality, n is unlikly to be more than 3 or 4
            let Some(branch) = context
                .virtual_branches_in_workspace
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
        for (branch_id, integration_result) in &integration_results {
            if !matches!(integration_result, IntegrationResult::DeleteBranch) {
                continue;
            };

            let branch = virtual_branches_state.get_branch(*branch_id)?;
            virtual_branches_state.delete_branch_entry(branch_id)?;
            command_context.delete_branch_reference(&branch)?;
        }

        let permission = context._permission.expect("Permission provided above");

        // Unapply branches
        for (branch_id, integration_result) in &integration_results {
            if !matches!(integration_result, IntegrationResult::UnapplyBranch) {
                continue;
            };

            command_context
                .branch_manager()
                .save_and_unapply(*branch_id, permission)?;
        }

        let mut branches = virtual_branches_state.list_branches_in_workspace()?;

        // Update branch trees
        for (branch_id, integration_result) in &integration_results {
            let IntegrationResult::UpdatedObjects { head, tree } = integration_result else {
                continue;
            };

            let Some(branch) = branches.iter_mut().find(|branch| branch.id == *branch_id) else {
                continue;
            };

            branch.head = *head;
            branch.tree = *tree;

            virtual_branches_state.set_branch(branch.clone())?;
        }

        // Now that we've potentially updated the branch trees, lets checkout
        // the result of merging them all together.
        checkout_branch_trees(command_context, permission)?;

        virtual_branches_state.set_default_target(Target {
            sha: context.new_target.id(),
            ..default_target
        })?;

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
    let repo = command_context.repository();
    let new_target_id = context.new_target.id();
    let old_target_id = context.old_target.id();
    let fork_point = repo.merge_base(old_target_id, new_target_id)?;

    match resolution_approach {
        BaseBranchResolutionApproach::HardReset => Ok(new_target_id),
        BaseBranchResolutionApproach::Merge => {
            let new_head = gitbutler_merge_commits(
                repo,
                context.old_target,
                context.new_target,
                &context.target_branch_name,
                &context.target_branch_name,
            )?;

            Ok(new_head.id())
        }
        BaseBranchResolutionApproach::Rebase => {
            let commits = repo.l(old_target_id, LogUntil::Commit(fork_point))?;
            let new_head = cherry_rebase_group(repo, new_target_id, &commits, true)?;

            Ok(new_head)
        }
    }
}

fn compute_resolutions(
    context: &UpstreamIntegrationContext,
    resolutions: &[Resolution],
    base_branch_resolution_approach: Option<BaseBranchResolutionApproach>,
) -> Result<Vec<(BranchId, IntegrationResult)>> {
    let UpstreamIntegrationContext {
        repository,
        new_target,
        old_target,
        virtual_branches_in_workspace,
        target_branch_name,
        ..
    } = context;

    let results = resolutions
        .iter()
        .map(|resolution| {
            let Some(virtual_branch) = virtual_branches_in_workspace
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
                    let target_commit = repository.find_commit(virtual_branch.head)?;

                    let new_head = gitbutler_merge_commits(
                        repository,
                        target_commit,
                        new_target.clone(),
                        &virtual_branch.name,
                        target_branch_name,
                    )?;

                    let head = repository.find_commit(virtual_branch.head)?;
                    let tree = repository.find_tree(virtual_branch.tree)?;

                    // Rebase tree
                    let author_signature = signature(SignaturePurpose::Author)
                        .context("Failed to get gitbutler signature")?;
                    let committer_signature = signature(SignaturePurpose::Committer)
                        .context("Failed to get gitbutler signature")?;
                    let committed_tree = repository.commit(
                        None,
                        &author_signature,
                        &committer_signature,
                        "Uncommited changes",
                        &tree,
                        &[&head],
                    )?;

                    // Rebase commited tree
                    let new_commited_tree =
                        cherry_rebase_group(repository, new_head.id(), &[committed_tree], true)?;
                    let new_commited_tree = repository.find_commit(new_commited_tree)?;

                    if new_commited_tree.is_conflicted() {
                        Ok((
                            virtual_branch.id,
                            IntegrationResult::UpdatedObjects {
                                head: new_commited_tree.id(),
                                tree: repository
                                    .find_real_tree(&new_commited_tree, Default::default())?
                                    .id(),
                            },
                        ))
                    } else {
                        Ok((
                            virtual_branch.id,
                            IntegrationResult::UpdatedObjects {
                                head: new_head.id(),
                                tree: new_commited_tree.tree_id(),
                            },
                        ))
                    }
                }
                ResolutionApproach::Rebase => {
                    // Rebase the commits, then try rebasing the tree. If
                    // the tree ends up conflicted, commit the tree.

                    // If the base branch needs to resolve its divergence
                    // pick only the commits that are ahead of the old target head
                    let lower_bound = if base_branch_resolution_approach.is_some() {
                        old_target.id()
                    } else {
                        new_target.id()
                    };

                    // Rebase virtual branches' commits
                    let virtual_branch_commits =
                        repository.l(virtual_branch.head, LogUntil::Commit(lower_bound))?;

                    let new_head = cherry_rebase_group(
                        repository,
                        new_target.id(),
                        &virtual_branch_commits,
                        true,
                    )?;

                    let head = repository.find_commit(virtual_branch.head)?;
                    let tree = repository.find_tree(virtual_branch.tree)?;

                    // Rebase tree
                    let author_signature = signature(SignaturePurpose::Author)
                        .context("Failed to get gitbutler signature")?;
                    let committer_signature = signature(SignaturePurpose::Committer)
                        .context("Failed to get gitbutler signature")?;
                    let committed_tree = repository.commit(
                        None,
                        &author_signature,
                        &committer_signature,
                        "Uncommited changes",
                        &tree,
                        &[&head],
                    )?;

                    // Rebase commited tree
                    let new_commited_tree =
                        cherry_rebase_group(repository, new_head, &[committed_tree], true)?;
                    let new_commited_tree = repository.find_commit(new_commited_tree)?;

                    if new_commited_tree.is_conflicted() {
                        Ok((
                            virtual_branch.id,
                            IntegrationResult::UpdatedObjects {
                                head: new_commited_tree.id(),
                                tree: repository
                                    .find_real_tree(&new_commited_tree, Default::default())?
                                    .id(),
                            },
                        ))
                    } else {
                        Ok((
                            virtual_branch.id,
                            IntegrationResult::UpdatedObjects {
                                head: new_head,
                                tree: new_commited_tree.tree_id(),
                            },
                        ))
                    }
                }
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(results)
}

#[cfg(test)]
mod test {
    use gitbutler_branch::BranchOwnershipClaims;
    use gitbutler_testsupport::testing_repository::TestingRepository;
    use uuid::Uuid;

    use super::*;

    fn make_branch(head: git2::Oid, tree: git2::Oid) -> Branch {
        Branch {
            id: Uuid::new_v4().into(),
            name: "branchy branch".into(),
            notes: "bla bla bla".into(),
            source_refname: None,
            upstream: None,
            upstream_head: None,
            created_timestamp_ms: 69420,
            updated_timestamp_ms: 69420,
            tree,
            head,
            ownership: BranchOwnershipClaims::default(),
            order: 0,
            selected_for_changes: None,
            allow_rebasing: true,
            in_workspace: true,
            not_in_workspace_wip_change_id: None,
            heads: Default::default(),
        }
    }

    #[test]
    fn test_up_to_date_if_head_commits_equivalent() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let head_commit = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target: head_commit.clone(),
            new_target: head_commit,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpToDate,
        )
    }

    #[test]
    fn test_updates_required_if_new_head_ahead() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let old_target = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);
        let new_target = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "qux")]);

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![]),
        )
    }

    #[test]
    fn test_empty_branch() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let old_target = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);
        let new_target = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "qux")]);

        let branch = make_branch(old_target.id(), old_target.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(branch.id, BranchStatus::Empty)]),
        )
    }

    #[test]
    fn test_conflicted_head_branch() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let old_target = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);
        let branch_head = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "fux")]);
        let new_target = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "qux")]);

        let branch = make_branch(branch_head.id(), branch_head.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target: new_target.clone(),
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(
                branch.id,
                BranchStatus::Conflicted {
                    potentially_conflicted_uncommited_changes: false
                }
            )]),
        );

        let updates = compute_resolutions(
            &context,
            &[Resolution {
                branch_id: branch.id,
                branch_tree: branch.tree,
                approach: ResolutionApproach::Rebase,
            }],
            None,
        )
        .unwrap();

        assert_eq!(updates.len(), 1);
        let IntegrationResult::UpdatedObjects { head, tree } = updates[0].1 else {
            panic!("Should be variant UpdatedObjects")
        };

        let head_commit = test_repository.repository.find_commit(head).unwrap();
        assert_eq!(head_commit.parent(0).unwrap().id(), new_target.id());
        assert!(head_commit.is_conflicted());

        let head_tree = test_repository
            .repository
            .find_real_tree(&head_commit, Default::default())
            .unwrap();
        assert_eq!(head_tree.id(), tree)
    }

    #[test]
    fn test_conflicted_tree_branch() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let old_target = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);
        let branch_head = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "fux")]);
        let new_target = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "qux")]);

        let branch = make_branch(old_target.id(), branch_head.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(
                branch.id,
                BranchStatus::Conflicted {
                    potentially_conflicted_uncommited_changes: true
                }
            )]),
        )
    }

    #[test]
    fn test_conflicted_head_and_tree_branch() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let old_target = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);
        let branch_head = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "fux")]);
        let branch_tree = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "bax")]);
        let new_target = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "qux")]);

        let branch = make_branch(branch_head.id(), branch_tree.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(
                branch.id,
                BranchStatus::Conflicted {
                    potentially_conflicted_uncommited_changes: true
                }
            )]),
        )
    }

    #[test]
    fn test_integrated() {
        let test_repository = TestingRepository::open();
        let initial_commit = test_repository.commit_tree(None, &[("foo.txt", "bar")]);
        let old_target = test_repository.commit_tree(Some(&initial_commit), &[("foo.txt", "baz")]);
        let new_target = test_repository.commit_tree(Some(&old_target), &[("foo.txt", "qux")]);

        let branch = make_branch(new_target.id(), new_target.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(branch.id, BranchStatus::FullyIntegrated)]),
        )
    }

    #[test]
    fn test_integrated_commit_with_uncommited_changes() {
        let test_repository = TestingRepository::open();
        let initial_commit =
            test_repository.commit_tree(None, &[("foo.txt", "bar"), ("bar.txt", "bar")]);
        let old_target = test_repository.commit_tree(
            Some(&initial_commit),
            &[("foo.txt", "baz"), ("bar.txt", "bar")],
        );
        let new_target = test_repository
            .commit_tree(Some(&old_target), &[("foo.txt", "qux"), ("bar.txt", "bar")]);
        let tree = test_repository
            .commit_tree(Some(&old_target), &[("foo.txt", "baz"), ("bar.txt", "qux")]);

        let branch = make_branch(new_target.id(), tree.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(branch.id, BranchStatus::SaflyUpdatable)]),
        )
    }

    #[test]
    fn test_safly_updatable() {
        let test_repository = TestingRepository::open();
        let initial_commit =
            test_repository.commit_tree(None, &[("files-one.txt", "foo"), ("file-two.txt", "foo")]);
        let old_target = test_repository.commit_tree(
            Some(&initial_commit),
            &[("file-one.txt", "bar"), ("file-two.txt", "foo")],
        );
        let new_target = test_repository.commit_tree(
            Some(&old_target),
            &[("file-one.txt", "baz"), ("file-two.txt", "foo")],
        );

        let branch_head = test_repository.commit_tree(
            Some(&old_target),
            &[("file-one.txt", "bar"), ("file-two.txt", "bar")],
        );
        let branch_tree = test_repository.commit_tree(
            Some(&branch_head),
            &[("file-one.txt", "bar"), ("file-two.txt", "baz")],
        );

        let branch = make_branch(branch_head.id(), branch_tree.tree_id());

        let context = UpstreamIntegrationContext {
            _permission: None,
            old_target,
            new_target,
            repository: &test_repository.repository,
            virtual_branches_in_workspace: vec![branch.clone()],
            target_branch_name: "main".to_string(),
        };

        assert_eq!(
            upstream_integration_statuses(&context).unwrap(),
            BranchStatuses::UpdatesRequired(vec![(branch.id, BranchStatus::SaflyUpdatable)]),
        )
    }
}
