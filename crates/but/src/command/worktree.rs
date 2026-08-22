//! Implementation of the `but worktree` command.
//!
//! Worktrees created here are ordinary linked git worktrees. GitButler discovers them through
//! git's own registry and seeds their `HEAD`s as extra traversal tips, so `but status` shows one
//! as soon as it exists — this command owns creation only, never the worktree's lifecycle.

use std::path::Path;

use anyhow::{Context as _, Result, bail};
use but_ctx::Context;

use crate::utils::WriteWithUtils;

/// Create a worktree at `path`, checked out at the workspace's base commit.
pub fn new(ctx: &Context, out: &mut dyn WriteWithUtils, path: &Path, cow: bool) -> Result<()> {
    let repo = ctx.repo.get()?;
    let base = workspace_base(ctx)?;
    let source = ctx.workdir_or_fail()?;

    if path.exists() {
        bail!("'{}' already exists", path.display());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create '{}'", parent.display()))?;

    // The clone is only worth attempting when the destination can actually share blocks with the
    // source; a cross-filesystem clone silently costs a full copy, which is what we set out to
    // avoid.
    let clone = cow && cow_supported(&source, parent);
    if cow && !clone {
        writeln!(
            out,
            "Copy-on-write is unavailable here (unsupported filesystem, or a different one than the \
             source); falling back to a normal checkout."
        )?;
    }

    if clone {
        git_worktree_add(repo.common_dir(), path, base, WithCheckout::No)?;
        clone_worktree_contents(&source, path)?;
        // The clone left the worktree holding this working directory's content while its index is
        // empty. Tell git what it is looking at, then move it to the base commit: only the paths
        // that actually differ get rewritten, and untracked build output is left alone.
        let head = repo.head_id()?.detach();
        git_in_worktree(path, &["reset", "--mixed", &head.to_string()])?;
        git_in_worktree(path, &["reset", "--hard", &base.to_string()])?;
    } else {
        git_worktree_add(repo.common_dir(), path, base, WithCheckout::Yes)?;
    }

    writeln!(out, "Created worktree at: {}", path.display())?;
    writeln!(out, "Base: {base}")?;
    if clone {
        writeln!(
            out,
            "Populated by copy-on-write clone; untracked build output came along."
        )?;
    }
    Ok(())
}

/// The commit every applied branch in the workspace forks from.
fn workspace_base(ctx: &Context) -> Result<gix::ObjectId> {
    let guard = ctx.shared_worktree_access();
    let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    ws.lower_bound
        .context("the workspace has no common base to create a worktree from")
}

enum WithCheckout {
    Yes,
    No,
}

/// Create the linked worktree, detached at `commit`.
fn git_worktree_add(
    common_dir: &Path,
    path: &Path,
    commit: gix::ObjectId,
    checkout: WithCheckout,
) -> Result<()> {
    let mut args: Vec<&str> = vec!["worktree", "add", "--detach"];
    if matches!(checkout, WithCheckout::No) {
        args.push("--no-checkout");
    }
    let commit = commit.to_string();
    run_git(
        common_dir,
        &args,
        &[path.as_os_str().to_owned(), commit.into()],
    )
}

fn git_in_worktree(worktree: &Path, args: &[&str]) -> Result<()> {
    run_git(worktree, args, &[])
}

fn run_git(dir: &Path, args: &[&str], trailing: &[std::ffi::OsString]) -> Result<()> {
    let mut command =
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()));
    command.current_dir(dir).args(args).args(trailing);
    let output = command.stderr(std::process::Stdio::piped()).output()?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "git {} failed\n\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

/// Clone every entry of `source` into `dest`, skipping the `.git` file that marks the worktree.
fn clone_worktree_contents(source: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("failed to read '{}'", source.display()))?
    {
        let entry = entry?;
        // The linked worktree has its own `.git` file, pointing at its admin directory.
        if entry.file_name() == ".git" {
            continue;
        }
        let target = dest.join(entry.file_name());
        if target.exists() {
            continue;
        }
        clone_path(&entry.path(), &target).with_context(|| {
            format!(
                "failed to clone '{}' to '{}'",
                entry.path().display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

/// Whether a copy-on-write clone from `source` into `dest_parent` will actually share blocks.
///
/// Answered by cloning a probe file rather than by inspecting filesystem types: the filesystem may
/// support cloning while these two paths sit on different volumes, where it cannot help.
fn cow_supported(source: &Path, dest_parent: &Path) -> bool {
    let probe = source.join(".but-cow-probe");
    let clone = dest_parent.join(".but-cow-probe-clone");
    let _ = std::fs::remove_file(&clone);
    if std::fs::write(&probe, b"probe").is_err() {
        return false;
    }
    let supported = clone_path(&probe, &clone).is_ok();
    let _ = std::fs::remove_file(&probe);
    let _ = std::fs::remove_file(&clone);
    supported
}

/// Clone `source` to `dest` sharing storage, recursively for a directory.
///
/// `clonefile` has no safe wrapper in the dependency tree, and cloning a directory tree in one
/// syscall is the whole point of this path — walking it file by file would cost one syscall per
/// file across build output.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn clone_path(source: &Path, dest: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("path contains an interior nul byte"))?;
    let dest = CString::new(dest.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("path contains an interior nul byte"))?;
    // SAFETY: both pointers come from `CString`s that outlive the call, so they are valid,
    // NUL-terminated C strings; `clonefile` only reads them. A flags value of 0 is always valid.
    // It clones a directory tree in one call, and refuses if the destination already exists.
    let status = unsafe { libc::clonefile(source.as_ptr(), dest.as_ptr(), 0) };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn clone_path(_source: &Path, _dest: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "copy-on-write cloning is not implemented on this platform",
    ))
}
