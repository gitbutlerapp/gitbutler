use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use futures::{FutureExt, select};
use gix::bstr::ByteSlice;
use rand::{Rng, SeedableRng};

use super::executor::{AskpassServer, GitExecutor, Pid, Socket};
use crate::RefSpec;

/// The number of characters in the secret used for checking
/// askpass invocations by ssh/git when connecting to our process.
const ASKPASS_SECRET_LENGTH: usize = 24;

/// Errors from native Git operations and their authentication helpers.
///
/// You probably don't want to use this type. Use [`Error`] instead.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError<
    Eexec: std::error::Error + core::fmt::Debug + Send + Sync + 'static,
    Easkpass: std::error::Error + core::fmt::Debug + Send + Sync + 'static,
    Esocket: std::error::Error + core::fmt::Debug + Send + Sync + 'static,
> {
    #[error("askpass system operation failed: {0}")]
    Exec(Eexec),
    #[error(transparent)]
    Native(#[from] gix::Error),
    #[error("could not start native Git worker: {0}")]
    WorkerStart(std::io::Error),
    #[error("native Git worker stopped unexpectedly")]
    WorkerStopped,
    #[error("push to {destination} was rejected ({reason:?})\n{output}")]
    PushRejected {
        destination: String,
        reason: gix::push::Rejection,
        output: String,
    },
    #[error(
        "push succeeded, but local tracking references could not be updated: {source}\n{output}"
    )]
    PushTracking {
        #[source]
        source: gix::Error,
        output: String,
    },
    #[error("failed to accept connection to askpass server: {0}")]
    AskpassServer(Easkpass),
    #[error("i/o error communicating with askpass utility: {0}")]
    AskpassIo(Esocket),
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
        source: gix::Error,
    },
    /// The repository could not be opened.
    #[error("failed to open repository at `{}`: {source}", path.display())]
    RepositoryOpen {
        /// The repository path that could not be opened.
        path: PathBuf,
        /// The repository open error.
        #[source]
        source: gix::Error,
    },
}

/// Errors from native Git operations and their authentication helpers.
pub type Error<E> = RepositoryError<
    <E as GitExecutor>::Error,
    <<E as GitExecutor>::ServerHandle as AskpassServer>::Error,
    <<<E as GitExecutor>::ServerHandle as AskpassServer>::SocketHandle as Socket>::Error,
>;

enum HarnessEnv<'a> {
    /// Read configuration from this worktree directory or `.git` directory.
    Repo(&'a Path),
    /// Read global configuration before a clone has a repository.
    Global,
}

async fn with_authentication<F, Fut, E, Run, RunFuture, T>(
    executor: &E,
    envs: HashMap<String, String>,
    on_prompt: Option<F>,
    run: Run,
) -> Result<T, Error<E>>
where
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Run: FnOnce(HashMap<String, String>) -> RunFuture,
    RunFuture: std::future::Future<Output = Result<T, Error<E>>>,
{
    if let Some(on_prompt) = on_prompt {
        execute_with_indirect_askpass(executor, envs, on_prompt, run).await
    } else {
        run(envs).await
    }
}

/// Git's HTTP transport has no timeout, so a remote that accepts the connection and then goes
/// silent hangs the operation forever. These settings abort a transfer that moves less than
/// 1 byte per second for 60 seconds, while slow transfers that still make progress keep going.
const HTTP_STALL_LIMITS: [(&str, &str); 2] =
    [("http.lowSpeedLimit", "1"), ("http.lowSpeedTime", "60")];

