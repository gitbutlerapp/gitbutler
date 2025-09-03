use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use but_graph::VirtualBranchesTomlMetadata;
use but_rebase::{RebaseOutput, RebaseStep};
use but_workspace::stack_ext::StackExt;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{
    logging::{LogUntil, RepositoryExt as _},
    rebase::gitbutler_merge_commits,
    RepositoryExt as _,
};
use gitbutler_stack::StackId;
use gitbutler_workspace::branch_trees::{update_uncommited_changes, WorkspaceState};
use gix::{refs::FullName, ObjectId};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    upstream_integration::{as_buckets, flatten_buckets},
    VirtualBranchesExt as _,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum InteractiveIntegrationStep {
    Skip {
        id: Uuid,
        #[serde(with = "gitbutler_serde::object_id", rename = "commitId")]
        commit_id: ObjectId,
    },
    Pick {
        id: Uuid,
        #[serde(with = "gitbutler_serde::object_id", rename = "commitId")]
        commit_id: ObjectId,
    },
    Squash {
        id: Uuid,
        #[serde(with = "gitbutler_serde::object_id", rename = "commitA")]
        commit_a: ObjectId,
        #[serde(with = "gitbutler_serde::object_id", rename = "commitB")]
        commit_b: ObjectId,
    },
}

/// Get the initial integration steps for a branch.
///
/// This basically just lists the upstream and local commits in the display order (child to parent) and creates a `Pick` step for each.
/// The user can then modify this in the UI.
pub fn get_initial_integration_steps_for_branch(
    ctx: &CommandContext,
    stack_id: Option<StackId>,
    branch_name: String,
) -> Result<Vec<InteractiveIntegrationStep>> {
    let repo = ctx.gix_repo()?;
    let project = ctx.project();
    let meta =
        VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))?;
    let stack_details = but_workspace::stack_details_v3(stack_id, &repo, &meta)?;

    let branch_details = stack_details
        .branch_details
        .into_iter()
        .find(|b| b.name == branch_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Series '{}' not found in stack '{:?}'",
                branch_name,
                stack_id
            )
        })?;

    let mut initial_steps = vec![];

    for upstream_commit in branch_details.upstream_commits {
        initial_steps.push(InteractiveIntegrationStep::Pick {
            id: Uuid::new_v4(),
            commit_id: upstream_commit.id,
        });
    }

    for commit in branch_details.commits {
        initial_steps.push(InteractiveIntegrationStep::Pick {
            id: Uuid::new_v4(),
            commit_id: commit.id,
        });
    }

    Ok(initial_steps)
}

