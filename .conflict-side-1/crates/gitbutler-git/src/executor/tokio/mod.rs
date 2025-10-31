//! A [Tokio](https://tokio.rs)-based Git executor implementation.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::{collections::HashMap, path::Path};

use gix::bstr::ByteSlice;
use tokio::process::Command;

#[cfg(unix)]
pub use self::unix::TokioAskpassServer;
#[cfg(windows)]
pub use self::windows::TokioAskpassServer;

/// A Git executor implementation using the `git` command-line tool
/// via [`tokio::process::Command`].
pub struct TokioExecutor;

#[expect(unsafe_code)]
unsafe impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;
    type ServerHandle = TokioAskpassServer;

    async fn execute_raw<P: AsRef<Path>>(
        &self,
        args: &[&str],
        cwd: P,
        envs: Option<HashMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        let git_exe = gix::path::env::exe_invocation();
        let mut cmd = Command::new(git_exe);

        cmd.kill_on_drop(true);
        cmd.current_dir(cwd);

        #[cfg(not(windows))]
        cmd.args(args);

        #[cfg(windows)]
        {
            // On Windows, we have to pass the arguments
            // as-is using a special method since Windows
            // seems to parse backslashes for some unknown
            // reason.
            for arg in args {
                cmd.raw_arg(arg);
            }

            // On windows, CLI applications that aren't the `windows` subsystem
            // will create and show a console window that pops up next to the
            // main application window when run. We disable this behavior when
            // running `git.exe` by setting the `CREATE_NO_WINDOW` flag.
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }

        if let Some(envs) = envs {
            #[cfg(not(windows))]
            cmd.envs(envs);

            // On Windows, we have to escape backslashes in
            // environment variable values. Not sure why.
            #[cfg(windows)]
            {
                cmd.envs(envs.iter().map(|(k, v)| {
                    let v = v.replace('\\', "\\\\");
                    (k, v)
                }));
            }
        }

        let output = cmd.output().await?;

        debug_log_sanitised_git_cmd(&mut cmd);

        #[cfg(test)]
        {
            eprintln!(
                "\n\n GIT STDOUT:\n\n{}\n\nGIT STDERR:\n\n{}\n\nGIT EXIT CODE: {}\n",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
                output.status.code().unwrap_or(127) as usize
            );
        }

        if !output.status.success() {
            tracing::error!(
                ?cmd,
                stdout = output.stdout.as_bstr().to_string(),
                stderr = output.stderr.as_bstr().to_string(),
                "Git invocation failed"
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

fn debug_log_sanitised_git_cmd(cmd: &mut Command) {
    cmd.env_remove("GITBUTLER_ASKPASS_SECRET")
        .env_remove("GITBUTLER_ASKPASS_PIPE")
        .env_remove("SSH_ASKPASS");
    tracing::debug!(?cmd, "sanitised Git invocation");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use assert_cmd::Command;

    use super::*;
    use crate::executor::{AskpassServer, GitExecutor, Socket};

    // cargo test --package gitbutler-git --lib test_askpass
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_askpass() {
        let secret = "super-secret-secret";
        let executor = TokioExecutor;
        #[expect(unsafe_code)]
        let sock_server: TokioAskpassServer = unsafe { executor.create_askpass_server() }
            .await
            .expect("create_askpass_server():");
        let sock_server_string = sock_server.to_string();
        let handle = tokio::spawn(async move {
            let mut cmd = Command::cargo_bin("gitbutler-git-askpass").unwrap();
            let assert = cmd
                .env("GITBUTLER_ASKPASS_PIPE", sock_server_string)
                .env("GITBUTLER_ASKPASS_SECRET", secret)
                .arg("Please enter your password:")
                .assert();
            assert.success().stdout("super_secret_password\n");
        });

        let mut sock = sock_server
            .accept(Some(Duration::from_secs(10)))
            .await
            .expect("accept():");

        let peer_secret = sock.read_line().await.expect("read_line() peer_secret:");

        assert_eq!(peer_secret, secret);

        let prompt = sock.read_line().await.expect("read_line() prompt:");
        assert_eq!(prompt.trim(), "Please enter your password:");

        sock.write_line("super_secret_password")
            .await
            .expect("write_line() password:");
        handle.await.expect("Askpass command failed");
    }
}
