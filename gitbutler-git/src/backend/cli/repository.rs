use super::executor::{AskpassServer, GitExecutor, Pid, Socket};
use crate::{prelude::*, Authorization, ConfigScope};
use core::time::Duration;
use futures::{select, FutureExt};
use rand::Rng;

/// The number of characters in the secret used for checking
/// askpass invocations by ssh/git when connecting to our process.
const ASKPASS_SECRET_LENGTH: usize = 24;

/// Higher level errors that can occur when interacting with the CLI.
///
/// You probably don't want to use this type. Use [`Error`] instead.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError<
    Eexec: core::error::Error + core::fmt::Debug + Send + Sync + 'static,
    Easkpass: core::error::Error + core::fmt::Debug + Send + Sync + 'static,
    Esocket: core::error::Error + core::fmt::Debug + Send + Sync + 'static,
> {
    #[error("failed to execute git command: {0}")]
    Exec(Eexec),
    #[error("failed to create askpass server: {0}")]
    AskpassServer(Easkpass),
    #[error("i/o error communicating with askpass utility: {0}")]
    AskpassIo(Esocket),
    #[error(
        "git command exited with non-zero exit code {0}: {1:?}\n\nSTDOUT:\n{2}\n\nSTDERR:\n{3}"
    )]
    Failed(usize, Vec<String>, String, String),
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
}

/// Higher level errors that can occur when interacting with the CLI.
pub type Error<E> = RepositoryError<
    <E as GitExecutor>::Error,
    <<E as GitExecutor>::ServerHandle as AskpassServer>::Error,
    <<<E as GitExecutor>::ServerHandle as AskpassServer>::SocketHandle as Socket>::Error,
>;

/// A [`crate::Repository`] implementation using the `git` CLI
/// and the given [`GitExecutor`] implementation.
pub struct Repository<E: GitExecutor> {
    exec: E,
    path: String,
}

impl<E: GitExecutor> Repository<E> {
    /// Opens a repository using the given [`GitExecutor`].
    ///
    /// Note that this **does not** check if the repository exists,
    /// but assumes it does.
    #[inline]
    pub fn open_unchecked<P: AsRef<str>>(exec: E, path: P) -> Self {
        Self {
            exec,
            path: path.as_ref().to_owned(),
        }
    }

    /// (Re-)initializes a repository at the given path
    /// using the given [`GitExecutor`].
    pub async fn open_or_init<P: AsRef<str>>(exec: E, path: P) -> Result<Self, Error<E>> {
        let path = path.as_ref().to_owned();
        let args = vec!["init", "--quiet", &path];

        let (exit_code, stdout, stderr) =
            exec.execute(&args, None).await.map_err(Error::<E>::Exec)?;

        if exit_code == 0 {
            Ok(Self { exec, path })
        } else {
            Err(Error::<E>::Failed(
                exit_code,
                args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            ))
        }
    }

