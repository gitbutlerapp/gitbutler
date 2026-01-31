use std::path::Path;

use anyhow::{Context as _, Result, bail};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_workspace::{legacy::StacksFilter, ui::StackDetails};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::BranchManagerExt;
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{Stack, VirtualBranchesHandle};

use crate::command::debug_print;

pub fn list(ctx: &Context) -> Result<()> {
    let stacks = VirtualBranchesHandle::new(ctx.project_data_dir()).list_all_stacks()?;
    for stack in stacks {
        println!(
            "{active} {id} {name} {upstream} {default}",
            active = if stack.in_workspace { "✔️" } else { "⛌" },
            id = stack.id,
            name = stack.name(),
            upstream = stack
                .upstream
                .map_or_else(Default::default, |b| b.to_string()),
            default = if stack.in_workspace { "🌟" } else { "" }
        );
    }
    Ok(())
}

pub(crate) fn stacks(ctx: &Context) -> Result<Vec<(StackId, StackDetails)>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let stacks = {
        let meta = ctx.legacy_meta()?;
        but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
    }?;
    let mut details = vec![];
    for stack in stacks {
        let stack_id = stack
            .id
            .context("BUG(opt-stack-id): CLI code shouldn't trigger this")?;
        details.push((stack_id, {
            let meta = ctx.legacy_meta()?;
            but_workspace::legacy::stack_details_v3(stack_id.into(), &repo, &meta)
        }?));
    }
    Ok(details)
}

pub fn apply(ctx: &mut Context, branch_name: String, from_branch: bool) -> Result<()> {
    if from_branch {
        apply_from_branch(ctx, branch_name)
    } else {
        apply_by_name(ctx, branch_name)
    }
}

fn apply_by_name(ctx: &mut Context, branch_name: String) -> Result<()> {
    let stack = stack_by_name(&ctx.project_data_dir(), &branch_name)?;
    let mut guard = ctx.exclusive_worktree_access();
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
fn apply_from_branch(ctx: &mut Context, branch_name: String) -> Result<()> {
    let refname = Refname::Local(LocalRefname::new(&branch_name, None));
    let target = if let Some(stack) = stack_by_refname(&ctx.project_data_dir(), &refname)? {
        stack
            .source_refname
            .context("local reference name was missing")?
    } else {
        refname
    };

    let mut guard = ctx.exclusive_worktree_access();
    debug_print(ctx.branch_manager().create_virtual_branch_from_branch(
        &target,
        None,
        None,
        guard.write_permission(),
    )?)
}

pub fn create(ctx: &mut Context, branch_name: String) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let new_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some(branch_name),
            ..Default::default()
        },
        guard.write_permission(),
    )?;
    debug_print(new_stack_entry)
}

pub fn series(ctx: &Context, stack_name: String, new_series_name: String) -> Result<()> {
    let mut stack = stack_by_name(&ctx.project_data_dir(), &stack_name)?;
    stack.add_series_top_of_stack(ctx, new_series_name)?;
    Ok(())
}

pub fn commit(ctx: &mut Context, branch_name: String, message: String) -> Result<()> {
    let stack = stack_by_name(&ctx.project_data_dir(), &branch_name)?;
    let (_, d) = stacks(ctx)?
        .into_iter()
        .find(|(i, _)| *i == stack.id)
        .unwrap();

    let worktree = but_core::diff::worktree_changes(&*ctx.repo.get()?)?;
    let mut guard = ctx.exclusive_worktree_access();
    let file_changes: Vec<but_core::DiffSpec> =
        worktree.changes.iter().map(Into::into).collect::<Vec<_>>();

    let outcome = but_workspace::legacy::commit_engine::create_commit_simple(
        ctx,
        stack.id,
        None,
        file_changes,
        message,
        d.derived_name,
        guard.write_permission(),
    )?;
    debug_print(outcome)
}

fn stack_by_name(project_data_dir: &Path, name: &str) -> Result<Stack> {
    let mut found = find_all_stacks_by_name(project_data_dir, name)?;
    if found.is_empty() {
        bail!("No stack named '{name}'");
    } else if found.len() > 1 {
        bail!("Found more than one stack named '{name}'");
    }
    Ok(found.pop().expect("present"))
}

fn stack_by_refname(project_data_dir: &Path, refname: &Refname) -> Result<Option<Stack>> {
    let mut found = find_all_stacks_by_refname(project_data_dir, refname)?;
    if found.is_empty() {
        return Ok(None);
    } else if found.len() > 1 {
        bail!("Found more than one stack with refname '{refname}'");
    }
    Ok(Some(found.pop().expect("present")))
}

fn find_all_stacks_by_name(project_data_dir: &Path, name: &str) -> Result<Vec<Stack>> {
    let found = VirtualBranchesHandle::new(project_data_dir)
        .list_all_stacks()?
        .into_iter()
        .filter(|b| b.name() == name)
        .collect();
    Ok(found)
}

fn find_all_stacks_by_refname(project_data_dir: &Path, refname: &Refname) -> Result<Vec<Stack>> {
    let found = VirtualBranchesHandle::new(project_data_dir)
        .list_all_stacks()?
        .into_iter()
        .filter(|b| b.source_refname.as_ref() == Some(refname))
        .collect();
    Ok(found)
}
