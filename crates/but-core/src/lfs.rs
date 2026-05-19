//! Utilities for running Git operations in a mode that avoids implicit Git LFS hydration.

use std::{
    cell::Cell,
    ffi::OsString,
    fs,
    path::Path,
    process::Command,
    sync::{Mutex, MutexGuard},
};

use anyhow::{Context, Result, bail};

const GIT_LFS_SKIP_SMUDGE_ENV: &str = "GIT_LFS_SKIP_SMUDGE";

fn lfs_env_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

thread_local! {
    static LFS_SCOPE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// A process-local guard that makes Git LFS leave pointer files in the worktree.
///
/// Git LFS honors `GIT_LFS_SKIP_SMUDGE=1` in both `git-lfs smudge` and
/// `git-lfs filter-process`. Holding this guard scopes that behavior to a
/// GitButler-controlled operation and restores the previous environment value
/// when dropped.
pub struct LfsFastOperationScope {
    inner: Option<LfsFastOperationScopeInner>,
}

struct LfsFastOperationScopeInner {
    previous_skip_smudge: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl LfsFastOperationScope {
    /// Set `GIT_LFS_SKIP_SMUDGE=1` until the returned guard is dropped.
    ///
    /// This intentionally changes only the process environment. It does not
    /// alter user or repository Git configuration.
    pub fn new() -> Self {
        let is_nested = LFS_SCOPE_DEPTH.with(|depth| {
            let current = depth.get();
            depth.set(current + 1);
            current > 0
        });
        if is_nested {
            return Self { inner: None };
        }

        let guard = lfs_env_lock()
            .lock()
            .expect("LFS environment lock must not be poisoned");
        let previous_skip_smudge = std::env::var_os(GIT_LFS_SKIP_SMUDGE_ENV);
        // SAFETY: environment mutation is serialized by a process-wide mutex.
        unsafe {
            std::env::set_var(GIT_LFS_SKIP_SMUDGE_ENV, "1");
        }
        Self {
            inner: Some(LfsFastOperationScopeInner {
                previous_skip_smudge,
                _guard: guard,
            }),
        }
    }
}

impl Default for LfsFastOperationScope {
    fn default() -> Self {
        Self::new()
    }
}

/// Return true if `bytes` look like a Git LFS pointer file.
pub fn is_lfs_pointer(bytes: &[u8]) -> bool {
    bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n")
        || bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\r\n")
}

/// Replace LFS pointer files in the worktree with their local media objects.
///
/// This does not download missing objects. It only asks Git LFS to hydrate
/// pointers for objects that are already present in `.git/lfs/objects`.
pub fn checkout_worktree(workdir: &Path) -> Result<()> {
    if !worktree_has_lfs_files(workdir)? {
        return Ok(());
    }
    run_git_lfs_checkout(workdir, std::iter::empty::<&Path>())?;
    hydrate_pointer_files(workdir, list_lfs_paths(workdir)?)
}

/// Replace specific LFS pointer files with their local media objects.
pub fn checkout_paths<'a>(workdir: &Path, paths: impl IntoIterator<Item = &'a Path>) -> Result<()> {
    let paths = paths.into_iter().map(ToOwned::to_owned).collect::<Vec<_>>();
    run_git_lfs_checkout(workdir, paths.iter().map(|path| path.as_path()))?;
    hydrate_pointer_files(workdir, paths)
}

fn worktree_has_lfs_files(workdir: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["lfs", "ls-files", "--name-only"])
        .current_dir(workdir)
        .output()
        .with_context(|| format!("failed to inspect Git LFS files in {}", workdir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("'lfs' is not a git command") {
            return Ok(false);
        }
        bail!("failed to inspect Git LFS files: {}", stderr.trim());
    }
    Ok(!output.stdout.is_empty())
}

fn list_lfs_paths(workdir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let output = Command::new("git")
        .args(["lfs", "ls-files", "--name-only"])
        .current_dir(workdir)
        .output()
        .with_context(|| format!("failed to inspect Git LFS files in {}", workdir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to inspect Git LFS files: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(Into::into)
        .collect())
}

fn git_common_dir(workdir: &Path) -> Result<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(workdir)
        .output()
        .with_context(|| format!("failed to locate Git directory in {}", workdir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to locate Git directory: {}", stderr.trim());
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let common_dir = Path::new(&path);
    Ok(if common_dir.is_absolute() {
        common_dir.to_owned()
    } else {
        workdir.join(common_dir)
    })
}

fn hydrate_pointer_files(
    workdir: &Path,
    paths: impl IntoIterator<Item = std::path::PathBuf>,
) -> Result<()> {
    let lfs_objects_dir = git_common_dir(workdir)?.join("lfs").join("objects");
    for path in paths {
        let worktree_path = workdir.join(&path);
        let Ok(content) = fs::read(&worktree_path) else {
            continue;
        };
        if !is_lfs_pointer(&content) {
            continue;
        }
        let Some(oid) = pointer_oid(&content) else {
            continue;
        };
        let Some(first) = oid.get(0..2) else {
            continue;
        };
        let Some(second) = oid.get(2..4) else {
            continue;
        };
        let object_path = lfs_objects_dir.join(first).join(second).join(oid);
        if object_path.is_file() {
            fs::copy(&object_path, &worktree_path).with_context(|| {
                format!(
                    "failed to hydrate Git LFS pointer {} from {}",
                    path.display(),
                    object_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn pointer_oid(bytes: &[u8]) -> Option<&str> {
    let content = std::str::from_utf8(bytes).ok()?;
    content.lines().find_map(|line| {
        line.strip_prefix("oid sha256:")
            .filter(|oid| oid.len() >= 4)
    })
}

fn run_git_lfs_checkout<'a>(
    workdir: &Path,
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<()> {
    let mut command = Command::new("git");
    command.args(["lfs", "checkout"]).current_dir(workdir);
    let paths = paths.into_iter().collect::<Vec<_>>();
    if !paths.is_empty() {
        command.arg("--").args(paths);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to run Git LFS checkout in {}", workdir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to hydrate Git LFS files: {}", stderr.trim());
    }
    Ok(())
}

impl Drop for LfsFastOperationScope {
    fn drop(&mut self) {
        let remaining_depth = LFS_SCOPE_DEPTH.with(|depth| {
            let current = depth.get();
            let next = current.saturating_sub(1);
            depth.set(next);
            next
        });
        if remaining_depth > 0 {
            return;
        }

        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        match &inner.previous_skip_smudge {
            Some(value) => {
                // SAFETY: environment mutation is serialized by the guard held
                // by this value until after restoration.
                unsafe {
                    std::env::set_var(GIT_LFS_SKIP_SMUDGE_ENV, value);
                }
            }
            None => {
                // SAFETY: environment mutation is serialized by the guard held
                // by this value until after restoration.
                unsafe {
                    std::env::remove_var(GIT_LFS_SKIP_SMUDGE_ENV);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GIT_LFS_SKIP_SMUDGE_ENV, LfsFastOperationScope, is_lfs_pointer};

    #[test]
    fn restores_skip_smudge_env() {
        // SAFETY: LfsFastOperationScope serializes access for the duration of the test.
        unsafe {
            std::env::remove_var(GIT_LFS_SKIP_SMUDGE_ENV);
        }

        {
            let _scope = LfsFastOperationScope::new();
            assert_eq!(
                std::env::var(GIT_LFS_SKIP_SMUDGE_ENV).as_deref(),
                Ok("1"),
                "guard should enable skip-smudge while held"
            );
        }

        assert!(
            std::env::var_os(GIT_LFS_SKIP_SMUDGE_ENV).is_none(),
            "guard should restore the absent environment value"
        );
        unsafe {
            std::env::set_var(GIT_LFS_SKIP_SMUDGE_ENV, "0");
        }

        {
            let _scope = LfsFastOperationScope::new();
            assert_eq!(
                std::env::var(GIT_LFS_SKIP_SMUDGE_ENV).as_deref(),
                Ok("1"),
                "guard should override skip-smudge while held"
            );
        }

        assert_eq!(
            std::env::var(GIT_LFS_SKIP_SMUDGE_ENV).as_deref(),
            Ok("0"),
            "guard should restore the previous environment value"
        );

        // SAFETY: Clean up the value this test installed.
        unsafe {
            std::env::remove_var(GIT_LFS_SKIP_SMUDGE_ENV);
        }
    }

    #[test]
    fn detects_lfs_pointer_content() {
        assert!(
            is_lfs_pointer(
                b"version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 123\n"
            ),
            "Git LFS pointers must be detected before they are left in the worktree"
        );
        assert!(
            is_lfs_pointer(
                b"version https://git-lfs.github.com/spec/v1\r\noid sha256:abc\r\nsize 123\r\n"
            ),
            "Git LFS pointers may use CRLF line endings on Windows"
        );
        assert!(
            !is_lfs_pointer(b"%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n"),
            "Unity scene YAML must not be mistaken for a Git LFS pointer"
        );
    }

    #[test]
    fn nested_scope_restores_only_after_outer_scope_drops() {
        // SAFETY: LfsFastOperationScope serializes access while the outer guard is held.
        unsafe {
            std::env::set_var(GIT_LFS_SKIP_SMUDGE_ENV, "0");
        }

        {
            let _outer = LfsFastOperationScope::new();
            {
                let _inner = LfsFastOperationScope::new();
                assert_eq!(
                    std::env::var(GIT_LFS_SKIP_SMUDGE_ENV).as_deref(),
                    Ok("1"),
                    "nested guard should keep skip-smudge enabled"
                );
            }

            assert_eq!(
                std::env::var(GIT_LFS_SKIP_SMUDGE_ENV).as_deref(),
                Ok("1"),
                "inner guard drop should not restore while the outer guard is still held"
            );
        }

        assert_eq!(
            std::env::var(GIT_LFS_SKIP_SMUDGE_ENV).as_deref(),
            Ok("0"),
            "outer guard should restore the original environment value"
        );

        // SAFETY: Clean up the value this test installed.
        unsafe {
            std::env::remove_var(GIT_LFS_SKIP_SMUDGE_ENV);
        }
    }
}