    async fn execute_with_auth_harness(
        &self,
        args: &[&str],
        envs: Option<BTreeMap<String, String>>,
        authorization: &Authorization,
    ) -> Result<(usize, String, String), Error<E>> {
        let path = std::env::current_exe().map_err(|e| Error::<E>::NoSelfExe(e))?;
        let our_pid = std::process::id();

        let askpath_path = path
            .with_file_name("gitbutler-git-askpass")
            .to_string_lossy()
            .into_owned();
        #[cfg(not(target_os = "windows"))]
        let setsid_path = path
            .with_file_name("gitbutler-git-setsid")
            .to_string_lossy()
            .into_owned();

        let askpath_stat = self
            .exec
            .stat(&askpath_path)
            .await
            .map_err(Error::<E>::Exec)?;
        #[cfg(not(target_os = "windows"))]
        let setsid_stat = self
            .exec
            .stat(&setsid_path)
            .await
            .map_err(Error::<E>::Exec)?;

        let sock_path = std::env::temp_dir().join(format!("gitbutler-git-{our_pid}.sock"));

        // FIXME(qix-): This is probably not cryptographically secure, did this in a bit
        // FIXME(qix-): of a hurry. We should probably use a proper CSPRNG here, but this
        // FIXME(qix-): is probably fine for now (as this security mechanism is probably
        // FIXME(qix-): overkill to begin with).
        let secret = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(ASKPASS_SECRET_LENGTH)
            .map(char::from)
            .collect::<String>();

        let mut envs = envs.unwrap_or_default();
        envs.insert(
            "GITBUTLER_ASKPASS_PIPE".into(),
            sock_path.to_string_lossy().into_owned(),
        );
        envs.insert("GITBUTLER_ASKPASS_SECRET".into(), secret.clone());
        envs.insert("SSH_ASKPASS".into(), askpath_path);

        // DISPLAY is required by SSH to check SSH_ASKPASS.
        // Please don't ask us why, it's unclear.
        if !std::env::var("DISPLAY").map(|v| v != "").unwrap_or(false) {
            envs.insert("DISPLAY".into(), ":".into());
        }

        #[cfg(not(target_os = "windows"))]
        envs.insert(
            "GIT_SSH_COMMAND".into(),
            format!(
                "{} {}",
                setsid_path,
                envs.get("GIT_SSH_COMMAND").unwrap_or(&"ssh".into())
            ),
        );

        if let Authorization::Ssh { private_key, .. } = authorization {
            if let Some(private_key) = private_key {
                envs.insert("GIT_SSH_VARIANT".into(), "ssh".into());
                envs.insert("GIT_SSH_KEY".into(), private_key.clone());
            }
        }

        #[allow(unsafe_code)]
        let sock_server = unsafe { self.exec.create_askpass_server() }
            .await
            .map_err(Error::<E>::Exec)?;

        let mut child_process = core::pin::pin! {async {
            self.exec
            .execute(args, Some(envs))
            .await
            .map_err(Error::<E>::Exec)
        }.fuse()};

        loop {
            select! {
                res = sock_server.accept(Some(Duration::from_secs(60))).fuse() => {
                    let mut sock = res.map_err(Error::<E>::AskpassServer)?;

                    // get the PID of the peer
                    let peer_pid = sock.pid().map_err(Error::<E>::NoPid)?;

                    // get the full image path of the peer id; this is pretty expensive at the moment.
                    // TODO(qix-): see if dropping sysinfo for a more bespoke implementation is worth it.
                    let mut system = sysinfo::System::new();
                    system.refresh_processes();
                    let peer_path = system
                        .process(sysinfo::Pid::from_u32(peer_pid.try_into().map_err(|_| Error::<E>::NoSuchPid(peer_pid))?))
                        .and_then(|p| p.exe().map(|exe| exe.to_string_lossy().into_owned()))
                        .ok_or(Error::<E>::NoSuchPid(peer_pid))?;

                    // stat the askpass executable that is being invoked
                    let peer_stat = self.exec.stat(&peer_path).await.map_err(Error::<E>::Exec)?;

                    if peer_stat.ino == askpath_stat.ino {
                        if peer_stat.dev != askpath_stat.dev {
                            return Err(Error::<E>::AskpassDeviceMismatch);
                        }
                    } else if peer_stat.ino == setsid_stat.ino {
                        if peer_stat.dev != setsid_stat.dev {
                            return Err(Error::<E>::AskpassDeviceMismatch);
                        }
                    } else {
                        return Err(Error::<E>::AskpassExecutableMismatch);
                    }

                    // await for peer to send secret
                    let peer_secret = sock.read_line().await.map_err(Error::<E>::AskpassIo)?;

                    // check the secret
                    if peer_secret.trim() != secret {
                        return Err(Error::<E>::AskpassSecretMismatch);
                    }

                    // get the prompt
                    let prompt = sock.read_line().await.map_err(Error::<E>::AskpassIo)?;

                    match authorization {
                        Authorization::Auto => {
                            return Err(Error::<E>::NeedsAuthorization(prompt));
                        }
                        Authorization::Basic{username, password} => {
                            if prompt.contains("Username for") {
                                sock.write_line(username).await.map_err(Error::<E>::AskpassIo)?;
                            } else if prompt.contains("Password for") {
                                sock.write_line(password).await.map_err(Error::<E>::AskpassIo)?;
                            } else {
                                return Err(Error::<E>::NeedsAuthorization(prompt));
                            }
                        },
                        Authorization::Ssh { passphrase, .. } => {
                            if let Some(passphrase) = passphrase {
                            if prompt.contains("passphrase for key") {
                                sock.write_line(passphrase).await.map_err(Error::<E>::AskpassIo)?;
                                continue;
                            }
                            }

                            return Err(Error::<E>::NeedsAuthorization(prompt));
                        }
                    }
                },
                res = child_process => {
                    return res;
                }
            }
        }
    }
}

impl<E: GitExecutor + 'static> crate::Repository for Repository<E> {
    type Error = Error<E>;

    async fn config_get(
        &self,
        key: &str,
        scope: ConfigScope,
    ) -> Result<Option<String>, Self::Error> {
        let mut args = vec!["-C", &self.path, "config", "--get"];

        // NOTE(qix-): See source comments for ConfigScope to explain
        // NOTE(qix-): the `#[cfg(not(test))]` attributes.
        match scope {
            #[cfg(not(test))]
            ConfigScope::Auto => {}
            ConfigScope::Local => args.push("--local"),
            #[cfg(not(test))]
            ConfigScope::System => args.push("--system"),
            #[cfg(not(test))]
            ConfigScope::Global => args.push("--global"),
        }

        args.push(key);

        let (exit_code, stdout, stderr) = self
            .exec
            .execute(&args, None)
            .await
            .map_err(Error::<E>::Exec)?;

        if exit_code == 0 {
            Ok(Some(stdout))
        } else if exit_code == 1 && stderr.is_empty() {
            Ok(None)
        } else {
            Err(Error::<E>::Failed(
                exit_code,
                args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            ))
        }
    }

    async fn config_set(
        &self,
        key: &str,
        value: &str,
        scope: ConfigScope,
    ) -> Result<(), Self::Error> {
        let mut args = vec!["-C", &self.path, "config", "--replace-all"];

        // NOTE(qix-): See source comments for ConfigScope to explain
        // NOTE(qix-): the `#[cfg(not(test))]` attributes.
        match scope {
            #[cfg(not(test))]
            ConfigScope::Auto => {}
            ConfigScope::Local => args.push("--local"),
            #[cfg(not(test))]
            ConfigScope::System => args.push("--system"),
            #[cfg(not(test))]
            ConfigScope::Global => args.push("--global"),
        }

        args.push(key);
        args.push(value);

        let (exit_code, stdout, stderr) = self
            .exec
            .execute(&args, None)
            .await
            .map_err(Error::<E>::Exec)?;

        if exit_code == 0 {
            Ok(())
        } else {
            Err(Error::<E>::Failed(
                exit_code,
                args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            ))
        }
    }
}
