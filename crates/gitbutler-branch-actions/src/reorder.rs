use anyhow::{Context, Result, bail};
use but_rebase::{RebaseOutput, RebaseStep};
use git2::Oid;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{Stack, StackId};

use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommited_changes};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::VirtualBranchesExt;

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
    stack_id: StackId,
    new_order: StackOrder,
    perm: &mut WorktreeWritePermission,
) -> Result<RebaseOutput> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let state = ctx.project().virtual_branches();
    let repo = ctx.repo();
    let mut stack = state.get_stack(stack_id)?;
    let current_order = commits_order(ctx, &stack)?;
    new_order.validate(current_order.clone())?;

    let gix_repo = ctx.gix_repo()?;
    let default_target = state.get_default_target()?;
    let default_target_commit = repo
        .find_reference(&default_target.branch.to_string())?
        .peel_to_commit()?;
    let merge_base = repo.merge_base(
        default_target_commit.id(),
        stack.head_oid(&gix_repo)?.to_git2(),
    )?;

    let mut steps: Vec<RebaseStep> = Vec::new();
    for series in new_order.series.iter().rev() {
        for oid in series.commit_ids.iter().rev() {
            steps.push(RebaseStep::Pick {
                commit_id: oid.to_gix(),
                new_message: None,
            });
        }
        steps.push(RebaseStep::Reference(but_core::Reference::Virtual(
            series.name.clone(),
        )));
    }
    let mut builder = but_rebase::Rebase::new(&gix_repo, merge_base.to_gix(), None)?;
    let builder = builder.steps(steps)?;
    builder.rebase_noops(false);
    let output = builder.rebase()?;

    let new_head = output.top_commit.to_git2();

    // Ensure the stack head is set to the new oid after rebasing
    stack.set_stack_head(&state, &gix_repo, new_head, None)?;

    stack.set_heads_from_rebase_output(ctx, output.references.clone())?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    // Even if this fails, it's not actionable
    let _ = update_uncommited_changes(ctx, old_workspace, new_workspace, perm);
    crate::integration::update_workspace_commit(&state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    Ok(output)
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

pub fn commits_order(ctx: &CommandContext, stack: &Stack) -> Result<StackOrder> {
    let order: Result<Vec<SeriesOrder>> = stack
        .branches()
        .iter()
        .filter(|b| !b.archived)
        .rev()
        .map(|b| {
            Ok(SeriesOrder {
                name: b.name().to_owned(),
                commit_ids: b
                    .commits(ctx, stack)?
                    .local_commits
                    .iter()
                    .rev()
                    .map(|c| c.id())
                    .collect(),
            })
        })
        .collect();
    Ok(StackOrder { series: order? })
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
