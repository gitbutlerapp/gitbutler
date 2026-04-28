use anyhow::{Context as _, bail};
use but_core::DryRun;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use std::time::{Duration, Instant};

const PROJECT_TITLE: &str = "gitbutler-testing";
const BRANCH_NAME: &str = "e-branch-6";

enum Mode {
    Single,
    Benchmark { iterations: usize },
}

fn main() -> anyhow::Result<()> {
    let mode = parse_mode(std::env::args().skip(1))?;
    match mode {
        Mode::Single => run_single(),
        Mode::Benchmark { iterations } => run_benchmark(iterations),
    }
}

fn parse_mode(mut args: impl Iterator<Item = String>) -> anyhow::Result<Mode> {
    let Some(first) = args.next() else {
        return Ok(Mode::Single);
    };

    if first != "benchmark" {
        bail!(
            "Unknown command '{first}'. Use no args for single run, or 'benchmark --iterations <N>'"
        );
    }

    let mut iterations = 20usize;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--iterations" | "-n" => {
                let Some(value) = args.next() else {
                    bail!("Missing value for {arg}")
                };
                iterations = value
                    .parse::<usize>()
                    .with_context(|| format!("Invalid iterations value '{value}'"))?;
            }
            _ => bail!("Unknown benchmark argument '{arg}'"),
        }
    }

    if iterations == 0 {
        bail!("iterations must be greater than 0")
    }

    Ok(Mode::Benchmark { iterations })
}

fn open_context() -> anyhow::Result<but_ctx::Context> {
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
    let ctx = but_ctx::Context::open(project.git_dir())?;
    println!(
        "[timing] context open elapsed={:?}",
        open_ctx_started.elapsed()
    );
    Ok(ctx)
}

fn run_single() -> anyhow::Result<()> {
    let total_started = Instant::now();
    let mut ctx = open_context()?;

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

    Ok(())
}

fn run_benchmark(iterations: usize) -> anyhow::Result<()> {
    let total_started = Instant::now();
    let mut ctx = open_context()?;

    println!("[2/3] Benchmark setup...");
    let branch_ref: gix::refs::FullName = format!("refs/heads/{BRANCH_NAME}").try_into()?;
    let use_local_only = ctx.settings.feature_flags.mutation_workspace_local_only;
    println!(
        "Benchmarking with iterations={iterations} mutation_workspace_local_only={use_local_only}"
    );

    println!("[3/3] Running iterations...");
    let mut context_prepared = Vec::with_capacity(iterations);
    let mut commit_move = Vec::with_capacity(iterations);

    for idx in 0..iterations {
        let branch_details =
            but_api::legacy::workspace::branch_details(&ctx, BRANCH_NAME.to_owned(), None)?;
        if branch_details.commits.len() < 2 {
            bail!("Branch '{BRANCH_NAME}' must have at least 2 commits to benchmark move")
        }
        let subject = branch_details.commits[1].id;

        let context_prepared_elapsed = measure_context_prepare_elapsed(&mut ctx)?;
        context_prepared.push(context_prepared_elapsed);

        // Keep move timing representative by forcing context preparation from a cold workspace cache.
        ctx.invalidate_workspace_cache()?;
        let move_started = Instant::now();
        let _ = but_api::commit::move_commit::commit_move(
            &mut ctx,
            vec![subject],
            RelativeTo::Reference(branch_ref.clone()),
            InsertSide::Below,
            DryRun::No,
        )?;
        let commit_move_elapsed = move_started.elapsed();
        commit_move.push(commit_move_elapsed);

        println!(
            "[iter {}/{}] context_prepared={:?} commit_move={:?}",
            idx + 1,
            iterations,
            context_prepared_elapsed,
            commit_move_elapsed
        );
    }

    print_summary("context_prepared", &context_prepared);
    print_summary("commit_move", &commit_move);
    println!(
        "[timing] benchmark total elapsed={:?}",
        total_started.elapsed()
    );
    Ok(())
}

fn measure_context_prepare_elapsed(ctx: &mut but_ctx::Context) -> anyhow::Result<Duration> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.invalidate_workspace_cache()?;
    let started = Instant::now();
    if ctx.settings.feature_flags.mutation_workspace_local_only {
        let _ = ctx.workspace_mut_with_perm_mutation_local_only(guard.write_permission())?;
    } else {
        let _ = ctx.workspace_mut_with_perm(guard.write_permission())?;
    }
    Ok(started.elapsed())
}

fn print_summary(label: &str, samples: &[Duration]) {
    let p50 = percentile(samples, 0.50);
    let p95 = percentile(samples, 0.95);
    let mean_ms = samples
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / samples.len() as f64;
    println!(
        "[summary] {label}: n={} p50={:.3}ms p95={:.3}ms mean={:.3}ms",
        samples.len(),
        p50,
        p95,
        mean_ms
    );
}

fn percentile(samples: &[Duration], ratio: f64) -> f64 {
    let mut values: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    values.sort_by(f64::total_cmp);
    let rank = ((values.len() as f64 * ratio).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[rank]
}