/// Returns config overrides for each of [`HTTP_STALL_LIMITS`] the user has not configured.
fn http_stall_limit_overrides(harness_env: &HarnessEnv<'_>) -> Vec<String> {
    let config = match harness_env {
        HarnessEnv::Repo(repo_path) => gix::open(repo_path)
            .ok()
            .map(|repo| repo.config_snapshot().plumbing().clone()),
        HarnessEnv::Global => gix::config::File::from_globals().ok().map(|mut config| {
            // Git applies `GIT_CONFIG_*` overrides on top; a repository's config has them already.
            if let Ok(overrides) = gix::config::File::from_environment_overrides() {
                config.append(overrides).ok();
            }
            config
        }),
    };
    HTTP_STALL_LIMITS
        .into_iter()
        .filter(|(key, _)| {
            config
                .as_ref()
                .is_none_or(|config| config.raw_value(*key).is_err())
        })
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

/// Run an operation while the GUI answers its helper prompts over a pipe or socket.
async fn execute_with_indirect_askpass<F, Fut, E, Run, RunFuture, T>(
    executor: &E,
    mut envs: HashMap<String, String>,
    mut on_prompt: F,
    run: Run,
) -> Result<T, Error<E>>
where
    E: GitExecutor,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
    Run: FnOnce(HashMap<String, String>) -> RunFuture,
    RunFuture: std::future::Future<Output = Result<T, Error<E>>>,
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

    let mut child_process = core::pin::pin!(run(envs).fuse());

    loop {
        select! {
            res = child_process => {
                return res;
            },
            res = sock_server.accept().fuse() => {
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
                        Err(Error::<E>::AskpassDeviceMismatch)?;
                    }

                    true
                } else {
                    false
                };

                if !valid_executable {
                    Err(Error::<E>::AskpassExecutableMismatch)?;
                }

                // await for peer to send secret
                let peer_secret = sock.read_line().await.map_err(Error::<E>::AskpassIo)?;

                // check the secret
                if peer_secret.trim() != secret {
                    Err(Error::<E>::AskpassSecretMismatch)?;
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
fn new_env_with_git_ssh_configured(harness_env: &HarnessEnv<'_>) -> HashMap<String, String> {
    let mut envs = HashMap::new();

    // These values can contain non-Unicode paths; preserving them requires an OsString environment map.
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

    // Keepalives close the connection once the server hasn't answered for 60 seconds
    // (4 probes, 15 seconds apart); without them a half-open connection hangs forever.
    let git_ssh_command = format!(
        "ssh -o StrictHostKeyChecking=accept-new -o KbdInteractiveAuthentication=no -o ServerAliveInterval=15 -o ServerAliveCountMax=4{additional_options}"
    );
    envs.insert("GIT_SSH_COMMAND".into(), git_ssh_command);
    envs
}

/// Gets GIT_SSH_COMMAND from env first, config second.
///
/// A non-UTF-8 command returns `Ok(None)` until the authentication environment supports `OsString`.
fn get_git_ssh_command(harness_env: &HarnessEnv<'_>) -> anyhow::Result<Option<String>> {
    match harness_env {
        HarnessEnv::Repo(repo_path) => Ok(gix::open(repo_path)?
            .config_snapshot()
            .trusted_program(gix::config::tree::Core::SSH_COMMAND)?
            .and_then(|program| program.to_str().map(String::from))),
        HarnessEnv::Global => Ok(gix::config::File::from_globals()?
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
        let remote_not_found = source.is_not_found();
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
    let harness_env = HarnessEnv::Repo(repo_path);
    let mut envs = new_env_with_git_ssh_configured(&harness_env);
    envs.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    envs.insert("LC_ALL".into(), "C".into());
    let mut overrides = http_stall_limit_overrides(&harness_env);
    overrides.push("protocol.version=2".into());
    let repo_path = repo_path.to_owned();
    let remote_name = remote.to_owned();
    with_authentication(&executor, envs, on_prompt, |envs| async move {
        run_native::<E, _>(move |interrupt| {
            use gix::remote::fetch::{Status, refs::update::Mode};

            let options = gix::open::Options::default().config_overrides(overrides);
            #[cfg(unix)]
            let options =
                options.credential_cache_daemon(but_utils::credential_cache::daemon_command());
            let mut repo = options.open(&repo_path)?.to_thread_local();
            repo.committer_or_set_generic_fallback()?;
            let environment = envs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()));
            let mut remote = repo
                .find_remote(remote_name.as_str())?
                .with_working_dir(repo_path)?
                .with_command_environment(environment);
            remote.replace_refspecs(&refspecs, gix::remote::Direction::Fetch)?;
            let mut options = remote.fetch_options()?;
            options.write_fetch_head = true;
            // Git ignores configured pruneTags when explicit refspecs are supplied.
            options.prune_tags = false;
            let outcome = remote.fetch(options, gix::progress::Discard, interrupt)?;
            let update_refs = match outcome.status {
                Status::Change { update_refs, .. } | Status::NoPackReceived { update_refs, .. } => {
                    update_refs
                }
            };
            for (update, mapping) in update_refs.updates.iter().zip(&outcome.ref_map.mappings) {
                match &update.mode {
                    Mode::NoChangeNeeded
                    | Mode::FastForward
                    | Mode::Forced
                    | Mode::New
                    | Mode::ImplicitTagNotSentByRemote => {}
                    Mode::RejectedSourceObjectNotFound { .. }
                    | Mode::RejectedTagUpdate
                    | Mode::RejectedNonFastForward
                    | Mode::RejectedToReplaceWithUnborn
                    | Mode::RejectedCurrentlyCheckedOut { .. } => {
                        gix::error::bail!("Fetch of {:?} failed: {}", mapping.local, update.mode);
                    }
                }
            }
            Ok(())
        })
        .await
    })
    .await
    .map_err(native_error::<E>)
}

fn native_error<E: GitExecutor>(error: Error<E>) -> crate::Error<Error<E>> {
    if let RepositoryError::Native(source) = &error {
        if source.is_unauthenticated() || source.is_permission_denied() {
            return crate::Error::AuthorizationFailed(error);
        }
        if source.is_not_found()
            && let Some(name) = source.metadata().find_map(|metadata| {
                match metadata.get("ref").or_else(|| metadata.get("source")) {
                    Some(gix::error::MetadataValue::Bytes(name)) => {
                        Some(String::from_utf8_lossy(name).into_owned())
                    }
                    Some(gix::error::MetadataValue::String(name)) => Some(name.clone()),
                    _ => None,
                }
            })
        {
            return crate::Error::RefNotFound(name);
        }
    }
    crate::Error::Backend(error)
}

fn fetch_refspecs(repo: &gix::Repository, remote: &str) -> gix::Result<Vec<gix::bstr::BString>> {
    let refspecs: Vec<_> = repo
        .find_remote(remote)?
        .refspecs(gix::remote::Direction::Fetch)
        .iter()
        .map(|spec| spec.to_ref().to_bstring())
        .collect();

    Ok(if refspecs.is_empty() {
        vec![format!("+refs/heads/*:refs/remotes/{remote}/*").into()]
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
    let repo_path = repo_path.as_ref().to_owned();
    let harness_env = HarnessEnv::Repo(&repo_path);
    let mut envs = new_env_with_git_ssh_configured(&harness_env);
    envs.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    envs.insert("LC_ALL".into(), "C".into());
    let mut overrides = http_stall_limit_overrides(&harness_env);
    overrides.push("protocol.version=2".into());
    let remote_name = remote.to_owned();
    let results = with_authentication(&executor, envs, on_prompt, |envs| async move {
        run_native::<E, _>(move |interrupt| {
            let options = gix::open::Options::default().config_overrides(overrides);
            #[cfg(unix)]
            let options =
                options.credential_cache_daemon(but_utils::credential_cache::daemon_command());
            let repo = options.open(&repo_path)?.to_thread_local();
            let spec = refspec.to_string();
            let parsed =
                gix::refspec::parse(spec.as_str().into(), gix::refspec::parse::Operation::Push)?;
            let mut update = gix::push::Update::from_refspec(&repo, parsed)?;
            if force && force_push_protection {
                update.lease = gix::push::Lease::Tracking {
                    force_if_includes: update.new_id.is_some(),
                };
            } else {
                update.force |= force;
            }
            let remote = repo
                .find_remote(remote_name.as_str())?
                .with_working_dir(&repo_path)?
                .with_command_environment(
                    envs.into_iter()
                        .map(|(key, value)| (key.into(), value.into())),
                );
            let push_options: Vec<_> = push_opts.into_iter().map(Into::into).collect();
            let mut results = Vec::new();
            for url in remote.urls(gix::remote::Direction::Push) {
                let selected = remote
                    .clone()
                    .with_push_url_without_url_rewrite(url.clone())?;
                let mut output = Vec::new();
                let outcome = selected.push(
                    [update.clone()],
                    gix::push::Options {
                        quiet: true,
                        push_options: &push_options,
                        ..Default::default()
                    },
                    gix::progress::Discard,
                    |_, bytes| {
                        output.extend_from_slice(bytes);
                        Ok(())
                    },
                    interrupt,
                );
                let tracking_error = outcome
                    .as_ref()
                    .ok()
                    .and_then(|outcome| selected.update_tracking_refs(outcome).err());
                results.push((
                    outcome,
                    String::from_utf8_lossy(&output).into_owned(),
                    tracking_error,
                ));
            }
            gix::error::ensure!(
                !results.is_empty(),
                gix::error::message("The remote has no push URL")
            );
            Ok(results)
        })
        .await
    })
    .await
    .map_err(native_error::<E>)?;
    let mut output = String::new();
    let mut first_error = None;
    let mut first_tracking_error = None;
    for (outcome, remote_output, tracking_error) in results {
        output.push_str(&remote_output);
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(source) => {
                first_error.get_or_insert_with(|| native_error::<E>(Error::<E>::Native(source)));
                continue;
            }
        };
        if let Some((destination, reason)) =
            outcome
                .updates
                .into_iter()
                .find_map(|update| match update.status {
                    gix::push::Status::Rejected(reason) => {
                        Some((update.update.destination, reason))
                    }
                    _ => None,
                })
        {
            use gix::push::Rejection;
            let backend = Error::<E>::PushRejected {
                destination: destination.to_string(),
                reason: reason.clone(),
                output: remote_output,
            };
            let error = match reason {
                Rejection::StaleLease | Rejection::RemoteUpdated => {
                    crate::Error::ForcePushProtection(backend)
                }
                Rejection::NonFastForward
                | Rejection::FetchFirst
                | Rejection::NeedsForce
                | Rejection::AlreadyExists => crate::Error::NonFastForward(backend),
                Rejection::Server(reason)
                    if reason.to_ascii_lowercase().contains_str("no new changes") =>
                {
                    crate::Error::GerritNoNewChanges(backend)
                }
                Rejection::Server(reason)
                    if reason
                        .to_ascii_lowercase()
                        .contains_str("permission denied") =>
                {
                    crate::Error::AuthorizationFailed(backend)
                }
                Rejection::NonCommitBranch | Rejection::Atomic | Rejection::Server(_) => {
                    crate::Error::Backend(backend)
                }
            };
            first_error.get_or_insert(error);
        } else if let Some(source) = tracking_error {
            first_tracking_error.get_or_insert_with(|| {
                crate::Error::Backend(Error::<E>::PushTracking {
                    source,
                    output: remote_output,
                })
            });
        }
    }
    match first_error.or(first_tracking_error) {
        Some(error) => Err(error),
        None => Ok(output),
    }
}

/// Clones the given repository URL to the target directory.
///
/// If `on_prompt` is provided, we override SSH_ASKPASS to point to our custom askpass client to
/// ferry prompts and responses between the SSH process and this process. This should be used
/// carefully as it will unceremoniously override any askpass configuration the user might have,
/// making it unsuitable for use in e.g. the CLI. It is designed to be used with a GUI/TUI.
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
    let target_dir = target_dir.as_ref().to_owned();
    let destination = target_dir.display().to_string();
    let repository_url = repository_url.to_owned();
    let work_dir = target_dir.parent().unwrap_or(Path::new(".")).to_owned();
    let harness_env = HarnessEnv::Global;
    let mut envs = new_env_with_git_ssh_configured(&harness_env);
    envs.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    let overrides = http_stall_limit_overrides(&harness_env);
    with_authentication(&executor, envs, on_prompt, |envs| async move {
        run_native::<E, _>(move |interrupt| {
            use gix::error::ResultExt;
            let work_dir = std::path::absolute(work_dir).or_error()?;
            let mut repository_url = gix::Url::try_from(repository_url.as_str())?;
            repository_url.canonicalize(&work_dir)?;
            let options = gix::open::Options::default().config_overrides(overrides);
            #[cfg(unix)]
            let options = options.credential_cache_daemon(but_utils::credential_cache::daemon_command());
            let environment: Vec<_> = envs.into_iter().map(|(key, value)| (key.into(), value.into())).collect();
            let mut clone = gix::clone::PrepareFetch::new(
                repository_url,
                &target_dir,
                gix::create::Kind::WithWorktree,
                Default::default(),
                options,
            )?.configure_remote({
                let environment = environment.clone();
                move |remote| Ok(remote.with_working_dir(&work_dir)?.with_command_environment(environment.clone()))
            });
            let (checkout, _) = clone.fetch_then_checkout(gix::progress::Discard, interrupt)?;
            let (repo, outcome) = checkout.with_command_environment(environment.clone())
                .main_worktree(gix::progress::Discard, interrupt)?;
            gix::error::ensure!(
                !interrupt.load(std::sync::atomic::Ordering::Relaxed),
                gix::error::cancelled("Clone checkout was interrupted")
            );
            gix::error::ensure!(
                outcome.errors.is_empty() && outcome.collisions.is_empty(),
                gix::error::message!("Clone checkout failed: {:?}, collisions: {:?}", outcome.errors, outcome.collisions)
            );
            if let Some(head_id) = repo.head()?.try_peel_to_id()? {
                use std::ffi::OsStr;
                let old_head = repo.object_hash().null().to_string();
                let new_head = head_id.to_string();
                let environment: Vec<_> = environment.iter()
                    .map(|(key, value)| (key.as_os_str(), value.as_os_str())).collect();
                for output in repo.run_hook_event_serially("post-checkout", gix::hook::Options {
                    arguments: &[OsStr::new(&old_head), OsStr::new(&new_head), OsStr::new("1")],
                    environment: &environment,
                    ..Default::default()
                })? {
                    gix::error::ensure!(output.status.success(),
                        gix::error::message!("Post-checkout hook failed: {}{}", output.stdout.as_bstr(), output.stderr.as_bstr()));
                }
            }
            Ok(())
        }).await
    }).await.map_err(|error| {
        if matches!(&error, RepositoryError::Native(source) if source.is_unauthenticated() || source.is_permission_denied()) {
            crate::Error::AuthorizationFailed(error)
        } else if matches!(&error, RepositoryError::Native(source) if source.is_validation()
            && source.metadata().any(|metadata| metadata.get("input")
                == Some(&gix::error::MetadataValue::from(destination.as_bytes())))) {
            crate::Error::RemoteExists(destination, error)
        } else {
            crate::Error::Backend(error)
        }
    })
}

async fn run_native<E: GitExecutor, T: Send + 'static>(
    run: impl FnOnce(&std::sync::atomic::AtomicBool) -> gix::Result<T> + Send + 'static,
) -> Result<T, Error<E>> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Cancel(Arc<AtomicBool>);
    impl Drop for Cancel {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    let interrupt = Arc::clone(&cancel.0);
    let (sender, receiver) = futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name("gitbutler.git".into())
        .spawn(move || {
            let _ = sender.send(run(&interrupt));
        })
        .map_err(Error::<E>::WorkerStart)?;
    receiver
        .await
        .map_err(|_| Error::<E>::WorkerStopped)?
        .map_err(Error::<E>::Native)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn fetch_distinguishes_missing_remotes_from_invalid_configuration() -> anyhow::Result<()>
    {
        let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
        let repo = gix::init(tmp.path())?;
        let on_prompt = None::<fn(String) -> std::future::Ready<Option<String>>>;
        let err = super::fetch(tmp.path(), crate::tokio::TokioExecutor, "origin", on_prompt)
            .await
            .unwrap_err();
        assert!(
            matches!(err, crate::Error::NoSuchRemote(..)),
            "a missing remote retains its public error variant"
        );

        but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
            config.set_raw_value("remote.origin.url", "https://example.com/repo")?;
            config.set_raw_value(
                "remote.origin.fetch",
                "refs/heads/*:refs/remotes/origin/*:extra",
            )?;
            Ok(())
        })?;
        let err = super::fetch(tmp.path(), crate::tokio::TokioExecutor, "origin", on_prompt)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                crate::Error::Backend(super::RepositoryError::RemoteConfiguration { .. })
            ),
            "an existing remote with an invalid refspec remains a configuration error"
        );
        Ok(())
    }
}
