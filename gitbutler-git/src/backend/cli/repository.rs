use super::executor::GitExecutor;
use crate::{prelude::*, ConfigScope};
use rand::Rng;

/// The number of characters in the secret used for checking
/// askpass invocations by ssh/git when connecting to our process.
const ASKPASS_SECRET_LENGTH: usize = 24;

/// Higher level errors that can occur when interacting with the CLI.
#[derive(Debug, thiserror::Error)]
pub enum Error<E: core::error::Error + core::fmt::Debug + Send + Sync + 'static> {
    #[error("failed to execute git command: {0}")]
    Exec(E),
    #[error(
        "git command exited with non-zero exit code {0}: {1:?}\n\nSTDOUT:\n{2}\n\nSTDERR:\n{3}"
    )]
    Failed(usize, Vec<String>, String, String),
    #[error("failed to determine path to this executable: {0}")]
    NoSelfExe(std::io::Error),
}

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
    pub async fn open_or_init<P: AsRef<str>>(exec: E, path: P) -> Result<Self, Error<E::Error>> {
        let path = path.as_ref().to_owned();
        let args = vec!["init", "--quiet", &path];

        let (exit_code, stdout, stderr) = exec.execute(&args, None).await.map_err(Error::Exec)?;

        if exit_code == 0 {
            Ok(Self { exec, path })
        } else {
            Err(Error::Failed(
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
    ) -> Result<(usize, String, String), Error<E::Error>> {
        let path = std::env::current_exe().map_err(|e| Error::NoSelfExe(e))?;
        let our_pid = std::process::id();

        let askpath_path = path.with_file_name("gitbutler-git-askpass");
        #[cfg(not(target_os = "windows"))]
        let setsid_path = path.with_file_name("gitbutler-git-setsid");

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
        envs.insert(
            "SSH_ASKPASS".into(),
            askpath_path.to_string_lossy().into_owned(),
        );

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
                setsid_path.to_string_lossy(),
                envs.get("GIT_SSH_COMMAND").unwrap_or(&"ssh".into())
            ),
        );

        // TODO(qix-): implement the actual socket server code (right now this won't work)

        self.exec
            .execute(args, Some(envs))
            .await
            .map_err(Error::Exec)
    }
}

impl<E: GitExecutor + 'static> crate::Repository for Repository<E> {
    type Error = Error<E::Error>;

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

        let (exit_code, stdout, stderr) =
            self.exec.execute(&args, None).await.map_err(Error::Exec)?;

        if exit_code == 0 {
            Ok(Some(stdout))
        } else if exit_code == 1 && stderr.is_empty() {
            Ok(None)
        } else {
            Err(Error::Failed(
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

        let (exit_code, stdout, stderr) =
            self.exec.execute(&args, None).await.map_err(Error::Exec)?;

        if exit_code == 0 {
            Ok(())
        } else {
            Err(Error::Failed(
                exit_code,
                args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            ))
        }
    }
}
