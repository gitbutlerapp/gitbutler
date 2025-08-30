use anyhow::{bail, Result};
use but_graph::VirtualBranchesTomlMetadata;
use but_rebase::{Rebase, RebaseStep};
use but_workspace::stack_ext::StackExt;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{update_uncommited_changes, WorkspaceState};
use gix::ObjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        #[serde(with = "gitbutler_serde::object_id_vec", rename = "commits")]
        commits: Vec<ObjectId>,
        message: Option<String>,
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

/// Integrate a branch with the given steps.
pub fn integrate_branch_with_steps(
    ctx: &CommandContext,
    stack_id: StackId,
    branch_name: String,
    steps: Vec<InteractiveIntegrationStep>,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let repository = ctx.gix_repo()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let mut source_stack = vb_state.get_stack_in_workspace(stack_id)?;
    let merge_base = source_stack.merge_base(ctx)?;

    let original_rebase_steps = source_stack.as_rebase_steps_rev(ctx, &repository)?;
    let mut new_rebase_steps = vec![];

    let branch_ref = repository
        .try_find_reference(&branch_name)?
        .ok_or_else(|| {
            anyhow::anyhow!("Source branch '{}' not found in repository", branch_name)
        })?;
    let branch_ref_name = branch_ref.name().to_owned();

    let mut inside_branch = false;

    for step in original_rebase_steps {
        if let RebaseStep::Reference(but_core::Reference::Git(name)) = &step {
            if *name == branch_ref_name {
                inside_branch = true;
            } else if inside_branch {
                inside_branch = false;
            }
        }

        if let RebaseStep::Reference(but_core::Reference::Virtual(name)) = &step {
            if *name == branch_name {
                inside_branch = true;
            } else if inside_branch {
                inside_branch = false;
            }
        }

        if !inside_branch {
            // Not inside the source branch, keep the step as is
            new_rebase_steps.push(step);
            continue;
        }

        match &step {
            RebaseStep::Pick { .. } => {
                continue;
            }
            RebaseStep::SquashIntoPreceding { .. } => {
                continue;
            }
            RebaseStep::Reference(_) => {
                new_rebase_steps.push(step);
                let rebase_steps = integration_steps_to_rebase_steps(&steps)?;
                new_rebase_steps.extend(rebase_steps);
                continue;
            }
        }
    }

    new_rebase_steps.reverse();

    let mut rebase = Rebase::new(&repository, merge_base, None)?;
    rebase.steps(new_rebase_steps)?;
    rebase.rebase_noops(false);
    let result = rebase.rebase()?;
    let head = result.top_commit.to_git2();

    source_stack.set_stack_head(&vb_state, &repository, head, None)?;
    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    update_uncommited_changes(ctx, old_workspace, new_workspace, perm)?;
    source_stack.set_heads_from_rebase_output(ctx, result.references)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)?;

    Ok(())
}

/// Turn the integration steps into rebase steps.
fn integration_steps_to_rebase_steps(
    steps: &[InteractiveIntegrationStep],
) -> Result<Vec<RebaseStep>> {
    let mut rebase_steps = vec![];
    for step in steps {
        match step {
            InteractiveIntegrationStep::Pick { commit_id, .. } => {
                rebase_steps.push(RebaseStep::Pick {
                    commit_id: commit_id.to_owned(),
                    new_message: None,
                });
            }
            InteractiveIntegrationStep::Skip { .. } => {
                // Skip steps are simply not added to the rebase steps
            }
            InteractiveIntegrationStep::Squash {
                commits, message, ..
            } => {
                if commits.len() < 2 {
                    return Err(anyhow::anyhow!(
                        "Squash step must have at least two commits"
                    ));
                }

                if let Some((last_commit, all_but_last)) = commits.split_last() {
                    for commit_a in all_but_last {
                        rebase_steps.push(RebaseStep::SquashIntoPreceding {
                            commit_id: commit_a.to_owned(),
                            new_message: message.to_owned().map(Into::into),
                        });
                    }
                    rebase_steps.push(RebaseStep::Pick {
                        commit_id: last_commit.to_owned(),
                        new_message: None,
                    });
                }
            }
        }
    }

    Ok(rebase_steps)
}

pub fn integrate_upstream_commits_for_series(
    ctx: &CommandContext,
    stack_id: StackId,
    perm: &mut WorktreeWritePermission,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    let strategy = integration_strategy.unwrap_or(IntegrationStrategy::Rebase);
    match strategy {
        IntegrationStrategy::Merge => {
            // TODO: Have a nice way of doing merge integration
            bail!("Merge strategy is not supported yet. Please use Rebase strategy.");
        }
        IntegrationStrategy::Rebase => {
            let steps = get_initial_integration_steps_for_branch(
                ctx,
                Some(stack_id),
                series_name.to_owned(),
            )?;
            integrate_branch_with_steps(ctx, stack_id, series_name, steps, perm)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IntegrationStrategy {
    Merge,
    Rebase,
    // TODO: HardReset,
}
