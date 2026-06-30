use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{FutureExt, select};
use gix::bstr::ByteSlice;
use rand::{Rng, SeedableRng};

use super::executor::{AskpassServer, GitExecutor, Pid, Socket};
use crate::RefSpec;

/// The number of characters in the secret used for checking
/// askpass invocations by ssh/git when connecting to our process.
const ASKPASS_SECRET_LENGTH: usize = 24;

/// Higher level errors that can occur when interacting with the CLI.
///
/// You probably don't want to use this type. Use [`Error`] instead.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError<
    Eexec: std::error::Error + core::fmt::Debug + Send + Sync + 'static,
    Easkpass: std::error::Error + core::fmt::Debug + Send + Sync + 'static,
    Esocket: std::error::Error + core::fmt::Debug + Send + Sync + 'static,
> {
    #[error("failed to execute git command: {0}")]
    Exec(Eexec),
    #[error("failed to create askpass server: {0}")]
    AskpassServer(Easkpass),
    #[error("i/o error communicating with askpass utility: {0}")]
    AskpassIo(Esocket),
    #[error(
        "git command exited with non-zero exit code {status}:\n\nARGS:\n{args:?}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}"
    )]
    Failed {
        status: usize,
        args: Vec<String>,
        stdout: String,
        stderr: String,
    },
    #[error("failed to determine path to this executable: {0}")]
    NoSelfExe(std::io::Error),
    #[error("askpass secret mismatch")]
    AskpassSecretMismatch,
    #[error("git requires authorization credentials but none were provided: prompt was {0:?}")]
    NeedsAuthorization(String),
    #[error("unable to determine PID of askpass peer: {0}")]
    NoPid(Esocket),
    #[cfg(unix)]
    #[error("unable to determine UID of askpass peer: {0}")]
    NoUid(Esocket),
    #[error("failed to perform askpass security check; no such PID: {0}")]
    NoSuchPid(Pid),
    #[error("failed to perform askpass security check; device mismatch")]
    AskpassDeviceMismatch,
    #[error("failed to perform askpass security check; executable mismatch")]
    AskpassExecutableMismatch,
    #[error("Run `{prefix}cargo build -p gitbutler-git` to get the askpass binary at '{path}'")]
    AskpassExecutableNotFound { path: String, prefix: String },
    /// The remote configuration could not be read.
    #[error("failed to read configuration for remote `{remote}`: {source}")]
    RemoteConfiguration {
        /// The remote whose configuration could not be read.
        remote: String,
        /// The configuration lookup error.
        #[source]
        source: gix::remote::find::existing::Error,
    },
    /// The repository could not be opened.
    #[error("failed to open repository at `{}`: {source}", path.display())]
    RepositoryOpen {
        /// The repository path that could not be opened.
        path: PathBuf,
        /// The repository open error.
        #[source]
        source: gix::open::Error,
    },
}

/// Higher level errors that can occur when interacting with the CLI.
pub type Error<E> = RepositoryError<
    <E as GitExecutor>::Error,
    <<E as GitExecutor>::ServerHandle as AskpassServer>::Error,
    <<<E as GitExecutor>::ServerHandle as AskpassServer>::SocketHandle as Socket>::Error,
>;

enum HarnessEnv<P: AsRef<Path>> {
    /// The contained P is the repository's worktree directory or its `.git` directory.
    Repo(P),
    /// The contained P is the path that the command should be executed in
    Global(P),
}

#[cold]
async fn execute_with_auth_harness<P, F, Fut, E>(
    harness_env: HarnessEnv<P>,
    executor: &E,
    args: &[&str],
    on_prompt: Option<F>,
) -> Result<(usize, String, String), Error<E>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let envs = new_env_with_git_ssh_configured(&harness_env);

    if let Some(on_prompt) = on_prompt {
        execute_with_indirect_askpass(harness_env, executor, args, envs, on_prompt).await
    } else {
        execute_direct(harness_env, executor, args, envs).await
    }
}

