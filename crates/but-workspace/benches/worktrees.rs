//! # Worktree benchmark
//!
//! Build with `cargo bench -p but-workspace --features worktree-cow --bench worktrees --no-run`
//! for the workspace's gix feature set. Add `gix/max-performance` to the feature list
//! to match the gix CLI/desktop performance bundle. Record which variant is measured;
//! parallelism alone does not enable default pack caching.
//! Run the resulting executable directly; Cargo's benchmark argument is not needed.
//!
//! Set `WORKTREE_BENCH_REPO` to the source repository to benchmark and
//! `WORKTREE_BENCH_TMPDIR` to a dedicated case-sensitive APFS image mount, to avoid checkout collisions
//! like those with the Linux kernel.
//! Both are required and must be nonempty; selection fails before scratch setup if either is missing.
//! There is no default source repository.
//! `WORKTREE_BENCH_RUNS=10`, `WORKTREE_BENCH_WARMUPS=2`, and
//! `WORKTREE_BENCH_LABEL=production` control repetitions and output labels.
//! The source repository is read only. A clean seed checkout borrows its object database.
//!
//! Create a scratch image on macOS, outside measured operations:
//!
//! ```sh
//! hdiutil create -size 16g -type SPARSE -fs 'Case-sensitive APFS' \
//!   -volname WorktreeBenchmark -nospotlight /private/tmp/worktrees.sparseimage
//! mkdir -p /private/tmp/worktrees-mount
//! hdiutil attach /private/tmp/worktrees.sparseimage \
//!   -mountpoint /private/tmp/worktrees-mount -nobrowse
//! # Set WORKTREE_BENCH_REPO to your source repository before running.
//! WORKTREE_BENCH_TMPDIR=/private/tmp/worktrees-mount target/release/deps/worktrees-<hash> \
//!   > /private/tmp/worktree-samples.jsonl
//! hdiutil detach /private/tmp/worktrees-mount
//! ```
//!
//! Save the pre-migration executable to compare both implementations on the same
//! seed and image. Alternate before/after invocations with one measured round per
//! invocation and collect JSONL samples outside the checkout. Record the results as
//! Markdown in the commit message, then discard temporary samples and logs.
//! Preparation, synchronization, disk accounting, validation and forced-removal
//! dirtiness are outside the timers.
//! Creation includes registration, branch creation, files and index; removal includes
//! safety checks and recursive deletion. Branch cleanup is untimed. Registration-only
//! worktrees need forced removal even without injected dirtiness.
//!
//! Effective disk usage is the isolated volume's change in free blocks after volume
//! synchronization. It includes checkout/private administration and incremental shared
//! Git metadata. Unlike `du`, it accounts for partial sharing and filesystem metadata.
//! An allocation calibration checks a full write, APFS clone, and partial overwrite.
//! Logical bytes are reported separately. Keep other writers off the image.
//!
//! Measurements describe warm-cache macOS APFS-image performance, not durable-write
//! latency or native-volume/cold-cache performance. COW on Linux is a full-copy test
//! mock and cannot produce valid APFS comparison numbers.
//!
//! The explicitly invoked concurrency regression is:
//!
//! ```sh
//! cargo test -p but-workspace remove_finishes_when_a_bounded_writer \
//!   -- --ignored --nocapture
//! ```
//!
//! Before migration, 32 writes arriving after deletion of the marker reproduced
//! `Directory not empty`: checkout remained, while administration was already gone.
//! This stress is scheduling-sensitive; upstream gix also has deterministic coverage
//! injecting one late write after scanning each root.
//!
//! Production removal inherits gix's automatic worker count, capped at four on
//! macOS. Native APFS measurements favored four over three workers for Git-created,
//! gix-created, and CoW checkouts. Use native-volume timings to tune worker counts;
//! the sparse image needed for isolated space accounting can distort deletion costs.
//! Safety checks and the maximum retry count are unchanged.
//!
//! Full creation preserves Git's `post-checkout` hook through `git hook run` after
//! gix completes registration and checkout. No-checkout registration skips it.
//! A hook error retains the completed worktree, matching Git.
use anyhow::{Context, Result, ensure};
use bstr::ByteSlice;
use but_testsupport::gix_testtools;
use but_workspace::worktrees;

#[cfg(target_os = "macos")]
use std::{ffi::CString, os::unix::ffi::OsStrExt};
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

