use anyhow::{bail, Context, Result};
use gitbutler_branch::{BranchCreateRequest, BranchIdentity, BranchUpdateRequest};
use gitbutler_branch_actions::{get_branch_listing_details, list_branches, BranchManagerExt};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{Stack, VirtualBranchesHandle};

use crate::command::debug_print;

pub fn set_base(project: Project, short_tracking_branch_name: String) -> Result<()> {
    let branch_name = format!("refs/remotes/{}", short_tracking_branch_name)
        .parse()
        .context("Invalid branch name")?;
    debug_print(gitbutler_branch_actions::set_base_branch(
        &project,
        &branch_name,
    )?)
}

pub fn list_all(project: Project) -> Result<()> {
    let ctx = CommandContext::open(&project)?;
    debug_print(list_branches(&ctx, None, None)?)
}

pub fn details(project: Project, branch_names: Vec<BranchIdentity>) -> Result<()> {
    let ctx = CommandContext::open(&project)?;
    debug_print(get_branch_listing_details(&ctx, branch_names)?)
}

pub fn list(project: Project) -> Result<()> {
    let stacks = VirtualBranchesHandle::new(project.gb_dir()).list_all_stacks()?;
    for stack in stacks {
        println!(
            "{active} {id} {name} {upstream} {default}",
            active = if stack.in_workspace { "✔️" } else { "⛌" },
            id = stack.id,
            name = stack.name,
            upstream = stack
                .upstream
                .map_or_else(Default::default, |b| b.to_string()),
            default = if stack.in_workspace { "🌟" } else { "" }
        );
    }
    Ok(())
}

pub fn status(project: Project) -> Result<()> {
    debug_print(gitbutler_branch_actions::list_virtual_branches(&project)?)
}

pub fn unapply(project: Project, branch_name: String) -> Result<()> {
    let stack = stack_by_name(&project, &branch_name)?;
    debug_print(gitbutler_branch_actions::save_and_unapply_virutal_branch(
        &project, stack.id,
    )?)
}

pub fn apply(project: Project, branch_name: String, from_branch: bool) -> Result<()> {
    if from_branch {
        apply_from_branch(project, branch_name)
    } else {
        apply_by_name(project, branch_name)
    }
}

fn apply_by_name(project: Project, branch_name: String) -> Result<()> {
    let stack = stack_by_name(&project, &branch_name)?;
    let ctx = CommandContext::open(&project)?;
    let mut guard = project.exclusive_worktree_access();
    debug_print(
        ctx.branch_manager().create_virtual_branch_from_branch(
            stack
                .source_refname
                .as_ref()
                .context("local reference name was missing")?,
            None,
            None,
            guard.write_permission(),
        )?,
    )
}

fn apply_from_branch(project: Project, branch_name: String) -> Result<()> {
    let refname = Refname::Local(LocalRefname::new(&branch_name, None));
    let target = if let Some(stack) = stack_by_refname(&project, &refname)? {
        stack
            .source_refname
            .context("local reference name was missing")?
    } else {
        refname
    };

    let ctx = CommandContext::open(&project)?;

    let mut guard = project.exclusive_worktree_access();
    debug_print(ctx.branch_manager().create_virtual_branch_from_branch(
        &target,
        None,
        None,
        guard.write_permission(),
    )?)
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
        let new = VirtualBranchesHandle::new(project.gb_dir()).get_stack(new)?;
        set_default_branch(&project, &new)?;
    }
    debug_print(new)
}

pub fn set_default(project: Project, branch_name: String) -> Result<()> {
    let stack = stack_by_name(&project, &branch_name)?;
    set_default_branch(&project, &stack)
}

fn set_default_branch(project: &Project, stack: &Stack) -> Result<()> {
    gitbutler_branch_actions::update_virtual_branch(
        project,
        BranchUpdateRequest {
            id: stack.id,
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
    let mut stack = stack_by_name(&project, &stack_name)?;
    let ctx = CommandContext::open(&project)?;
    stack.add_series_top_of_stack(&ctx, new_series_name, None)?;
    Ok(())
}

pub fn commit(project: Project, branch_name: String, message: String) -> Result<()> {
    let stack = stack_by_name(&project, &branch_name)?;
    let list_result = gitbutler_branch_actions::list_virtual_branches(&project)?;

    if !list_result.skipped_files.is_empty() {
        eprintln!(
            "{} files could not be processed (binary or large size)",
            list_result.skipped_files.len()
        )
    }

    let target_branch = list_result
        .branches
        .iter()
        .find(|b| b.id == stack.id)
        .expect("A populated branch exists for a branch we can list");
    if target_branch.ownership.claims.is_empty() {
        bail!(
            "Branch '{branch_name}' has no change to commit{hint}",
            hint = {
                let candidate_names = list_result
                    .branches
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

    debug_print(gitbutler_branch_actions::create_commit(
        &project,
        stack.id,
        &message,
        Some(&target_branch.ownership),
    )?)
}

fn stack_by_name(project: &Project, name: &str) -> Result<Stack> {
    let mut found = find_all_stacks_by_name(project, name)?;
    if found.is_empty() {
        bail!("No stack named '{name}'");
    } else if found.len() > 1 {
        bail!("Found more than one stack named '{name}'");
    }
    Ok(found.pop().expect("present"))
}

fn stack_by_refname(project: &Project, refname: &Refname) -> Result<Option<Stack>> {
    let mut found = find_all_stacks_by_refname(project, refname)?;
    if found.is_empty() {
        return Ok(None);
    } else if found.len() > 1 {
        bail!("Found more than one stack with refname '{refname}'");
    }
    Ok(Some(found.pop().expect("present")))
}

fn find_all_stacks_by_name(project: &Project, name: &str) -> Result<Vec<Stack>> {
    let found = VirtualBranchesHandle::new(project.gb_dir())
        .list_all_stacks()?
        .into_iter()
        .filter(|b| b.name == name)
        .collect();
    Ok(found)
}

fn find_all_stacks_by_refname(project: &Project, refname: &Refname) -> Result<Vec<Stack>> {
    let found = VirtualBranchesHandle::new(project.gb_dir())
        .list_all_stacks()?
        .into_iter()
        .filter(|b| b.source_refname.as_ref() == Some(refname))
        .collect();
    Ok(found)
}
