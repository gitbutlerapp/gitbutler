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
    let ctx = but_ctx::Context::from_repo_for_testing(gix::open(repo_path)?)?;

    for _ in 0..3 {
        black_box(but_api::workspace_revision::compute(&ctx)?);
        black_box(but_api::legacy::workspace::head_info_data(&ctx)?);
    }

    let revision = measure(samples, || but_api::workspace_revision::compute(&ctx))?;
    let head_info = measure(samples, || but_api::legacy::workspace::head_info_data(&ctx))?;
    println!("samples: {samples}");
    println!("WorkspaceRevision median: {:.3} ms", revision / 1_000_000.0);
    println!(
        "head_info median:         {:.3} ms",
        head_info / 1_000_000.0
    );
    println!("head_info / revision:    {:.2}x", head_info / revision);
    Ok(())
}

fn measure<T>(
    samples: usize,
    mut operation: impl FnMut() -> anyhow::Result<T>,
) -> anyhow::Result<f64> {
    anyhow::ensure!(samples > 0, "samples must be greater than zero");
    let mut elapsed = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        black_box(operation()?);
        elapsed.push(start.elapsed().as_nanos());
    }
    elapsed.sort_unstable();
    Ok(elapsed[elapsed.len() / 2] as f64)
}
