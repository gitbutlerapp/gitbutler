//! Helpers for resolving binary paths.
#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;

/// Resolve the path to the current executable, assuming it's `but`, such that it can be executed.
///
/// # Linux
/// Under Linux, the `/proc/self/exe` magic symlink is maintained by the kernel to point to the
/// executable. The kernel keeps the executable's inode alive even if the file has been moved
/// or deleted, and therefore this symlink can be executed as long as the process is running.
///
/// Resolving the symlink with [`std::env::current_exe`] fully resolves `/proc/self/exe` to the
/// executable it points to. Executing that fails if the binary has been removed. This is the case
/// when emitting metrics for the `update install` command. The executable first renames itself
/// (causing `/proc/self/exe` to point to the new location) and then removes itself once the new
/// version is successfully installed (causing `/proc/self/exe` to point to a non-existing binary).
///
/// As [`std::env::current_exe`] resolves the symlink, it also means that the `but ->
/// gitbutler-tauri` symlink trick to execute the CLI via the primary `gitbutler-tauri` executable
/// fails, as it's executed with the fully resolved path.
///
/// Note that both of the above issues could _probably_ be addressed by using argv[0] as the path,
/// but as we are always guaranteed to have `/proc/self/exe` we might as well use it.
///
/// # Windows and macOS
/// Under macOS and Windows, there's no equivalent to Linux's /proc/self/exe, so we
/// can't easily refer to the "current process' program" like we can there.
///
/// On macOS, std::env::current_exe() is implemented with _NSGetExecutablePath, which provides the
/// path the executable was launched with. In the `update install` case, metrics will then be
/// emitted with the _new_ version rather than the one that actually ran the command. The only way
/// around this is to emit metrics before cleaning up the old version, but that does not seem
/// worthwhile at the moment.
///
/// On Windows, current_exe() is implemented with GetModuleFileNameW, which also
/// returns the path with which the executable was launched, and so has the same
/// problem. Although at the time of writing, `update install` is not implemented
/// for Windows.
pub fn current_exe_for_but_exec() -> std::io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Ok("/proc/self/exe".into())
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe()
    }
}

/// Determine if this program was executed with the intention of running the `but` CLI, handling the
/// case where this may be `gitbutler-tauri` invoked via symlink.
pub fn is_executed_as_but() -> anyhow::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        Ok(std::env::args_os().next().is_some_and(|exec_path| {
            // Assumption: We never self-exec the GUI itself
            exec_path == "/proc/self/exe"
                || Path::new(&exec_path)
                    .file_stem()
                    .is_some_and(|stem| stem == "but")
        }))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let current_exe = std::env::current_exe()?;
        Ok(current_exe.file_stem().is_some_and(|stem| stem == "but"))
    }
}
