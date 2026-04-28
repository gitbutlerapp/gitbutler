use anyhow::{Context as _, bail};
use but_core::DryRun;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use std::time::Instant;

const PROJECT_TITLE: &str = "gitbutler-testing";
const BRANCH_NAME: &str = "e-branch-6";
fn main() -> anyhow::Result<()> {
    let total_started = Instant::now();

    println!("[1/3] Listing projects...");
    let projects_started = Instant::now();
    let projects = gitbutler_project::dangerously_list_projects_without_migration()?;
    for project in &projects {
        println!("- id={} title={}", project.id, project.title);
    }

    let project = projects
        .into_iter()
        .find(|project| project.title == PROJECT_TITLE)
        .with_context(|| format!("Project '{PROJECT_TITLE}' was not found"))?
        .migrated()?;
    println!(
        "[timing] project listing+selection elapsed={:?}",
        projects_started.elapsed()
    );

    println!(
        "Selected project: id={} title={} git_dir={}",
        project.id,
        project.title,
        project.git_dir().display()
    );

    let open_ctx_started = Instant::now();
    let mut ctx = but_ctx::Context::open(project.git_dir())?;
    println!(
        "[timing] context open elapsed={:?}",
        open_ctx_started.elapsed()
    );

    println!("[warmup] Comparing mutation workspace accessors (cold -> warm)...");

    {
        let mut guard = ctx.exclusive_worktree_access();
        ctx.invalidate_workspace_cache()?;
        let started = Instant::now();
        let _ = ctx.workspace_mut_with_perm(guard.write_permission())?;
        println!(
            "[timing] workspace_mut_with_perm (normal) cold elapsed={:?}",
            started.elapsed()
        );
    }
    {
        let mut guard = ctx.exclusive_worktree_access();
        let started = Instant::now();
        let _ = ctx.workspace_mut_with_perm(guard.write_permission())?;
        println!(
            "[timing] workspace_mut_with_perm (normal) warm elapsed={:?}",
            started.elapsed()
        );
    }

    {
        let mut guard = ctx.exclusive_worktree_access();
        ctx.invalidate_workspace_cache()?;
        let started = Instant::now();
        let _ = ctx.workspace_mut_with_perm_mutation_local_only(guard.write_permission())?;
        println!(
            "[timing] workspace_mut_with_perm_mutation_local_only cold elapsed={:?}",
            started.elapsed()
        );
    }
    {
        let mut guard = ctx.exclusive_worktree_access();
        let started = Instant::now();
        let _ = ctx.workspace_mut_with_perm_mutation_local_only(guard.write_permission())?;
        println!(
            "[timing] workspace_mut_with_perm_mutation_local_only warm elapsed={:?}",
            started.elapsed()
        );
    }

    println!("[2/3] Reading head info and branch details...");
    let head_info_started = Instant::now();
    let head_info = but_api::legacy::workspace::head_info(&ctx)?;
    println!(
        "[timing] head_info elapsed={:?}",
        head_info_started.elapsed()
    );
    println!(
        "Head info: stacks={} managed_ref={} managed_commit={} entrypoint={}",
        head_info.stacks.len(),
        head_info.is_managed_ref,
        head_info.is_managed_commit,
        head_info.is_entrypoint
    );

    let branch_details_started = Instant::now();
    let branch_details =
        but_api::legacy::workspace::branch_details(&ctx, BRANCH_NAME.to_owned(), None)?;
    println!(
        "[timing] initial branch_details elapsed={:?}",
        branch_details_started.elapsed()
    );
    println!(
        "Branch '{}' has {} commits:",
        BRANCH_NAME,
        branch_details.commits.len()
    );
    for (idx, commit) in branch_details.commits.iter().enumerate() {
        println!("  [{idx}] {}", commit.id);
    }

    if branch_details.commits.len() < 2 {
        bail!("Branch '{BRANCH_NAME}' must have at least 2 commits to move index 1 to the top");
    }

    let old_top = branch_details.commits[0].id;
    let subject = branch_details.commits[1].id;

    println!("[3/3] Moving commit index 1 to top...");
    println!("Old top={old_top} subject={subject}");

    let branch_ref: gix::refs::FullName = format!("refs/heads/{BRANCH_NAME}").try_into()?;

    let move_started = Instant::now();
    let _ = but_api::commit::move_commit::commit_move(
        &mut ctx,
        vec![subject],
        RelativeTo::Reference(branch_ref),
        InsertSide::Below,
        DryRun::No,
    )?;
    println!("[timing] commit_move elapsed={:?}", move_started.elapsed());

    let refresh_started = Instant::now();
    let updated = but_api::legacy::workspace::branch_details(&ctx, BRANCH_NAME.to_owned(), None)?;
    println!(
        "[timing] post-move branch_details elapsed={:?}",
        refresh_started.elapsed()
    );
    let new_top = updated
        .commits
        .first()
        .map(|commit| commit.id)
        .context("Branch has no commits after move")?;

    println!("Move complete: old_top={old_top} new_top={new_top} moved={subject}");
    println!("[timing] total elapsed={:?}", total_started.elapsed());

    std::process::exit(0);
}
