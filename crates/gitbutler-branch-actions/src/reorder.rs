use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use git2::{Commit, Oid};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_stack::{Series, StackId};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    branch_trees::{
        checkout_branch_trees, compute_updated_branch_head_for_commits, BranchHeadAndTree,
    },
    VirtualBranchesExt,
};

/// This API allows the client to reorder commits in a stack.
/// Commits may be moved within the same series or between different series.
/// Moving of series is not permitted.
///
/// # Errors
/// Errors out upon invalid stack order input. The following conditions are checked:
/// - The number of series in the order must match the number of series in the stack
/// - The series names in the reorder request must match the names in the stack
/// - The series themselves in the reorder request must be the same as the ones in the stack (this API is about moving commits, not series)
/// - The number of commits in the reorder request must match the number of commits in the stack
/// - The commit ids in the reorder request must be in the stack
pub fn reorder_stack(
    ctx: &CommandContext,
    branch_id: StackId,
    new_order: StackOrder,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let state = ctx.project().virtual_branches();
    let repo = ctx.repository();
    let mut stack = state.get_branch(branch_id)?;
    let all_series = stack.list_series(ctx)?;
    let current_order = series_order(&all_series);
    new_order.validate(current_order.clone())?;

    let default_target = state.get_default_target()?;
    let default_target_commit = repo
        .find_reference(&default_target.branch.to_string())?
        .peel_to_commit()?;
    let old_head = repo.find_commit(stack.head())?;
    let merge_base = repo.merge_base(default_target_commit.id(), stack.head())?;

    let ids_to_rebase = new_order
        .series
        .iter()
        .flat_map(|s| s.commit_ids.iter())
        .cloned()
        .collect_vec();
    let new_head = cherry_rebase_group(repo, merge_base, &ids_to_rebase)?;
    // Calculate the new head and tree
    let BranchHeadAndTree {
        head: new_head_oid,
        tree: new_tree_oid,
    } = compute_updated_branch_head_for_commits(repo, old_head.id(), old_head.tree_id(), new_head)?;

    // Ensure the stack head is set to the new oid after rebasing
    stack.set_stack_head(ctx, new_head_oid, Some(new_tree_oid))?;

    let mut new_heads: HashMap<String, Commit<'_>> = HashMap::new();
    let mut previous = merge_base;
    for series in new_order.series.iter().rev() {
        let commit = if let Some(commit_id) = series.commit_ids.first() {
            repo.find_commit(*commit_id)?
        } else {
            repo.find_commit(previous)?
        };
        previous = commit.id();
        new_heads.insert(series.name.clone(), commit);
    }
    // Set the series heads accordingly in one go
    stack.set_all_heads(ctx, new_heads)?;

    checkout_branch_trees(ctx, perm)?;
    crate::integration::update_workspace_commit(&state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

/// Represents the order of series (branches) and changes (commits) in a stack.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackOrder {
    /// The series are ordered from newest to oldest (most recent stacks go first)
    pub series: Vec<SeriesOrder>,
}

/// Represents the order of changes (commits) in a series (branch).
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesOrder {
    /// Unique name of the series (branch). Must already exist in the stack.
    pub name: String,
    /// This is the desired commit order for the series. Because the commits will be rabased,
    /// naturally, the the commit ids will be different afte updating.
    /// The changes are ordered from newest to oldest (most recent changes go first)
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub commit_ids: Vec<Oid>,
}

impl StackOrder {
    fn validate(&self, current_order: StackOrder) -> Result<()> {
        // Ensure the number of series is the same between the reorder update request and the stack
        if self.series.len() != current_order.series.len() {
            bail!(
                "The number of series in the order ({}) does not match the number of series in the stack ({})",
                self.series.len(),
                current_order.series.len()
            );
        }
        // Ensure that the names in the reorder update request match the names in the stack
        for series_order in &self.series {
            if !current_order
                .series
                .iter()
                .any(|s| s.name == series_order.name)
            {
                bail!("Series '{}' does not exist in the stack", series_order.name);
            }
        }
        // Ensure that the series themselves in the updater request are the same as the ones in the stack (this API is about moving commits, not series)
        for (new_order, current_order) in self.series.iter().zip(current_order.series.iter()) {
            if new_order.name != current_order.name {
                bail!(
                    "Series '{}' in the order does not match the series '{}' in the stack. Series can't be reordered with this API, it's only for commits",
                    new_order.name,
                    current_order.name
                );
            }
        }

        let new_order_commit_ids = self
            .series
            .iter()
            .flat_map(|s| s.commit_ids.iter())
            .cloned()
            .collect_vec();
        let current_order_commit_ids = current_order
            .series
            .iter()
            .flat_map(|s| s.commit_ids.iter())
            .cloned()
            .collect_vec();

        // Ensure that the number of commits in the order is the same as the number of commits in the stack
        if new_order_commit_ids.len() != current_order_commit_ids.len() {
            bail!(
                "The number of commits in the request order ({}) does not match the number of commits in the stack ({})",
                new_order_commit_ids.len(),
                current_order_commit_ids.len()
            );
        }
        // Ensure that every commit in the order is in the stack
        for commit_id in &new_order_commit_ids {
            if !current_order_commit_ids.contains(commit_id) {
                bail!("Commit '{}' does not exist in the stack", commit_id);
            }
        }

        // Ensure the new order is not a noop
        if self
            .series
            .iter()
            .map(|s| s.commit_ids.clone())
            .collect_vec()
            == current_order
                .series
                .iter()
                .map(|s| s.commit_ids.clone())
                .collect_vec()
        {
            bail!("The new order is the same as the current order");
        }

        Ok(())
    }
}

pub fn series_order(all_series: &[Series<'_>]) -> StackOrder {
    let series_order: Vec<SeriesOrder> = all_series
        .iter()
        .rev()
        .map(|series| {
            let commit_ids = series
                .local_commits
                .iter()
                .rev()
                .map(|commit| commit.id())
                .collect();
            SeriesOrder {
                name: series.head.name.clone(),
                commit_ids,
            }
        })
        .collect();
    StackOrder {
        series: series_order,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validation_ok() -> Result<()> {
        let new_order = StackOrder {
            series: vec![
                SeriesOrder {
                    name: "branch-2".to_string(),
                    commit_ids: vec![
                        Oid::from_str("6").unwrap(),
                        Oid::from_str("5").unwrap(),
                        Oid::from_str("4").unwrap(),
                    ],
                },
                SeriesOrder {
                    name: "branch-1".to_string(),
                    commit_ids: vec![
                        Oid::from_str("3").unwrap(),
                        Oid::from_str("1").unwrap(), // swapped with below
                        Oid::from_str("2").unwrap(),
                    ],
                },
            ],
        };
        let result = new_order.validate(existing_order());
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn noop_errors_out() -> Result<()> {
        let result = existing_order().validate(existing_order());
        assert_eq!(
            result.unwrap_err().to_string(),
            "The new order is the same as the current order"
        );
        Ok(())
    }

    #[test]
    fn non_existing_id_errors_out() -> Result<()> {
        let new_order = StackOrder {
            series: vec![
                SeriesOrder {
                    name: "branch-2".to_string(),
                    commit_ids: vec![
                        Oid::from_str("6").unwrap(),
                        Oid::from_str("5").unwrap(),
                        Oid::from_str("4").unwrap(),
                    ],
                },
                SeriesOrder {
                    name: "branch-1".to_string(),
                    commit_ids: vec![
                        Oid::from_str("3").unwrap(),
                        Oid::from_str("9").unwrap(), // does not exist
                        Oid::from_str("1").unwrap(),
                    ],
                },
            ],
        };
        let result = new_order.validate(existing_order());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Commit '9000000000000000000000000000000000000000' does not exist in the stack"
        );
        Ok(())
    }

    #[test]
    fn number_of_commits_mismatch_errors_out() -> Result<()> {
        let new_order = StackOrder {
            series: vec![
                SeriesOrder {
                    name: "branch-2".to_string(),
                    commit_ids: vec![
                        Oid::from_str("6").unwrap(),
                        Oid::from_str("5").unwrap(),
                        Oid::from_str("4").unwrap(),
                    ],
                },
                SeriesOrder {
                    name: "branch-1".to_string(),
                    commit_ids: vec![
                        Oid::from_str("3").unwrap(), // missing
                        Oid::from_str("1").unwrap(),
                    ],
                },
            ],
        };
        let result = new_order.validate(existing_order());
        assert_eq!(
            result.unwrap_err().to_string(),
            "The number of commits in the request order (5) does not match the number of commits in the stack (6)"
        );
        Ok(())
    }

    #[test]
    fn series_out_of_order_errors_out() -> Result<()> {
        let new_order = StackOrder {
            series: vec![
                SeriesOrder {
                    name: "branch-1".to_string(), // wrong order
                    commit_ids: vec![
                        Oid::from_str("6").unwrap(),
                        Oid::from_str("5").unwrap(),
                        Oid::from_str("4").unwrap(),
                    ],
                },
                SeriesOrder {
                    name: "branch-2".to_string(), // wrong order
                    commit_ids: vec![
                        Oid::from_str("3").unwrap(),
                        Oid::from_str("2").unwrap(),
                        Oid::from_str("1").unwrap(),
                    ],
                },
            ],
        };
        let result = new_order.validate(existing_order());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Series 'branch-1' in the order does not match the series 'branch-2' in the stack. Series can't be reordered with this API, it's only for commits"
        );
        Ok(())
    }

    #[test]
    fn different_series_name_errors_out() -> Result<()> {
        let new_order = StackOrder {
            series: vec![
                SeriesOrder {
                    name: "does-not-exist".to_string(), // invalid series name
                    commit_ids: vec![
                        Oid::from_str("6").unwrap(),
                        Oid::from_str("5").unwrap(),
                        Oid::from_str("4").unwrap(),
                    ],
                },
                SeriesOrder {
                    name: "branch-1".to_string(),
                    commit_ids: vec![
                        Oid::from_str("3").unwrap(),
                        Oid::from_str("2").unwrap(),
                        Oid::from_str("1").unwrap(),
                    ],
                },
            ],
        };
        let result = new_order.validate(existing_order());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Series 'does-not-exist' does not exist in the stack"
        );
        Ok(())
    }

    #[test]
    fn different_number_of_series_errors_out() -> Result<()> {
        let new_order = StackOrder {
            series: vec![SeriesOrder {
                name: "branch-1".to_string(),
                commit_ids: vec![
                    Oid::from_str("3").unwrap(),
                    Oid::from_str("2").unwrap(),
                    Oid::from_str("1").unwrap(),
                ],
            }],
        };
        let result = new_order.validate(existing_order());
        assert_eq!(
            result.unwrap_err().to_string(),
            "The number of series in the order (1) does not match the number of series in the stack (2)"
        );
        Ok(())
    }

    fn existing_order() -> StackOrder {
        StackOrder {
            series: vec![
                SeriesOrder {
                    name: "branch-2".to_string(),
                    commit_ids: vec![
                        Oid::from_str("6").unwrap(),
                        Oid::from_str("5").unwrap(),
                        Oid::from_str("4").unwrap(),
                    ],
                },
                SeriesOrder {
                    name: "branch-1".to_string(),
                    commit_ids: vec![
                        Oid::from_str("3").unwrap(),
                        Oid::from_str("2").unwrap(),
                        Oid::from_str("1").unwrap(),
                    ],
                },
            ],
        }
    }
}