/// Askpass-aware execution of Git commands, allowing the GUI to communicate with the askpass
/// process over a pipe or socket.
async fn execute_with_indirect_askpass<P, F, Fut, E>(
    harness_env: HarnessEnv<P>,
    executor: &E,
    args: &[&str],
    mut envs: HashMap<String, String>,
    mut on_prompt: F,
) -> Result<(usize, String, String), Error<E>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let mut current_exe = std::env::current_exe().map_err(Error::<E>::NoSelfExe)?;
    // On Windows, we get these \\? prefix that have issues. Let's do nothing there for now,
    // and otherwise switch to `gix::path::realpath()` everywhere.
    if cfg!(unix) {
        current_exe = current_exe.canonicalize().unwrap_or(current_exe);
    }

    // TODO(qix-): Get parent PID of connecting processes to make sure they're us.
    // This is a bit of a hack. Under a test environment, Cargo is running a
    // test runner with a quasi-random suffix. The actual executables live in
    // the parent directory. Thus, we have to do this under test. It's not
    // ideal, but it works for now.
    //
    // TODO: remove this special case once `gitbutler-branch-actions` is gone.
    if current_exe.iter().nth_back(1) == Some("deps".as_ref()) {
        current_exe = current_exe.parent().unwrap().to_path_buf();
    }

    let askpath_path = askpass_executable_path(&current_exe);

    let res = executor.stat(&askpath_path).await.map_err(Error::<E>::Exec);
    if res.is_err() {
        let (path, prefix) = if let Some(workdir) = std::env::current_dir().ok().and_then(|cwd| {
            gix::discover::upwards(&cwd)
                .ok()
                .and_then(|p| p.0.into_repository_and_work_tree_directories().1)
        }) {
            let prefix = std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .map(|path| {
                    format!(
                        "CARGO_TARGET_DIR={path} ",
                        path = path.strip_prefix(&workdir).unwrap_or(&path).display()
                    )
                })
                .unwrap_or_default();
            (
                askpath_path.strip_prefix(&workdir).unwrap_or(&askpath_path),
                prefix,
            )
        } else {
            (askpath_path.as_path(), "".into())
        };
        return Err(Error::<E>::AskpassExecutableNotFound {
            path: path.display().to_string(),
            prefix,
        });
    }
    let askpath_stat = res?;

    #[expect(unsafe_code)]
    let sock_server = unsafe { executor.create_askpass_server() }
        .await
        .map_err(Error::<E>::Exec)?;

    // NB: StdRng is always a cryptographically secure random generator.
    let secret = rand::rngs::StdRng::from_os_rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(ASKPASS_SECRET_LENGTH)
        .map(char::from)
        .collect::<String>();

    envs.insert("GITBUTLER_ASKPASS_PIPE".into(), sock_server.to_string());
    envs.insert("GITBUTLER_ASKPASS_SECRET".into(), secret.clone());

    // Note: SSH_ASKPASS_REQUIRE is available since SSH 8.4, which was released in 2020, and as
    // such has further backwards compatibility than we do with the Tauri GUI in general. At this
    // point it is therefore relatively safe to depend on this behavior.
    //
    // See https://www.openssh.org/txt/release-8.4
    //
    // At the time of writing, our oldest supported Linux distro is Ubuntu 22.04, and it has OpenSSH 8.9: https://packages.ubuntu.com/jammy/openssh-client
    //
    // macOS has shipped with OpenSSH 8.6 since Monterey (2021): https://www.reddit.com/r/MacOSBeta/comments/nzouk2/monterey_ssh_version/
    //
    // Windows 11 does not have a concept of a controlling terminal, but appears to require
    // SSH_ASKPASS_REQUIRE=force to look at SSH_ASKPASS: https://github.com/PowerShell/Win32-OpenSSH/issues/2115
    //
    // Note that when setting SSH_ASKPASS_REQUIRE to force, we do NOT need to set DISPLAY and we do
    // NOT need to disconnect from the controlling terminal, as is otherwise the case for SSH to
    // consider having a peek at the SSH_ASKPASS variable.
    //
    // See the OpenSSH client manual for more info.
    envs.insert("SSH_ASKPASS".into(), askpath_path.display().to_string());
    envs.insert("SSH_ASKPASS_REQUIRE".into(), "force".into());

    let cwd = match harness_env {
        HarnessEnv::Repo(p) | HarnessEnv::Global(p) => p,
    };
    let mut child_process = core::pin::pin! {
        async {
            executor
                .execute(args, cwd, Some(envs))
                .await
                .map_err(Error::<E>::Exec)
        }.fuse()
    };

    loop {
        select! {
            res = child_process => {
                return res;
            },
            res = sock_server.accept(Some(Duration::from_secs(120))).fuse() => {
                let mut sock = res.map_err(Error::<E>::AskpassServer)?;

                // get the PID of the peer
                let peer_pid = sock.pid().map_err(Error::<E>::NoPid)?;

                // get the full image path of the peer id; this is pretty expensive at the moment.
                // TODO(qix-): see if dropping sysinfo for a more bespoke implementation is worth it.
                let mut system = sysinfo::System::new();
                system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

                #[cfg(unix)]
                let peer_pid_u32: u32 = peer_pid
                    .try_into()
                    .map_err(|_| Error::<E>::NoSuchPid(peer_pid))?;
                #[cfg(windows)]
                let peer_pid_u32: u32 = peer_pid;

                let peer_path = system
                    .process(sysinfo::Pid::from_u32(peer_pid_u32))
                    .and_then(|p| p.exe().map(|exe| exe.to_string_lossy().into_owned()))
                    .ok_or(Error::<E>::NoSuchPid(peer_pid))?;

                // stat the askpass executable that is being invoked
                let peer_stat = executor.stat(&peer_path).await.map_err(Error::<E>::Exec)?;

                let valid_executable = if peer_stat.ino == askpath_stat.ino {
                    if peer_stat.dev != askpath_stat.dev {
                        return Err(Error::<E>::AskpassDeviceMismatch)?;
                    }

                    true
                } else {
                    false
                };

                if !valid_executable {
                    return Err(Error::<E>::AskpassExecutableMismatch)?;
                }

                // await for peer to send secret
                let peer_secret = sock.read_line().await.map_err(Error::<E>::AskpassIo)?;

                // check the secret
                if peer_secret.trim() != secret {
                    return Err(Error::<E>::AskpassSecretMismatch)?;
                }

                // get the prompt
                let prompt = sock.read_line().await.map_err(Error::<E>::AskpassIo)?;

                // call the prompt handler
                let response = on_prompt(prompt.clone()).await;
                if let Some(response) = response {
                    sock.write_line(&response).await.map_err(Error::<E>::AskpassIo)?;
                } else {
                    return Err(Error::<E>::NeedsAuthorization(prompt));
                }
            }
        }
    }
}

