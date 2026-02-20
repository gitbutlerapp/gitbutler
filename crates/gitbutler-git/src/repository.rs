use std::{collections::HashMap, path::Path};

use gix::bstr::ByteSlice;

use super::executor::{AskpassServer, GitExecutor, Pid, Socket};
use crate::RefSpec;

#[cfg(feature = "askpass")]
use futures::{FutureExt, select};
#[cfg(feature = "askpass")]
use rand::{Rng, SeedableRng};
#[cfg(feature = "askpass")]
use std::time::Duration;

#[cfg(feature = "askpass")]
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
    #[error("git command exited with non-zero exit code {status}: {args:?}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}")]
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
async fn execute_with_auth_harness<P, F, Fut, E, Extra>(
    harness_env: HarnessEnv<P>,
    executor: &E,
    args: &[&str],
    envs: Option<HashMap<String, String>>,
    // below arguments only used if askpass is enabled
    mut _on_prompt: F,
    _extra: Extra,
) -> Result<(usize, String, String), Error<E>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String, Extra) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Extra: Send + Clone,
{
    #[cfg(feature = "askpass")]
    return execute_with_indirect_askpass(harness_env, executor, args, envs, _on_prompt, _extra).await;
    #[cfg(not(feature = "askpass"))]
    return execute_direct(harness_env, executor, args, envs).await;
}

#[cfg(feature = "askpass")]
/// Askpass-aware execution of Git commands, allowing the GUI to communicate with the askpass
/// process over a pipe.
async fn execute_with_indirect_askpass<P, F, Fut, E, Extra>(
    harness_env: HarnessEnv<P>,
    executor: &E,
    args: &[&str],
    envs: Option<HashMap<String, String>>,
    mut on_prompt: F,
    extra: Extra,
) -> Result<(usize, String, String), Error<E>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String, Extra) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Extra: Send + Clone,
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

    let askpath_path = current_exe
        .with_file_name("gitbutler-git-askpass")
        .with_extension(std::env::consts::EXE_EXTENSION);

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
            (askpath_path.strip_prefix(&workdir).unwrap_or(&askpath_path), prefix)
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

    let mut envs = envs.unwrap_or_default();
    envs.insert("GITBUTLER_ASKPASS_PIPE".into(), sock_server.to_string());
    envs.insert("GITBUTLER_ASKPASS_SECRET".into(), secret.clone());

    // Note: SSH_ASKPASS_REQUIRE is available since SSH 8.4, which was released in 2020, and as
    // such has further backwards compatibility than we do with the Tauri GUI in general. As such,
    // we use it. See https://www.openssh.org/txt/release-8.4
    //
    // At the time of writing, our oldest supported OS is Ubuntu 22.04, and it has OpenSSH 8.9.
    //
    // Note that when setting SSH_ASKPAS_REQUIRE to force, we do NOT need to set DISPLAY and we do
    // NOT need to disconnect from the controlling terminal, as is otherwise the case for SSH to
    // consider having a peek at the SSH_ASKPASS variable.
    //
    // See the OpenSSH client manual for more info.
    envs.insert("SSH_ASKPASS".into(), askpath_path.display().to_string());
    envs.insert("SSH_ASKPASS_REQUIRE".into(), "force".into());

    let git_ssh_command = resolve_git_ssh_command(&harness_env, &envs);
    envs.insert("GIT_SSH_COMMAND".into(), git_ssh_command);

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
                let response = on_prompt(prompt.clone(), extra.clone()).await;
                if let Some(response) = response {
                    sock.write_line(&response).await.map_err(Error::<E>::AskpassIo)?;
                } else {
                    return Err(Error::<E>::NeedsAuthorization(prompt));
                }
            }
        }
    }
}

#[cfg(not(feature = "askpass"))]
/// Directly execute the Git command without invoking the askpass pipe machinery. This is useful
/// for the CLI, as the child process simply inherits stdin from the parent process and therefore
/// doesn't need the askpass mechanism.
///
/// It's doubly useful in the sense that the CLI then does not need the askpass and setsid
/// binaries.
async fn execute_direct<P, E>(
    harness_env: HarnessEnv<P>,
    executor: &E,
    args: &[&str],
    envs: Option<HashMap<String, String>>,
) -> Result<(usize, String, String), Error<E>>
where
    P: AsRef<Path>,
    E: GitExecutor,
{
    let mut envs = envs.unwrap_or_default();
    envs.insert("GIT_SSH_COMMAND".into(), resolve_git_ssh_command(&harness_env, &envs));

    let cwd = match harness_env {
        HarnessEnv::Repo(p) | HarnessEnv::Global(p) => p,
    };

    executor.execute(args, cwd, Some(envs)).await.map_err(Error::<E>::Exec)
}