fn git_args(root: &Path, args: &[impl AsRef<OsStr>]) -> Result<String> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn env_usize(name: &str, default: usize) -> Result<usize> {
    std::env::var(name).map_or(Ok(default), |value| {
        value.parse().with_context(|| name.to_owned())
    })
}

#[cfg(target_os = "macos")]
fn free_bytes(path: &Path) -> Result<u64> {
    unsafe extern "C" {
        fn sync_volume_np(path: *const libc::c_char, flags: libc::c_int) -> libc::c_int;
    }
    let path = CString::new(path.as_os_str().as_bytes())?;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: a live C string and appropriately sized output storage are passed to native APIs.
    unsafe {
        if sync_volume_np(path.as_ptr(), 2) != 0
            || libc::statfs(path.as_ptr(), stats.as_mut_ptr()) != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let stats = stats.assume_init();
        Ok(stats.f_bfree * u64::from(stats.f_bsize))
    }
}
#[cfg(not(target_os = "macos"))]
fn free_bytes(_: &Path) -> Result<u64> {
    anyhow::bail!("effective allocation measurement requires an isolated APFS volume on macOS")
}

fn logical_bytes(path: &Path) -> Result<u64> {
    let mut bytes = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        bytes += if meta.is_dir() {
            logical_bytes(&entry.path())?
        } else {
            meta.len()
        };
    }
    Ok(bytes)
}
/// Validate that the scratch volume's free-block accounting reflects copy-on-write sharing.
///
/// Outside the benchmark timers, write a 16 MiB file, clone it with [`fs::copy`], then overwrite
/// the clone's first 1 MiB. Each allocation reading synchronizes the volume through [`free_bytes`].
/// Require at least 16 MiB allocated for the original, less than 8 MiB additional allocation for
/// the clone, and at least 1 MiB additional allocation for the overwrite. These are sanity checks,
/// not exact size measurements; filesystem metadata can contribute to the deltas.
///
/// `root` must be on an isolated macOS APFS volume without other writers, since the readings are
/// volume-wide. Log the byte deltas and remove `root/accounting-calibration` on success; on failure,
/// return an error and leave any calibration files for inspection.
fn verify_allocation_accounting(root: &Path) -> Result<()> {
    let path = root.join("accounting-calibration");
    fs::create_dir(&path)?;
    let before = free_bytes(root)?;
    let original = path.join("original");
    fs::write(&original, vec![42; 16 * 1024 * 1024])?;
    let written = free_bytes(root)?;
    // std::fs::copy uses clonefile on macOS, as does production COW.
    let cloned = path.join("clone");
    fs::copy(&original, &cloned)?;
    let shared = free_bytes(root)?;
    fs::OpenOptions::new()
        .write(true)
        .open(&cloned)?
        .write_all(&vec![7; 1024 * 1024])?;
    let changed = free_bytes(root)?;
    ensure!(
        before.saturating_sub(written) >= 16 * 1024 * 1024,
        "volume allocation must see full writes"
    );
    ensure!(
        written.saturating_sub(shared) < 8 * 1024 * 1024,
        "volume must support shared APFS clones"
    );
    ensure!(
        shared.saturating_sub(changed) >= 1024 * 1024,
        "volume allocation must see partial clone writes"
    );
    fs::remove_dir_all(path)?;
    eprintln!(
        "accounting calibration: write={}, clone={}, partial_write={}",
        before.saturating_sub(written),
        written.saturating_sub(shared),
        shared.saturating_sub(changed)
    );
    Ok(())
}
fn main() -> Result<()> {
    let source = PathBuf::from(
        std::env::var_os("WORKTREE_BENCH_REPO")
            .filter(|value| !value.is_empty())
            .context("set WORKTREE_BENCH_REPO to the source repository to benchmark")?,
    );
    let root = PathBuf::from(
        std::env::var_os("WORKTREE_BENCH_TMPDIR")
            .filter(|value| !value.is_empty())
            .context("set WORKTREE_BENCH_TMPDIR to an isolated case-sensitive APFS image mount")?,
    );
    fs::create_dir_all(&root)?;
    let case_probe = root.join("case-probe");
    fs::write(&case_probe, b"probe")?;
    let case_sensitive = !root.join("CASE-PROBE").exists();
    fs::remove_file(case_probe)?;
    ensure!(
        case_sensitive,
        "Linux checkout requires case-sensitive scratch storage"
    );
    let base = git_args(&source, &["rev-parse", "HEAD"])?;
    let seed = root.join("seed");
    if !seed.exists() {
        fs::create_dir(&seed)?;
        git_args(&seed, &["init", "-q"])?;
        let objects = git_args(
            &source,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "objects",
            ],
        )?;
        fs::write(
            seed.join(".git/objects/info/alternates"),
            format!("{objects}\n"),
        )?;
        for (key, value) in [
            ("user.name", "Worktree benchmark"),
            ("user.email", "benchmark@example.invalid"),
            ("core.ignorecase", "false"),
            ("core.autocrlf", "false"),
            ("core.fsmonitor", "false"),
            ("gc.auto", "0"),
        ] {
            git_args(&seed, &["config", key, value])?;
        }
        git_args(&seed, &["update-ref", "refs/heads/seed", &base])?;
        git_args(&seed, &["symbolic-ref", "HEAD", "refs/heads/seed"])?;
        git_args(&seed, &["reset", "--hard", &base])?;
    }
    ensure!(
        git_args(&seed, &["rev-parse", "HEAD"])? == base,
        "seed must match pinned source HEAD"
    );
    ensure!(
        git_args(&seed, &["status", "--porcelain"])?.is_empty(),
        "seed must remain clean"
    );
    verify_allocation_accounting(&root)?;
    let repo = gix::open(&seed)?;
    let base_id = base.parse::<gix::ObjectId>()?;
    let runs = env_usize("WORKTREE_BENCH_RUNS", 10)?;
    let warmups = env_usize("WORKTREE_BENCH_WARMUPS", 2)?;
    let label = std::env::var("WORKTREE_BENCH_LABEL").unwrap_or_else(|_| "production".into());
    eprintln!(
        "label={label}, base={base}, git={}, runs={runs}, warmups={warmups}",
        git_args(&seed, &["--version"])?
    );
    for round in 0..warmups + runs {
        for mode in ["full", "cow", "registration"] {
            for force in [false, true] {
                let path = root.join("candidate");
                let branch: &gix::refs::FullNameRef = "refs/heads/benchmark".try_into()?;
                let before = free_bytes(&root)?;
                let started = Instant::now();
                let name = match mode {
                    "full" => worktrees::add(&repo, &path, branch, base_id)?,
                    "cow" => worktrees::add_cow(&repo, &path, branch, base_id)?,
                    "registration" => {
                        repo.reference(
                            branch,
                            base_id,
                            gix::refs::transaction::PreviousValue::MustNotExist,
                            "benchmark branch",
                        )?;
                        let created = repo
                            .prepare_add_worktree(
                                &path,
                                gix::worktree::add::Head::Attached(branch.to_owned()),
                                &std::sync::atomic::AtomicBool::default(),
                            )?
                            .persist()?;
                        created
                            .worktree()
                            .expect("linked worktree")
                            .id()?
                            .expect("linked id")
                            .to_owned()
                    }
                    _ => unreachable!(),
                };
                let create_seconds = started.elapsed().as_secs_f64();
                let after = free_bytes(&root)?;
                let git_dir = repo
                    .worktree_proxy_by_id(name.as_bstr())?
                    .expect("registered candidate")
                    .git_dir()
                    .to_owned();
                let logical = logical_bytes(&path)? + logical_bytes(&git_dir)?;
                if mode != "registration" {
                    ensure!(
                        git_args(&path, &["status", "--porcelain"])?.is_empty(),
                        "candidate must be a complete clean checkout"
                    );
                }
                if force {
                    fs::write(path.join("Makefile"), b"changed for forced removal\n")?;
                    fs::write(path.join("benchmark-untracked"), b"untracked\n")?;
                }
                // Registration without checkout reports tracked deletions and requires forced removal.
                let started = Instant::now();
                worktrees::remove(&repo, name.as_bstr(), force || mode == "registration")?;
                let remove_seconds = started.elapsed().as_secs_f64();
                ensure!(
                    !path.exists() && !git_dir.exists(),
                    "both removal roots must disappear"
                );
                repo.find_reference(branch)?.delete()?;
                if round >= warmups {
                    println!(
                        "{{\"label\":\"{label}\",\"mode\":\"{mode}\",\"force\":{force},\"round\":{},\"create_seconds\":{create_seconds},\"remove_seconds\":{remove_seconds},\"allocated_bytes\":{},\"logical_bytes\":{logical}}}",
                        round - warmups,
                        i128::from(before) - i128::from(after)
                    );
                }
            }
        }
    }
    Ok(())
}
