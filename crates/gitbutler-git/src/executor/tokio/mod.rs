//! A [Tokio](https://tokio.rs)-based Git executor implementation.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::{collections::HashMap, path::Path};
use tokio::process::Command;

#[cfg(unix)]
pub use self::unix::TokioAskpassServer;
#[cfg(windows)]
pub use self::windows::TokioAskpassServer;

/// A Git executor implementation using the `git` command-line tool
/// via [`tokio::process::Command`].
pub struct TokioExecutor;

#[allow(unsafe_code)]
unsafe impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;
    type ServerHandle = TokioAskpassServer;

    async fn execute_raw<P: AsRef<Path>>(
        &self,
        args: &[&str],
        cwd: P,
        envs: Option<HashMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        let mut cmd = Command::new({
            #[cfg(unix)]
            {
                "git"
            }
            #[cfg(windows)]
            {
                "git.exe"
            }
        });

        // Output the command being executed to stderr, for debugging purposes
        // (only on test configs).
        #[cfg(any(test, debug_assertions))]
        {
            let mut envs_str = String::new();
            if let Some(envs) = &envs {
                for (key, value) in envs.iter() {
                    envs_str.push_str(&format!("{key}={value:?} "));
                }
            }
            let args_str = args
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!("env {envs_str} git {args_str}");
        }

        cmd.kill_on_drop(true);
        cmd.args(args);
        cmd.current_dir(cwd);

        #[cfg(windows)]
        {
            // On windows, CLI applications that aren't the `windows` subsystem
            // will create and show a console window that pops up next to the
            // main application window when run. We disable this behavior when
            // running `git.exe` by setting the `CREATE_NO_WINDOW` flag.
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }

        if let Some(envs) = envs {
            cmd.envs(envs);
        }

        let output = cmd.output().await?;

        #[cfg(any(test, debug_assertions))]
        {
            eprintln!(
                "\n\n GIT STDOUT:\n\n{}\n\nGIT STDERR:\n\n{}\n\nGIT EXIT CODE: {}\n",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
                output.status.code().unwrap_or(127) as usize
            );
        }

        Ok((
            output.status.code().unwrap_or(127) as usize,
            String::from_utf8_lossy(&output.stdout).trim().into(),
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ))
    }

    async unsafe fn create_askpass_server(&self) -> Result<Self::ServerHandle, Self::Error> {
        #[cfg(unix)]
        {
            Self::ServerHandle::new().await
        }
        #[cfg(windows)]
        {
            Self::ServerHandle::new()
        }
    }

    async fn stat<P: AsRef<Path>>(&self, path: P) -> Result<super::FileStat, Self::Error> {
        #[cfg(unix)]
        {
            self::unix::stat(path).await
        }
        #[cfg(windows)]
        {
            self::windows::stat(path).await
        }
    }
}