fn resolve_git_ssh_command<P>(harness_env: &HarnessEnv<P>, envs: &HashMap<String, String>) -> String
where
    P: AsRef<Path>,
{
    let base_ssh_command = match envs
        .get("GIT_SSH_COMMAND")
        .cloned()
        .or_else(|| envs.get("GIT_SSH").cloned())
        .or_else(|| std::env::var("GIT_SSH_COMMAND").ok())
        .or_else(|| std::env::var("GIT_SSH").ok())
    {
        Some(v) => v,
        None => get_core_sshcommand(harness_env)
            .ok()
            .flatten()
            .unwrap_or_else(|| "ssh".into()),
    };

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

    format!(
        "{base_ssh_command} -o StrictHostKeyChecking=accept-new -o KbdInteractiveAuthentication=no{additional_options}"
    )
}

fn get_core_sshcommand<P>(harness_env: &HarnessEnv<P>) -> anyhow::Result<Option<String>>
where
    P: AsRef<Path>,
{
    match harness_env {
        HarnessEnv::Repo(repo_path) => Ok(gix::open(repo_path.as_ref())?
            .config_snapshot()
            .trusted_program(&gix::config::tree::Core::SSH_COMMAND)
            .map(|program| program.to_string_lossy().into_owned())),
        HarnessEnv::Global(_) => Ok(gix::config::File::from_globals()?
            .string(gix::config::tree::Core::SSH_COMMAND)
            .map(|program| program.to_str_lossy().into_owned())),
    }
}

/// Fetches the given refspec from the given remote in the repository
/// at the given path. Any prompts for the user are passed to the asynchronous
/// callback `on_prompt` which should return the user's response or `None` if the
/// operation should be aborted, in which case an `Err` value is returned from this
/// function.
pub async fn fetch<P, F, Fut, E, Extra>(
    repo_path: P,
    executor: E,
    remote: &str,
    refspec: RefSpec,
    on_prompt: F,
    extra: Extra,
) -> Result<(), crate::Error<Error<E>>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String, Extra) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Extra: Send + Clone,
{
    let mut args = vec!["fetch", "--quiet", "--prune"];

    let refspec = refspec.to_string();

    args.push(remote);
    args.push(&refspec);

    let (status, stdout, stderr) =
        execute_with_auth_harness(HarnessEnv::Repo(repo_path), &executor, &args, None, on_prompt, extra).await?;

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

/// Pushes a refspec to the given remote in the repository at the given path.
/// Any prompts for the user are passed to the asynchronous callback `on_prompt`,
/// which should return the user's response or `None` if the operation should be
/// aborted, in which case an `Err` value is returned from this function.
#[expect(clippy::too_many_arguments)]
pub async fn push<P, F, Fut, E, Extra>(
    repo_path: P,
    executor: E,
    remote: &str,
    refspec: RefSpec,
    force: bool,
    force_push_protection: bool,
    on_prompt: F,
    extra: Extra,
    push_opts: Vec<String>,
) -> Result<String, crate::Error<Error<E>>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String, Extra) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Extra: Send + Clone,
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
        execute_with_auth_harness(HarnessEnv::Repo(repo_path), &executor, &args, None, on_prompt, extra).await?;

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

    Err(base_error.into())
}

/// Clones the given repository URL to the target directory.
/// Any prompts for the user are passed to the asynchronous callback `on_prompt`,
/// which should return the user's response or `None` if the operation should be
/// aborted, in which case an `Err` value is returned from this function.
///
/// Unlike fetch/push, this function always uses the Git CLI regardless of any
/// backend selection, as it needs to work before a repository exists.
pub async fn clone<P, F, Fut, E, Extra>(
    repository_url: &str,
    target_dir: P,
    executor: E,
    on_prompt: F,
    extra: Extra,
) -> Result<(), crate::Error<Error<E>>>
where
    P: AsRef<Path>,
    E: GitExecutor,
    F: FnMut(String, Extra) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Extra: Send + Clone,
{
    let target_dir = target_dir.as_ref();

    // For clone, we run from the parent directory of the target
    let work_dir = target_dir.parent().unwrap_or(Path::new("."));

    let target_dir_str = target_dir.to_string_lossy();
    let args = vec!["clone", "--", repository_url, &target_dir_str];

    let (status, stdout, stderr) =
        execute_with_auth_harness(HarnessEnv::Global(work_dir), &executor, &args, None, on_prompt, extra).await?;

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
