use std::{env, hint::black_box, path::PathBuf, time::Instant};

fn main() -> anyhow::Result<()> {
    let repo_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(env::current_dir()?);
    let samples = env::args()
        .nth(2)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(20);
    let repo = gix::open(repo_path)?;
    let ref_count = repo.references()?.all()?.count();
    let remote_count = repo.remote_names().len();
    let linked_worktree_count = repo.worktrees()?.len();
    let shallow_boundary_count = repo.shallow_commits()?.map_or(0, |commits| commits.len());
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?;
    let forge_association_count = {
        let db = ctx.db.get_cache()?;
        but_forge::review_associations_by_head(&db)?.len()
    };

    for _ in 0..3 {
        black_box(but_api::workspace_revision::compute(&ctx)?);
        black_box(reload_and_compute(&mut ctx)?);
        black_box(but_api::legacy::workspace::head_info(&ctx)?);
        black_box(but_api::legacy::workspace::head_info_snapshot(&ctx)?);
    }

    let revision = measure(samples, || but_api::workspace_revision::compute(&ctx))?;
    let reload_revision = measure(samples, || reload_and_compute(&mut ctx))?;
    let head_info = measure(samples, || but_api::legacy::workspace::head_info(&ctx))?;
    let snapshot = measure(samples, || {
        but_api::legacy::workspace::head_info_snapshot(&ctx)
    })?;
    println!("samples: {samples}");
    println!(
        "repository: {ref_count} refs, {remote_count} remotes, {linked_worktree_count} linked worktrees, {shallow_boundary_count} shallow boundaries, {forge_association_count} forge associations"
    );
    print_stats("WorkspaceRevision", revision);
    print_stats("reload + revision", reload_revision);
    print_stats("head_info", head_info);
    print_stats("snapshot endpoint", snapshot);
    println!(
        "reload overhead:          {:.2}x",
        reload_revision.p50 / revision.p50
    );
    println!(
        "head_info / revision:     {:.2}x",
        head_info.p50 / revision.p50
    );
    println!(
        "snapshot / head_info:     {:.2}x",
        snapshot.p50 / head_info.p50
    );
    Ok(())
}

fn reload_and_compute(ctx: &mut but_ctx::Context) -> anyhow::Result<String> {
    ctx.repo.get_mut()?.reload()?;
    but_api::workspace_revision::compute(ctx)
}

#[derive(Clone, Copy)]
struct Stats {
    p50: f64,
    p95: f64,
    p99: f64,
}

fn measure<T>(
    samples: usize,
    mut operation: impl FnMut() -> anyhow::Result<T>,
) -> anyhow::Result<Stats> {
    anyhow::ensure!(samples > 0, "samples must be greater than zero");
    let mut elapsed = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        black_box(operation()?);
        elapsed.push(start.elapsed().as_nanos());
    }
    elapsed.sort_unstable();
    let percentile = |p: usize| elapsed[(elapsed.len() - 1) * p / 100] as f64;
    Ok(Stats {
        p50: percentile(50),
        p95: percentile(95),
        p99: percentile(99),
    })
}

fn print_stats(name: &str, stats: Stats) {
    println!(
        "{name:<22} p50 {:>8.3} ms  p95 {:>8.3} ms  p99 {:>8.3} ms",
        stats.p50 / 1_000_000.0,
        stats.p95 / 1_000_000.0,
        stats.p99 / 1_000_000.0
    );
}
