//! Utilities for running Git operations in a mode that avoids implicit Git LFS hydration.

use std::{
    cell::Cell,
    ffi::OsString,
    sync::{Mutex, MutexGuard},
};

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
    use super::{GIT_LFS_SKIP_SMUDGE_ENV, LfsFastOperationScope};

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