fn askpass_executable_path(current_exe: &Path) -> PathBuf {
    askpass_executable_path_from_override(std::env::var_os("GITBUTLER_ASKPASS_BIN"), current_exe)
}

fn askpass_executable_path_from_override(
    override_path: Option<OsString>,
    current_exe: &Path,
) -> PathBuf {
    override_path.map(PathBuf::from).unwrap_or_else(|| {
        current_exe
            .with_file_name("gitbutler-git-askpass")
            .with_extension(std::env::consts::EXE_EXTENSION)
    })
}

/// Directly execute the Git command without invoking the askpass pipe machinery. This is useful
/// for the CLI, as the child process simply inherits stdin from the parent process and therefore
/// doesn't need the askpass mechanism.
///
/// It's doubly useful in the sense that the CLI then does not need the askpass binary.
async fn execute_direct<P, E>(
    harness_env: HarnessEnv<P>,
    executor: &E,
    args: &[&str],
    envs: HashMap<String, String>,
) -> Result<(usize, String, String), Error<E>>
where
    P: AsRef<Path>,
    E: GitExecutor,
{
    let cwd = match harness_env {
        HarnessEnv::Repo(p) | HarnessEnv::Global(p) => p,
    };

    executor
        .execute(args, cwd, Some(envs))
        .await
        .map_err(Error::<E>::Exec)
}

