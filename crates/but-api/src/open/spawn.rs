use std::process::Command;

use anyhow::Context;

/// Spawn a process and reap it on a background thread.
///
/// Spawns like this are fire-and-forget from the caller perspective, but we still
/// wait on the child process asynchronously to avoid leaving zombie processes behind.
pub(crate) fn spawn_and_reap(
    mut cmd: Command,
    executable_name: &str,
    path: &str,
) -> anyhow::Result<()> {
    tracing::info!(?cmd, "spawn command");
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to launch {executable_name} at '{path}'"))?;

    let executable_name = executable_name.to_owned();
    let thread_executable_name = executable_name.clone();
    std::thread::Builder::new()
            .name(format!("reap-{executable_name}"))
            .stack_size(512 * 1024)
            .spawn(move || match child.wait() {
                Ok(stat) => if !stat.success() {
                    tracing::warn!(executable = %thread_executable_name, status = ?stat, "Process exited with error");
                },
                Err(err) => {
                    tracing::warn!(executable = %thread_executable_name, error = %err, "Failed to reap process")
                }
            })
            .ok();

    Ok(())
}