pub fn integrate_upstream_commits_for_series(
    ctx: &CommandContext,
    stack_id: StackId,
    perm: &mut WorktreeWritePermission,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let repo = ctx.repo();
    let vb_state = ctx.project().virtual_branches();

    let stack = vb_state.get_stack_in_workspace(stack_id)?;
    let branches = stack.branches();

    let default_target = vb_state.get_default_target()?;
    let remote = default_target.push_remote_name();

    let subject_branch = branches
        .iter()
        .find(|branch| branch.name() == &series_name)
        .ok_or(anyhow!("Series not found"))?;
    let upstream_reference = subject_branch.remote_reference(remote.as_str());
    let remote_head = repo.find_reference(&upstream_reference)?.peel_to_commit()?;
    let gix_repo = ctx.gix_repo()?;

    let series_head = subject_branch.head_oid(&gix_repo)?;
    let series_head = repo.find_commit(series_head.to_git2())?;

    let strategy = integration_strategy.unwrap_or_else(|| {
        let do_rebease = stack.allow_rebasing
            || Some(subject_branch.name()) != branches.first().map(|b| b.name());
        if do_rebease {
            IntegrationStrategy::Rebase
        } else {
            IntegrationStrategy::Merge
        }
    });

    let integrate_upstream_context = IntegrateUpstreamContext {
        repo,
        target_branch_head: default_target.sha,
        branch_name: subject_branch.name(),
        branch_full_name: subject_branch.full_name()?,
        remote_head: remote_head.id(),
        remote_branch_name: &subject_branch.remote_reference(&remote),
        strategy,
        stack_steps: stack.as_rebase_steps(ctx, &gix_repo)?,
        gix_repo: &gix_repo,
    };

    let ((head, tree), _new_series_head, rebase_output) =
        integrate_upstream_context.inner_integrate_upstream_commits_for_series(series_head.id())?;

    let mut branch = stack.clone();
    branch.set_stack_head(&vb_state, &gix_repo, head, tree)?;
    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    update_uncommited_changes(ctx, old_workspace, new_workspace, perm)?;
    branch.set_heads_from_rebase_output(ctx, rebase_output.references)?;
    // branch.replace_head(ctx, &series_head, &repo.find_commit(new_series_head)?)?;
    crate::integration::update_workspace_commit(&vb_state, ctx)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IntegrationStrategy {
    Merge,
    Rebase,
}

struct IntegrateUpstreamContext<'a, 'b> {
    repo: &'a git2::Repository,
    /// GitButler's target branch
    target_branch_head: git2::Oid,

    /// The name of the local branch
    branch_name: &'b str,
    branch_full_name: FullName,

    /// The remote branch head
    remote_head: git2::Oid,
    /// The name of the remote branch
    remote_branch_name: &'b str,

    /// Strategy to use when integrating the upstream commits
    strategy: IntegrationStrategy,

    stack_steps: Vec<RebaseStep>,
    gix_repo: &'a gix::Repository,
}

impl IntegrateUpstreamContext<'_, '_> {
    /// Unlike the `inner_integrate_upstream_commits` method, this will do the rebase in two steps.
    /// First it will rebase the series head and it's remote commits, then it will rebase any remaining on the stack.
    fn inner_integrate_upstream_commits_for_series(
        &self,
        series_head: git2::Oid,
    ) -> Result<((git2::Oid, Option<git2::Oid>), git2::Oid, RebaseOutput)> {
        let (new_stack_head, new_series_head, rebase_output) = match self.strategy {
            IntegrationStrategy::Merge => {
                // If rebase is not allowed AND this is the latest series - create a merge commit on top
                let series_head_commit = self.repo.find_commit(series_head)?;
                let remote_head_commit = self.repo.find_commit(self.remote_head)?;
                let merge_commit = gitbutler_merge_commits(
                    self.repo,
                    series_head_commit,
                    remote_head_commit,
                    self.branch_name,        // for error messages only
                    self.remote_branch_name, // for error messages only
                )?;
                let mut steps = self.stack_steps.clone();
                // Go over the steps, and immediatelly after the series head, insert the merge commit
                for (i, step) in steps.iter().enumerate() {
                    if let RebaseStep::Pick { commit_id, .. } = step {
                        if commit_id == &series_head.to_gix() {
                            steps.insert(
                                i + 1,
                                RebaseStep::Pick {
                                    commit_id: merge_commit.id().to_gix(),
                                    new_message: None,
                                },
                            );
                            break;
                        }
                    }
                }

                let merge_base = self.repo.merge_base_octopussy(&[
                    self.target_branch_head,
                    series_head,
                    self.remote_head,
                ])?;
                let mut rebase = but_rebase::Rebase::new(self.gix_repo, merge_base.to_gix(), None)?;
                rebase.steps(steps)?;
                rebase.rebase_noops(false);
                let output = rebase.rebase()?;
                let stack_head = output.top_commit.to_git2();
                (
                    stack_head,
                    new_series_head(&output, self.branch_name, &self.branch_full_name),
                    output,
                )
            }
            IntegrationStrategy::Rebase => {
                // Get the commits to rebase for the series
                let OrderCommitsResult {
                    merge_base,
                    ordered_commits,
                } = order_commits_for_rebasing(
                    self.repo,
                    self.target_branch_head,
                    series_head,
                    self.remote_head,
                )?;

                let mut buckets = as_buckets(self.stack_steps.clone());
                let (_, steps) = buckets
                    .iter_mut()
                    .find(|(r, _)| match r {
                        but_core::Reference::Virtual(name) => name == self.branch_name,
                        but_core::Reference::Git(name) => name == &self.branch_full_name,
                    })
                    .ok_or_else(|| {
                        anyhow::anyhow!("failed to find branch to rebase in the stack rebase steps")
                    })?;
                // replace the steps with the ordered commits
                *steps = ordered_commits
                    .iter()
                    .rev()
                    .map(|commit_id| RebaseStep::Pick {
                        commit_id: commit_id.to_gix(),
                        new_message: None,
                    })
                    .collect();
                let updated_steps = flatten_buckets(buckets);
                let mut rebase = but_rebase::Rebase::new(self.gix_repo, merge_base.to_gix(), None)?;
                rebase.steps(updated_steps)?;
                rebase.rebase_noops(false);
                let output = rebase.rebase()?;
                let stack_head = output.top_commit.to_git2();
                (
                    stack_head,
                    new_series_head(&output, self.branch_name, &self.branch_full_name),
                    output,
                )
            }
        };
        Ok(((new_stack_head, None), new_series_head, rebase_output))
    }
}

fn new_series_head(
    output: &RebaseOutput,
    branch_name: &str,
    full_branch_name: &FullName,
) -> git2::Oid {
    output
        .references
        .iter()
        .find(|r| match r.reference.clone() {
            but_core::Reference::Virtual(name) => name == branch_name,
            but_core::Reference::Git(name) => &name == full_branch_name,
        })
        .map(|r| r.commit_id)
        .expect("failed to find the new series head")
        .to_git2()
}

struct OrderCommitsResult {
    merge_base: git2::Oid,
    ordered_commits: Vec<git2::Oid>,
}

/// Interweave the local and remote commits in the correct order.
///
/// The order is determined in the following way:
/// 1. Whenever there are discrepancies defer to the remote branch.
/// 2. Match the commits by their change id.
/// 3. If a remote commit does not match the any local commits, insert it in the order it came.
/// 4. Insert the unmatched local commits above its parents.
fn order_commits_for_rebasing(
    repo: &git2::Repository,
    target_branch_head: git2::Oid,
    local_head: git2::Oid,
    remote_head: git2::Oid,
) -> Result<OrderCommitsResult> {
    let merge_base = repo.merge_base_octopussy(&[target_branch_head, local_head, remote_head])?;

    // 1. Build the change id map for the local branch.
    let local_branch_commits = repo.l(local_head, LogUntil::Commit(target_branch_head), false)?;
    let remote_branch_commits = repo.l(remote_head, LogUntil::Commit(merge_base), false)?;

    let change_id_map = build_change_id_map(&local_branch_commits, repo)?;

    let mut ordered_for_zipping: Vec<(Option<String>, git2::Oid)> = vec![];
    let mut added_local_commits: HashSet<git2::Oid> = HashSet::new();

    // 2. Start populating the ordered list with the remote commits
    // and their respective local commits (matched by change id).
    process_remote_commits_for_zipping(
        repo,
        &change_id_map,
        remote_branch_commits,
        &mut ordered_for_zipping,
        &mut added_local_commits,
    )?;

    // 3. Add the remaining local commits.
    insert_remaining_local_commits(
        repo,
        target_branch_head,
        local_branch_commits,
        &added_local_commits,
        &mut ordered_for_zipping,
    )?;

    let ordered_for_zipping = ordered_for_zipping
        .iter()
        .map(|(_, id)| *id)
        .dedup()
        .collect();

    Ok(OrderCommitsResult {
        merge_base,
        ordered_commits: ordered_for_zipping,
    })
}

/// Insert the remaining local commits in the correct order.
///
/// All the local commits that were not matched by an incoming change id need to be inserted
/// at the right position in the ordered list.
fn insert_remaining_local_commits(
    repo: &git2::Repository,
    merge_base: git2::Oid,
    local_branch_commits: Vec<git2::Oid>,
    added_local_commits: &HashSet<git2::Oid>,
    ordered_for_zipping: &mut Vec<(Option<String>, git2::Oid)>,
) -> Result<(), anyhow::Error> {
    // We iterate over them in reverse order so we can insert them at the correct position
    let branch_commits_remaining = local_branch_commits
        .iter()
        .filter(|commit_id| !added_local_commits.contains(commit_id))
        .rev();

    for remaining_commit_id in branch_commits_remaining {
        let insertion_index = find_insertion_index_for_remaining_commit(
            repo,
            remaining_commit_id,
            merge_base,
            ordered_for_zipping,
        )?;

        let remaining_commit = repo.find_commit(*remaining_commit_id)?;
        ordered_for_zipping.insert(
            insertion_index,
            (remaining_commit.change_id(), *remaining_commit_id),
        );
    }
    Ok(())
}

/// Find the correct insertion index for the remaining commit
///
/// We want to insert the remaining commit on top of all of its parents
/// in order to minimize the possibility of conflicts.
/// We also want to insert the remaining commit on top of as few incoming remote
/// commits as possible.
///
/// The insertion index is the index of the first parent of the remaining commit.
fn find_insertion_index_for_remaining_commit(
    repo: &git2::Repository,
    remaining_commit_id: &git2::Oid,
    merge_base: git2::Oid,
    ordered_for_zipping: &[(Option<String>, git2::Oid)],
) -> Result<usize, anyhow::Error> {
    let remaining_commit_id_parent_ids =
        repo.l(*remaining_commit_id, LogUntil::Commit(merge_base), false)?;
    let remaining_commit_id_parents = remaining_commit_id_parent_ids
        .iter()
        .filter_map(|id| repo.find_commit(*id).ok())
        .collect_vec();

    let mut insertion_index = None;

    for (index, oredered_element) in ordered_for_zipping.iter().enumerate() {
        match &oredered_element.0 {
            Some(ordered_change_id) => {
                let found_parent = remaining_commit_id_parents.iter().any(|parent| {
                    if let Some(parent_change_id) = parent.change_id() {
                        parent_change_id == *ordered_change_id
                    } else {
                        parent.id() == oredered_element.1
                    }
                });

                if found_parent {
                    if let Some(actual_insertion_index) = insertion_index {
                        insertion_index = Some(std::cmp::min(index, actual_insertion_index));
                    } else {
                        insertion_index = Some(index);
                    }
                }
            }
            None => {
                let found_parent = remaining_commit_id_parents
                    .iter()
                    .any(|parent| parent.id() == oredered_element.1);

                if found_parent {
                    if let Some(actual_insertion_index) = insertion_index {
                        insertion_index = Some(std::cmp::min(index, actual_insertion_index));
                    } else {
                        insertion_index = Some(index);
                    }
                }
            }
        }
    }

    Ok(insertion_index.unwrap_or_default())
}

/// Start populating the ordered list with the remote commits and matching local commits.
///
/// In the order of the remote commits, we add the remote and then local commits that match the change id.
/// If a change id is not unique, we add all the local commits that match the change id
/// and once added, we skip the change id the next time we see it.
///
/// That process is not ideal, because we assume that the best position to insert the local commits is
/// right after the first remote commit that matches the change id.
/// And we are force to assume that because there is no way to know better, yet.
fn process_remote_commits_for_zipping<'a>(
    repo: &'a git2::Repository,
    change_id_map: &'a HashMap<String, Vec<git2::Oid>>,
    remote_branch_commits: Vec<git2::Oid>,
    ordered_for_zipping: &'a mut Vec<(Option<String>, git2::Oid)>,
    added_local_commits: &'a mut HashSet<git2::Oid>,
) -> Result<(), anyhow::Error> {
    let mut visited_change_ids = HashSet::new();
    for remote_commit_id in remote_branch_commits {
        let remote_commit = repo.find_commit(remote_commit_id)?;

        let change_id = match remote_commit.change_id() {
            Some(change_id) => change_id,
            None => {
                ordered_for_zipping.push((None, remote_commit_id));
                continue;
            }
        };

        ordered_for_zipping.push((Some(change_id.clone()), remote_commit_id));

        if visited_change_ids.contains(&change_id) {
            continue;
        }

        if let Some(local_commit_id) = change_id_map.get(&change_id) {
            local_commit_id.iter().for_each(|id| {
                ordered_for_zipping.push((Some(change_id.clone()), *id));
                added_local_commits.insert(*id);
            });
        }

        visited_change_ids.insert(change_id);
    }
    Ok(())
}