/// Create an environment variable mapping that is guaranteed to have the Git SSH command configured
/// in either GIT_SSH_COMMAND or GIT_SSH.
///
/// Resolution order reflects Git: GIT_SSH_COMMAND -> core.sshCommand -> GIT_SSH
/// See https://github.com/git/git/blob/60f07c4f5c5f81c8a994d9e06b31a4a3a1679864/connect.c#L1382-L1397
///
/// If no configuration is encountered in any of these locations, we set our own default
/// configuration for `ssh` in GIT_SSH_COMMAND. This should be the case hit by the vast majority of
/// users, and it is tuned for what we believe is a good balance between security and convenience.
///
/// Note that we never add any options to existing configuration. If the user has made explicit
/// choices about how SSH should behave, we respect those choices and leave it to the user to deal
/// with the consequences.
fn new_env_with_git_ssh_configured<P>(harness_env: &HarnessEnv<P>) -> HashMap<String, String>
where
    P: AsRef<Path>,
{
    let mut envs = HashMap::new();

    // Minor correctness issue: Neither GIT_SSH_COMMAND nor GIT_SSH are required to be unicode.
    // It's entirely possible to have non-unicode byte sequences in paths, for example. This should
    // be rewritten to use `std::env::var_os()` and the executor should be tweaked to let `envs`
    // contain raw binary strings. This is however exceedingly unlikely to be a problem in practice
    // so we'll keep it like this for now.
    if let Ok(Some(git_ssh_command)) = get_git_ssh_command(harness_env) {
        envs.insert("GIT_SSH_COMMAND".into(), git_ssh_command);
        return envs;
    }

    if let Ok(git_ssh) = std::env::var("GIT_SSH") {
        envs.insert("GIT_SSH".into(), git_ssh);
        return envs;
    }

    // There is nothing preconfigured - we apply our own defaults.
    let additional_options = {
        // In test environments, we don't want to pollute the user's known hosts file.
        // So, we just use /dev/null instead.
        #[cfg(test)]
        {
            " -o UserKnownHostsFile=/dev/null"
        }
        #[cfg(not(test))]
        {
            ""
        }
    };

    let git_ssh_command = format!(
        "ssh -o StrictHostKeyChecking=accept-new -o KbdInteractiveAuthentication=no{additional_options}"
    );
    envs.insert("GIT_SSH_COMMAND".into(), git_ssh_command);
    envs
}

/// Gets GIT_SSH_COMMAND from env first, config second.
///
/// Correctness issue: If the command is not valid UTF8, this function returns Ok(None). When the
/// executor is updated to be able to handle binary strings for envs, this should be rewritten
/// accordingly.
fn get_git_ssh_command<P>(harness_env: &HarnessEnv<P>) -> anyhow::Result<Option<String>>
where
    P: AsRef<Path>,
{
    match harness_env {
        HarnessEnv::Repo(repo_path) => Ok(gix::open(repo_path.as_ref())?
            .config_snapshot()
            .trusted_program(gix::config::tree::Core::SSH_COMMAND)
            .and_then(|program| program.to_str().map(String::from))),
        HarnessEnv::Global(_) => Ok(gix::config::File::from_globals()?
            .string(gix::config::tree::Core::SSH_COMMAND)
            .and_then(|program| program.to_str().ok().map(String::from))),
    }
}

