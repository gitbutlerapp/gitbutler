mod detect_hook_manager;
mod detect_hook_manager_in_hooks_dir;

use std::fs;

use tempfile::TempDir;

pub(super) use crate::common::create_hooks_dir as create_hooks_dir_result;

/// Return the per-binary test env-var key used by `is_binary_in_path`.
///
/// For example, `test_env_var_for("prek")` returns
/// `"_GITBUTLER_TEST_BINARY_PREK_AVAILABLE"`.
pub(super) fn test_env_var_for(binary: &str) -> String {
    format!("_GITBUTLER_TEST_BINARY_{}_AVAILABLE", binary.to_uppercase())
}

/// Create a temp directory with a `hooks/` subdirectory, returning both.
/// Panics on I/O failure — suitable for test functions that don't return `Result`.
pub(super) fn create_hooks_dir() -> (TempDir, std::path::PathBuf) {
    create_hooks_dir_result().unwrap()
}

/// Write a minimal `prek.toml` config file in the given project directory.
pub(super) fn create_prek_config(project_dir: &std::path::Path) {
    fs::write(project_dir.join("prek.toml"), "# prek config").unwrap();
}

/// Run a closure with the binary-availability flag set for `binary`
/// (simulates the named binary being in PATH).
pub(super) fn with_binary_available<F: FnOnce() -> R, R>(binary: &str, f: F) -> R {
    temp_env::with_var(test_env_var_for(binary), Some("1"), f)
}

/// Run a closure with the binary-availability flag set to "0" for `binary`
/// (simulates the named binary NOT being in PATH).
pub(super) fn with_binary_unavailable<F: FnOnce() -> R, R>(binary: &str, f: F) -> R {
    temp_env::with_var(test_env_var_for(binary), Some("0"), f)
}