/// Build a map of change ids to local commits
///
/// Usually, a change id is unique to a commit, but it's not a certainty.
fn build_change_id_map(
    local_branch_commits: &[git2::Oid],
    repo: &git2::Repository,
) -> Result<HashMap<String, Vec<git2::Oid>>> {
    let mut change_id_map = HashMap::new();
    for commit_id in local_branch_commits {
        let commit = repo.find_commit(*commit_id)?;
        let change_id = match commit.change_id() {
            Some(change_id) => change_id,
            None => continue,
        };
        change_id_map
            .entry(change_id)
            .or_insert_with(Vec::new)
            .push(*commit_id);
    }
    Ok(change_id_map)
}

#[cfg(test)]
mod test {
    use crate::branch_upstream_integration::IntegrateUpstreamContext;
    use gitbutler_testsupport::testing_repository::TestingRepository;

    mod inner_integrate_upstream_commits {
        use but_rebase::RebaseStep;
        use gitbutler_oxidize::OidExt;
        use gitbutler_repo::logging::LogUntil;
        use gitbutler_repo::logging::RepositoryExt as _;

        use crate::branch_upstream_integration::IntegrationStrategy;

        use super::*;

        /// Full Stack: Base -> A -> B -> C -> D
        /// Series One:         A -> B
        /// Series Two:                   C -> D
        /// Series One Remote:  A -> B -> X -> Y
        ///
        /// Result Stack: Base -> A -> B -> X -> Y -> C -> D
        /// Result Series One Head: Y
        #[test]
        fn other_added_remote_changes_multiple_series() {
            let repo = TestingRepository::open();

            let base_commit = repo.commit_tree(None, &[("foo.txt", "foo")]);
            let local_a = repo.commit_tree(Some(&base_commit), &[("foo.txt", "foo1")]);
            let local_b = repo.commit_tree(Some(&local_a), &[("foo.txt", "foo2")]);
            let local_c = repo.commit_tree(Some(&local_b), &[("foo.txt", "fooC")]);
            let local_d = repo.commit_tree(Some(&local_c), &[("foo.txt", "fooD")]);

            let steps = vec![
                RebaseStep::Pick {
                    commit_id: local_a.id().to_gix(),
                    new_message: None,
                },
                RebaseStep::Pick {
                    commit_id: local_b.id().to_gix(),
                    new_message: None,
                },
                RebaseStep::Reference(but_core::Reference::Virtual("One".to_string())),
                RebaseStep::Pick {
                    commit_id: local_c.id().to_gix(),
                    new_message: None,
                },
                RebaseStep::Pick {
                    commit_id: local_d.id().to_gix(),
                    new_message: None,
                },
                RebaseStep::Reference(but_core::Reference::Virtual("Two".to_string())),
            ];

            let remote_x = repo.commit_tree(Some(&local_b), &[("foo.txt", "foo3")]);
            let remote_y = repo.commit_tree(Some(&remote_x), &[("foo.txt", "foo4")]);

            let ctx = IntegrateUpstreamContext {
                repo: &repo.repository,
                target_branch_head: base_commit.id(),
                branch_name: "One",
                branch_full_name: "refs/heads/One".try_into().unwrap(),
                remote_head: remote_y.id(),
                remote_branch_name: "One",
                strategy: IntegrationStrategy::Rebase,
                stack_steps: steps,
                gix_repo: &repo.gix_repository(),
            };

            let ((head, _tree), new_series_head, _rebase_output) = ctx
                .inner_integrate_upstream_commits_for_series(local_b.id()) // series head is earlier than stack head
                .unwrap();
            assert_eq!(new_series_head, remote_y.id());
            assert_eq!(
                repo.repository
                    .l(head, LogUntil::Commit(base_commit.id()), false)
                    .unwrap()
                    .iter()
                    .map(|c| {
                        let commit = repo.repository.find_commit(*c).unwrap();
                        commit.message().unwrap().to_string()
                    })
                    .collect::<Vec<_>>(),
                vec![
                    local_d.message().unwrap(),
                    local_c.message().unwrap(),
                    remote_y.message().unwrap(),
                    remote_x.message().unwrap(),
                    local_b.message().unwrap(),
                    local_a.message().unwrap()
                ],
            );
        }
    }