/// Fetches from the given remote in the repository using its configured fetch refspecs.
///
/// If `on_prompt` is provided, we override SSH_ASKPASS to point to our custom askpass client to
/// ferry prompts and responses between the SSH process and this process. This should be used
/// carefully as it will unceremoniously override any askpass configuration the user might have,
/// making it unsuitable for use in e.g. the CLI. It is designed to be used with a GUI/TUI.
pub async fn fetch<P, F, Fut, E>(
    repo_path: P,
    executor: E,
    remote: &str,
    on_prompt: Option<F>,
) -> Result<(), crate::Error<Error<E>>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let repo_path = repo_path.as_ref();
    let repo = gix::open(repo_path).map_err(|source| Error::<E>::RepositoryOpen {
        path: repo_path.to_owned(),
        source,
    })?;
    let refspecs = fetch_refspecs(&repo, remote).map_err(|source| {
        let remote_not_found =
            matches!(source, gix::remote::find::existing::Error::NotFound { .. });
        let source = Error::<E>::RemoteConfiguration {
            remote: remote.to_owned(),
            source,
        };
        if remote_not_found {
            crate::Error::NoSuchRemote(remote.to_owned(), source)
        } else {
            source.into()
        }
    })?;
    let mut args = vec!["fetch", "--quiet"];

    args.push(remote);
    args.extend(refspecs.iter().map(String::as_str));

    let (status, stdout, stderr) =
        execute_with_auth_harness(HarnessEnv::Repo(repo_path), &executor, &args, on_prompt).await?;

    if status == 0 {
        Ok(())
    } else {
        // Was the ref not found?
        if let Some(refname) = stderr
            .lines()
            .find(|line| line.to_lowercase().contains("couldn't find remote ref"))
            .map(|line| line.split_whitespace().last().unwrap_or_default())
        {
            Err(crate::Error::RefNotFound(refname.to_owned()))?
        } else if stderr.to_lowercase().contains("permission denied") {
            Err(crate::Error::AuthorizationFailed(Error::<E>::Failed {
                status,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            }))?
        } else {
            Err(Error::<E>::Failed {
                status,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })?
        }
    }
}

fn fetch_refspecs(
    repo: &gix::Repository,
    remote: &str,
) -> Result<Vec<String>, gix::remote::find::existing::Error> {
    let refspecs: Vec<_> = repo
        .find_remote(remote)?
        .refspecs(gix::remote::Direction::Fetch)
        .iter()
        .map(|spec| spec.to_ref().to_bstring().to_string())
        .collect();

    Ok(if refspecs.is_empty() {
        vec![format!("+refs/heads/*:refs/remotes/{remote}/*")]
    } else {
        refspecs
    })
}

/// Pushes a refspec to the given remote in the repository at the given path.
///
/// If `on_prompt` is provided, we override SSH_ASKPASS to point to our custom askpass client to
/// ferry prompts and responses between the SSH process and this process. This should be used
/// carefully as it will unceremoniously override any askpass configuration the user might have,
/// making it unsuitable for use in e.g. the CLI. It is designed to be used with a GUI/TUI.
#[expect(clippy::too_many_arguments)]
pub async fn push<P, F, Fut, E>(
    repo_path: P,
    executor: E,
    remote: &str,
    refspec: RefSpec,
    force: bool,
    force_push_protection: bool,
    on_prompt: Option<F>,
    push_opts: Vec<String>,
) -> Result<String, crate::Error<Error<E>>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let mut args = vec!["push", "--quiet", "--no-verify"];

    let refspec = refspec.to_string();

    args.push(remote);
    args.push(&refspec);

    if force {
        if force_push_protection {
            args.push("--force-with-lease");
            args.push("--force-if-includes");
        } else {
            args.push("--force");
        }
    }

    for opt in push_opts.iter() {
        args.push("-o");
        args.push(opt.as_str());
    }

    let (status, stdout, stderr) =
        execute_with_auth_harness(HarnessEnv::Repo(repo_path), &executor, &args, on_prompt).await?;

    if status == 0 {
        return Ok(stderr);
    }

    let base_error = Error::<E>::Failed {
        status,
        args: args.into_iter().map(Into::into).collect(),
        stdout,
        stderr: stderr.clone(),
    };

    if status == 1 && force && force_push_protection {
        return Err(crate::Error::ForcePushProtection(base_error));
    }

    // Check for specific error patterns in stderr
    if let Some(refname) = stderr
        .lines()
        .find(|line| line.to_lowercase().contains("does not match any"))
        .and_then(|line| line.split_whitespace().last())
    {
        return Err(crate::Error::RefNotFound(refname.to_owned()));
    }

    if stderr.to_lowercase().contains("permission denied") {
        return Err(crate::Error::AuthorizationFailed(base_error));
    }

    if stderr.to_lowercase().contains("(no new changes)") {
        return Err(crate::Error::GerritNoNewChanges(base_error));
    }

    // Detected last so the more specific typed cases above always win. A non-fast-forward
    // rejection is recoverable: the caller can re-fetch and retry.
    let lowercased_stderr = stderr.to_lowercase();
    if lowercased_stderr.contains("! [rejected]")
        || lowercased_stderr.contains("(non-fast-forward)")
        || lowercased_stderr.contains("(fetch first)")
    {
        return Err(crate::Error::NonFastForward(base_error));
    }

    Err(base_error.into())
}

