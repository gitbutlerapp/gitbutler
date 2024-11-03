use anyhow::{bail, Context, Result};
use gitbutler_branch::{BranchCreateRequest, BranchIdentity, BranchUpdateRequest};
use gitbutler_branch_actions::{get_branch_listing_details, list_branches, BranchManagerExt};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_stack::{Stack, VirtualBranchesHandle};

use crate::command::debug_print;

pub fn list_all(project: Project) -> Result<()> {
    let ctx = CommandContext::open(&project)?;
    debug_print(list_branches(&ctx, None, None)?)
}

pub fn list_local(project: Project) -> Result<()> {
    debug_print(gitbutler_branch_actions::list_local_branches(project)?)
}

pub fn details(project: Project, branch_names: Vec<BranchIdentity>) -> Result<()> {
    let ctx = CommandContext::open(&project)?;
    debug_print(get_branch_listing_details(&ctx, branch_names)?)
}

pub fn list(project: Project) -> Result<()> {
    let branches = VirtualBranchesHandle::new(project.gb_dir()).list_all_branches()?;
    for vbranch in branches {
        println!(
            "{active} {id} {name} {upstream} {default}",
            active = if vbranch.in_workspace {
                "✔️"
            } else {
                "⛌"
            },
            id = vbranch.id,
            name = vbranch.name,
            upstream = vbranch
                .upstream
                .map_or_else(Default::default, |b| b.to_string()),
            default = if vbranch.in_workspace { "🌟" } else { "" }
        );
    }
    Ok(())
}

pub fn status(project: Project) -> Result<()> {
    debug_print(gitbutler_branch_actions::list_virtual_branches(&project)?)
}

pub fn unapply(project: Project, branch_name: String) -> Result<()> {
    let branch = branch_by_name(&project, &branch_name)?;
    debug_print(gitbutler_branch_actions::save_and_unapply_virutal_branch(
        &project, branch.id,
    )?)
}

pub fn apply(project: Project, branch_name: String) -> Result<()> {
    let branch = branch_by_name(&project, &branch_name)?;
    let ctx = CommandContext::open(&project)?;
    let mut guard = project.exclusive_worktree_access();
    debug_print(
        ctx.branch_manager().create_virtual_branch_from_branch(
            branch
                .source_refname
                .as_ref()
                .context("local reference name was missing")?,
            None,
            None,
            guard.write_permission(),
        )?,
    )
}

pub fn create(project: Project, branch_name: String, set_default: bool) -> Result<()> {
    let new = gitbutler_branch_actions::create_virtual_branch(
        &project,
        &BranchCreateRequest {
            name: Some(branch_name),
            ..Default::default()
        },
    )?;
    if set_default {
        let new = VirtualBranchesHandle::new(project.gb_dir()).get_branch(new)?;
        set_default_branch(&project, &new)?;
    }
    debug_print(new)
}

pub fn set_default(project: Project, branch_name: String) -> Result<()> {
    let branch = branch_by_name(&project, &branch_name)?;
    set_default_branch(&project, &branch)
}

fn set_default_branch(project: &Project, branch: &Stack) -> Result<()> {
    gitbutler_branch_actions::update_virtual_branch(
        project,
        BranchUpdateRequest {
            id: branch.id,
            name: None,
            notes: None,
            ownership: None,
            order: None,
            upstream: None,
            selected_for_changes: Some(true),
            allow_rebasing: None,
        },
    )
}

pub fn series(project: Project, stack_name: String, new_series_name: String) -> Result<()> {
    let mut stack = branch_by_name(&project, &stack_name)?;
    let ctx = CommandContext::open(&project)?;
    stack.add_series_top_of_stack(&ctx, new_series_name, None)?;
    Ok(())
}

pub fn commit(project: Project, branch_name: String, message: String) -> Result<()> {
    let branch = branch_by_name(&project, &branch_name)?;
    let (info, skipped) = gitbutler_branch_actions::list_virtual_branches(&project)?;

    if !skipped.is_empty() {
        eprintln!(
            "{} files could not be processed (binary or large size)",
            skipped.len()
        )
    }

    let populated_branch = info
        .iter()
        .find(|b| b.id == branch.id)
        .expect("A populated branch exists for a branch we can list");
    if populated_branch.ownership.claims.is_empty() {
        bail!(
            "Branch '{branch_name}' has no change to commit{hint}",
            hint = {
                let candidate_names = info
                    .iter()
                    .filter_map(|b| (!b.ownership.claims.is_empty()).then_some(b.name.as_str()))
                    .collect::<Vec<_>>();
                let mut candidates = candidate_names.join(", ");
                if !candidate_names.is_empty() {
                    candidates = format!(
                        ". {candidates} {have} changes.",
                        have = if candidate_names.len() == 1 {
                            "has"
                        } else {
                            "have"
                        }
                    )
                };
                candidates
            }
        )
    }

    let run_hooks = false;
    debug_print(gitbutler_branch_actions::create_commit(
        &project,
        branch.id,
        &message,
        Some(&populated_branch.ownership),
        run_hooks,
    )?)
}

pub fn branch_by_name(project: &Project, name: &str) -> Result<Stack> {
    let mut found: Vec<_> = VirtualBranchesHandle::new(project.gb_dir())
        .list_all_branches()?
        .into_iter()
        .filter(|b| b.name == name)
        .collect();
    if found.is_empty() {
        bail!("No virtual branch named '{name}'");
    } else if found.len() > 1 {
        bail!("Found more than one virtual branch named '{name}'");
    }
    Ok(found.pop().expect("present"))
}