    mod order_commits_for_rebasing {
        use crate::branch_upstream_integration::order_commits_for_rebasing;

        use super::*;

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B -> X -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> X -> Y
        #[test]
        fn other_added_remote_changes() {
            let test_repo = TestingRepository::open();

            let base_commit = test_repo.commit_tree(None, &[]);
            let local_a = test_repo.commit_tree_with_change_id(Some(&base_commit), "a", &[]);
            let local_b = test_repo.commit_tree_with_change_id(Some(&local_a), "b", &[]);

            let remote_x = test_repo.commit_tree_with_change_id(Some(&local_b), "x", &[]);
            let remote_y = test_repo.commit_tree_with_change_id(Some(&remote_x), "y", &[]);

            let commits = order_commits_for_rebasing(
                &test_repo.repository,
                base_commit.id(),
                local_b.id(),
                remote_y.id(),
            )
            .unwrap();

            assert_eq!(
                commits.ordered_commits,
                vec![remote_y.id(), remote_x.id(), local_b.id(), local_a.id()],
            );
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> A -> B' -> Y
        /// Trunk:  Base
        /// Result: Base -> A -> B -> B' -> Y
        #[test]
        fn modified_local_commit() {
            let test_repo = TestingRepository::open();

            let base_commit = test_repo.commit_tree(None, &[]);
            let local_a = test_repo.commit_tree_with_change_id(Some(&base_commit), "a", &[]);
            let local_b = test_repo.commit_tree_with_change_id(Some(&local_a), "b", &[]);

            // imagine someone on the remote rebased local_b
            let remote_b = test_repo.commit_tree_with_change_id(Some(&local_a), "b", &[]);
            let remote_y = test_repo.commit_tree_with_change_id(Some(&remote_b), "y", &[]);

            let commits = order_commits_for_rebasing(
                &test_repo.repository,
                base_commit.id(),
                local_b.id(),
                remote_y.id(),
            )
            .unwrap();

            assert_eq!(
                commits.ordered_commits,
                vec![remote_y.id(), remote_b.id(), local_b.id(), local_a.id()],
            );
        }

        /// Local:  Base -> A -> B
        /// Remote: Base -> M -> N -> A' -> B' -> Y
        /// Trunk:  Base -> M -> N
        /// Result: Base -> M -> N -> A -> A' -> B -> B' -> Y
        #[test]
        fn remote_includes_integrated_commits() {
            let test_repo = TestingRepository::open();

            // Setup:
            // (z)
            //  |
            //  y
            //  |
            //  x
            //  |
            // (n) (b)
            //  |   |
            //  m   a
            //  \   /
            //   base_commit
            let base_commit = test_repo.commit_tree(None, &[]);
            let trunk_m = test_repo.commit_tree_with_change_id(Some(&base_commit), "m", &[]);
            let trunk_n = test_repo.commit_tree_with_change_id(Some(&trunk_m), "n", &[]);

            let local_a = test_repo.commit_tree_with_change_id(Some(&base_commit), "a", &[]);
            let local_b = test_repo.commit_tree_with_change_id(Some(&local_a), "b", &[]);

            // imagine someone on the remote rebased local_a
            let remote_x = test_repo.commit_tree_with_change_id(Some(&trunk_n), "a", &[]);
            let remote_y = test_repo.commit_tree_with_change_id(Some(&remote_x), "b", &[]);
            let remote_z = test_repo.commit_tree_with_change_id(Some(&remote_y), "y", &[]);

            let commits = order_commits_for_rebasing(
                &test_repo.repository,
                trunk_n.id(),
                local_b.id(),
                remote_z.id(),
            )
            .unwrap();

            assert_eq!(
                commits.ordered_commits,
                vec![
                    remote_z.id(),
                    remote_y.id(),
                    local_b.id(),
                    remote_x.id(),
                    local_a.id(),
                    trunk_n.id(),
                    trunk_m.id()
                ],
            );
        }
    }
}