/// Clones the given repository URL to the target directory.
///
/// If `on_prompt` is provided, we override SSH_ASKPASS to point to our custom askpass client to
/// ferry prompts and responses between the SSH process and this process. This should be used
/// carefully as it will unceremoniously override any askpass configuration the user might have,
/// making it unsuitable for use in e.g. the CLI. It is designed to be used with a GUI/TUI.
///
/// Unlike fetch/push, this function always uses the Git CLI regardless of any
/// backend selection, as it needs to work before a repository exists.
pub async fn clone<P, F, Fut, E>(
    repository_url: &str,
    target_dir: P,
    executor: E,
    on_prompt: Option<F>,
) -> Result<(), crate::Error<Error<E>>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let target_dir = target_dir.as_ref();

    // For clone, we run from the parent directory of the target
    let work_dir = target_dir.parent().unwrap_or(Path::new("."));

    let target_dir_str = target_dir.to_string_lossy();
    let args = vec!["clone", "--", repository_url, &target_dir_str];

    let (status, stdout, stderr) =
        execute_with_auth_harness(HarnessEnv::Global(work_dir), &executor, &args, on_prompt)
            .await?;

    if status == 0 {
        Ok(())
    } else if stderr.to_lowercase().contains("permission denied") {
        Err(crate::Error::AuthorizationFailed(Error::<E>::Failed {
            status,
            args: args.into_iter().map(Into::into).collect(),
            stdout,
            stderr,
        }))?
    } else if stderr
        .to_lowercase()
        .contains("already exists and is not an empty directory")
    {
        Err(crate::Error::RemoteExists(
            target_dir.display().to_string(),
            Error::<E>::Failed {
                status,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            },
        ))?
    } else {
        Err(Error::<E>::Failed {
            status,
            args: args.into_iter().map(Into::into).collect(),
            stdout,
            stderr,
        })?
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::Path};

    #[test]
    fn askpass_executable_path_honors_override() {
        let path = super::askpass_executable_path_from_override(
            Some(OsString::from("/tmp/custom-askpass")),
            Path::new("/Applications/GitButler Lite.app/Contents/MacOS/GitButler Lite"),
        );

        assert_eq!(
            path,
            Path::new("/tmp/custom-askpass"),
            "explicit askpass path should be used for packaged Electron apps"
        );
    }

    #[test]
    fn askpass_executable_path_falls_back_next_to_current_exe() {
        let path =
            super::askpass_executable_path_from_override(None, Path::new("/tmp/gitbutler-lite"));
        let expected = if std::env::consts::EXE_EXTENSION.is_empty() {
            "/tmp/gitbutler-git-askpass".to_string()
        } else {
            format!(
                "/tmp/gitbutler-git-askpass.{}",
                std::env::consts::EXE_EXTENSION
            )
        };

        assert_eq!(
            path,
            Path::new(&expected),
            "fallback should preserve the historical current_exe sibling lookup"
        );
    }
}
