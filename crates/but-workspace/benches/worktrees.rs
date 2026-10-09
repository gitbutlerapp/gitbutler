//! Real-repository benchmark; see worktrees.md. Setup and accounting are untimed.
use anyhow::{Context, Result, ensure};
use bstr::ByteSlice;
use but_workspace::worktrees;
#[cfg(target_os = "macos")]
use std::{ffi::CString, os::unix::ffi::OsStrExt};
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

fn git(root: &Path, args: &[&OsStr]) -> Result<String> {
    let output = Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
fn git_args(root: &Path, args: &[&str]) -> Result<String> {
    git(root, &args.iter().map(OsStr::new).collect::<Vec<_>>())
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
fn calibrate(root: &Path) -> Result<()> {
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
    let source = PathBuf::from(std::env::var_os("WORKTREE_BENCH_REPO").unwrap_or_else(|| {
        "/Users/byron/dev/git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux".into()
    }));
    let root = PathBuf::from(
        std::env::var_os("WORKTREE_BENCH_TMPDIR")
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
    calibrate(&root)?;
    let repo = gix::open(&seed)?;
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
                    "full" => worktrees::add(&repo, &path, branch, base.parse()?)?,
                    "cow" => worktrees::add_cow(&repo, &path, branch, base.parse()?)?,
                    "registration" => {
                        git(
                            &seed,
                            &[
                                "worktree".as_ref(),
                                "add".as_ref(),
                                "--no-checkout".as_ref(),
                                "-b".as_ref(),
                                "benchmark".as_ref(),
                                "--".as_ref(),
                                path.as_os_str(),
                                base.as_ref(),
                            ],
                        )?;
                        gix::open(&path)?
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
                worktrees::remove(&repo, &path, force || mode == "registration")?;
                let remove_seconds = started.elapsed().as_secs_f64();
                ensure!(
                    !path.exists() && !git_dir.exists(),
                    "both removal roots must disappear"
                );
                git_args(&seed, &["branch", "-D", "benchmark"])?;
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
