use anyhow::{bail, Context, Result};
use but_graph::VirtualBranchesTomlMetadata;
use but_settings::AppSettings;
use but_workspace::{ui::StackDetails, StackId, StacksFilter};
use gitbutler_branch::{BranchCreateRequest, BranchIdentity, BranchUpdateRequest};
use gitbutler_branch_actions::{get_branch_listing_details, list_branches, BranchManagerExt};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::Project;
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{Stack, VirtualBranchesHandle};

use crate::command::debug_print;

pub fn list_commit_files(project: Project, commit_id_hex: String) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    let commit_id = gix::ObjectId::from_hex(commit_id_hex.as_bytes())?;
    debug_print(gitbutler_branch_actions::list_commit_files(
        &ctx,
        commit_id.to_git2(),
    )?)
}

pub fn set_base(project: Project, short_tracking_branch_name: String) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    let branch_name = format!("refs/remotes/{}", short_tracking_branch_name)
        .parse()
        .context("Invalid branch name")?;
    debug_print(gitbutler_branch_actions::set_base_branch(
        &ctx,
        &branch_name,
        false,
        ctx.project().exclusive_worktree_access().write_permission(),
    )?)
}

pub fn list_all(project: Project) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    debug_print(list_branches(&ctx, None, None)?)
}

pub fn details(project: Project, branch_names: Vec<BranchIdentity>) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
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
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    debug_print(stacks(&ctx))
}

pub(crate) fn stacks(ctx: &CommandContext) -> Result<Vec<(StackId, StackDetails)>> {
    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    let stacks = if ctx.app_settings().feature_flags.ws3 {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stacks_v3(&repo, &meta, StacksFilter::default(), None)
    } else {
        but_workspace::stacks(ctx, &ctx.project().gb_dir(), &repo, StacksFilter::default())
    }?;
    let mut details = vec![];
    for stack in stacks {
        let stack_id = stack
            .id
            .context("BUG(opt-stack-id): CLI code shouldn't trigger this")?;
        details.push((
            stack_id,
            if ctx.app_settings().feature_flags.ws3 {
                let meta = VirtualBranchesTomlMetadata::from_path(
                    ctx.project().gb_dir().join("virtual_branches.toml"),
                )?;
                but_workspace::stack_details_v3(stack_id.into(), &repo, &meta)
            } else {
                but_workspace::stack_details(&ctx.project().gb_dir(), stack_id, ctx)
            }?,
        ));
    }
    Ok(details)
}

pub fn unapply(project: Project, branch_name: String) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    let stack = stack_by_name(&project, &branch_name)?;
    debug_print(gitbutler_branch_actions::unapply_stack(
        &ctx,
        stack.id,
        Vec::new(),
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
    let ctx = CommandContext::open(&project, AppSettings::default())?;
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

    let ctx = CommandContext::open(&project, AppSettings::default())?;

    let mut guard = project.exclusive_worktree_access();
    debug_print(ctx.branch_manager().create_virtual_branch_from_branch(
        &target,
        None,
        None,
        guard.write_permission(),
    )?)
}

pub fn create(project: Project, branch_name: String, set_default: bool) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    let new_stack_entry = gitbutler_branch_actions::create_virtual_branch(
        &ctx,
        &BranchCreateRequest {
            name: Some(branch_name),
            ..Default::default()
        },
        ctx.project().exclusive_worktree_access().write_permission(),
    )?;
    if set_default {
        let new = VirtualBranchesHandle::new(project.gb_dir()).get_stack(new_stack_entry.id)?;
        set_default_branch(&project, &new)?;
    }
    debug_print(new_stack_entry)
}

pub fn set_default(project: Project, branch_name: String) -> Result<()> {
    let stack = stack_by_name(&project, &branch_name)?;
    set_default_branch(&project, &stack)
}

fn set_default_branch(project: &Project, stack: &Stack) -> Result<()> {
    let ctx = CommandContext::open(project, AppSettings::default())?;
    gitbutler_branch_actions::update_virtual_branch(
        &ctx,
        BranchUpdateRequest {
            id: Some(stack.id),
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
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    stack.add_series_top_of_stack(&ctx, new_series_name, None)?;
    Ok(())
}

pub fn commit(project: Project, branch_name: String, message: String) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    let stack = stack_by_name(&project, &branch_name)?;
    let (_, d) = stacks(&ctx)?
        .into_iter()
        .find(|(i, _)| *i == stack.id)
        .unwrap();

    let repo = ctx.gix_repo()?;
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let file_changes: Vec<but_workspace::DiffSpec> =
        worktree.changes.iter().map(Into::into).collect::<Vec<_>>();

    let outcome = but_workspace::commit_engine::create_commit_simple(
        &ctx,
        stack.id,
        None,
        file_changes,
        message,
        d.derived_name,
        ctx.project().exclusive_worktree_access().write_permission(),
    )?;
    debug_print(outcome)
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
