use super::executor::{AskpassServer, GitExecutor, Pid, Socket};
use crate::{Authorization, ConfigScope, RefSpec};
use futures::{select, FutureExt};
use rand::Rng;
use std::{collections::HashMap, time::Duration};

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
        "git command exited with non-zero exit code {status}: {args:?}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}"
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
    #[cold]
    pub async fn open_or_init<P: AsRef<str>>(exec: E, path: P) -> Result<Self, Error<E>> {
        let path = path.as_ref().to_owned();
        let args = vec!["init", "--quiet", &path];

        let (exit_code, stdout, stderr) =
            exec.execute(&args, None).await.map_err(Error::<E>::Exec)?;

        if exit_code == 0 {
            Ok(Self { exec, path })
        } else {
            Err(Error::<E>::Failed {
                status: exit_code,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })
        }
    }

    /// (Re-)initializes a bare repository at the given path
    /// using the given [`GitExecutor`].
    #[cold]
    pub async fn open_or_init_bare<P: AsRef<str>>(exec: E, path: P) -> Result<Self, Error<E>> {
        let path = path.as_ref().to_owned();
        let args = vec!["init", "--bare", "--quiet", &path];

        let (exit_code, stdout, stderr) =
            exec.execute(&args, None).await.map_err(Error::<E>::Exec)?;

        if exit_code == 0 {
            Ok(Self { exec, path })
        } else {
            Err(Error::<E>::Failed {
                status: exit_code,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })
        }
    }

    #[cold]
    async fn execute_with_auth_harness(
        &self,
        args: &[&str],
        envs: Option<HashMap<String, String>>,
        authorization: &Authorization,
    ) -> Result<(usize, String, String), Error<E>> {
        let path = std::env::current_exe().map_err(Error::<E>::NoSelfExe)?;

        // TODO(qix-): Get parent PID of connecting processes to make sure they're us.
        //let our_pid = std::process::id();

        // TODO(qix-): This is a bit of a hack. Under a test environment,
        // TODO(qix-): Cargo is running a test runner with a quasi-random
        // TODO(qix-): suffix. The actual executables live in the parent directory.
        // TODO(qix-): Thus, we have to do this under test. It's not ideal, but
        // TODO(qix-): it works for now.
        #[cfg(test)]
        let path = path.parent().unwrap();

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

        #[allow(unsafe_code)]
        let sock_server = unsafe { self.exec.create_askpass_server() }
            .await
            .map_err(Error::<E>::Exec)?;

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
        envs.insert("GITBUTLER_ASKPASS_PIPE".into(), sock_server.to_string());
        envs.insert("GITBUTLER_ASKPASS_SECRET".into(), secret.clone());
        envs.insert("SSH_ASKPASS".into(), askpath_path);

        // DISPLAY is required by SSH to check SSH_ASKPASS.
        // Please don't ask us why, it's unclear.
        if !std::env::var("DISPLAY")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
        {
            envs.insert("DISPLAY".into(), ":".into());
        }

        envs.insert(
            "GIT_SSH_COMMAND".into(),
            format!(
                "{}{}{} -o StrictHostKeyChecking=accept-new -o KbdInteractiveAuthentication=no{}",
                {
                    #[cfg(not(target_os = "windows"))]
                    {
                        format!("{setsid_path} ")
                    }
                    #[cfg(target_os = "windows")]
                    {
                        ""
                    }
                },
                envs.get("GIT_SSH_COMMAND").unwrap_or(&"ssh".into()),
                match authorization {
                    Authorization::Ssh { .. } => " -o PreferredAuthentications=publickey",
                    Authorization::Basic { .. } => " -o PreferredAuthentications=password",
                    _ => "",
                },
                {
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
                }
            ),
        );

        if let Authorization::Ssh {
            private_key: Some(private_key),
            ..
        } = authorization
        {
            envs.insert("GIT_SSH_VARIANT".into(), "ssh".into());
            envs.insert("GIT_SSH_KEY".into(), private_key.clone());
        }

        let mut child_process = core::pin::pin! {
            async {
                self.exec
                    .execute(args, Some(envs))
                    .await
                    .map_err(Error::<E>::Exec)
            }.fuse()
        };

        loop {
            select! {
                res = child_process => {
                    return res;
                },
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
                            return Err(Error::<E>::AskpassDeviceMismatch)?;
                        }
                    } else if peer_stat.ino == setsid_stat.ino {
                        if peer_stat.dev != setsid_stat.dev {
                            return Err(Error::<E>::AskpassDeviceMismatch)?;
                        }
                    } else {
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

                    // TODO(qix-): The prompt matching logic here is fragile as the remote
                    // TODO(qix-): can customize prompts. I need to investigate if there's
                    // TODO(qix-): a better way to do this.
                    match authorization {
                        Authorization::Auto => {
                            return Err(Error::<E>::NeedsAuthorization(prompt))?;
                        }
                        Authorization::Basic{username, password} => {
                            if prompt.to_lowercase().contains("username:") || prompt.to_lowercase().contains("username for") {
                                if let Some(username) = username {
                                    sock.write_line(username).await.map_err(Error::<E>::AskpassIo)?;
                                } else {
                                    return Err(Error::<E>::NeedsAuthorization(prompt))?;
                                }
                            } else if prompt.to_lowercase().contains("password:") || prompt.to_lowercase().contains("password for") {
                                if let Some(password) = password {
                                    sock.write_line(password).await.map_err(Error::<E>::AskpassIo)?;
                                } else {
                                    return Err(Error::<E>::NeedsAuthorization(prompt))?;
                                }
                            } else {
                                return Err(Error::<E>::NeedsAuthorization(prompt))?;
                            }
                        },
                        Authorization::Ssh { passphrase, .. } => {
                            if let Some(passphrase) = passphrase {
                                if prompt.contains("passphrase for key") {
                                    sock.write_line(passphrase).await.map_err(Error::<E>::AskpassIo)?;
                                    continue;
                                }
                            }

                            return Err(Error::<E>::NeedsAuthorization(prompt))?;
                        }
                    }
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
    ) -> Result<Option<String>, crate::Error<Self::Error>> {
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
            Err(Error::<E>::Failed {
                status: exit_code,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })?
        }
    }

    async fn config_set(
        &self,
        key: &str,
        value: &str,
        scope: ConfigScope,
    ) -> Result<(), crate::Error<Self::Error>> {
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
            Err(Error::<E>::Failed {
                status: exit_code,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })?
        }
    }

    async fn fetch(
        &self,
        remote: &str,
        refspec: RefSpec,
        authorization: &Authorization,
    ) -> Result<(), crate::Error<Self::Error>> {
        let mut args = vec![
            "-C",
            &self.path,
            "fetch",
            "--quiet",
            "--no-write-fetch-head",
        ];

        let refspec = refspec.to_string();

        args.push(remote);
        args.push(&refspec);

        let (status, stdout, stderr) = self
            .execute_with_auth_harness(&args, None, authorization)
            .await?;

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

    async fn create_remote(
        &self,
        remote: &str,
        uri: &str,
    ) -> Result<(), crate::Error<Self::Error>> {
        let args = vec!["-C", &self.path, "remote", "add", remote, uri];

        let (status, stdout, stderr) = self
            .exec
            .execute(&args, None)
            .await
            .map_err(Error::<E>::Exec)?;

        if status != 0 {
            Err(Error::<E>::Failed {
                status,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })?
        } else {
            Ok(())
        }
    }

    async fn create_or_update_remote(
        &self,
        remote: &str,
        uri: &str,
    ) -> Result<(), crate::Error<Self::Error>> {
        let created = self
            .create_remote(remote, uri)
            .await
            .map(|_| true)
            .or_else(|e| match e {
                crate::Error::RemoteExists(..) => Ok(false),
                e => Err(e),
            })?;

        if created {
            return Ok(());
        }

        let args = vec!["-C", &self.path, "remote", "set-url", remote, uri];

        let (status, stdout, stderr) = self
            .exec
            .execute(&args, None)
            .await
            .map_err(Error::<E>::Exec)?;

        if status == 0 {
            Ok(())
        } else if status != 0 && stderr.to_lowercase().contains("error: no such remote") {
            self.create_remote(remote, uri).await
        } else {
            Err(Error::<E>::Failed {
                status,
                args: args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            })?
        }
    }

    async fn remote(&self, remote: &str) -> Result<String, crate::Error<Self::Error>> {
        let args = vec!["-C", &self.path, "remote", "get-url", remote];

        let (status, stdout, stderr) = self
            .exec
            .execute(&args, None)
            .await
            .map_err(Error::<E>::Exec)?;

        if status == 0 {
            Ok(stdout)
        } else if status != 0 && stderr.to_lowercase().contains("error: no such remote") {
            Err(crate::Error::NoSuchRemote(
                remote.to_owned(),
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
}
