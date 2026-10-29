//! Explicit executable setup for gitoxide's native credential cache.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, atomic::AtomicBool},
};

use anyhow::{Context as _, Result, ensure};
use gix::credentials::program::cache::{Daemon, DaemonCommand};

static DAEMON: OnceLock<DaemonCommand> = OnceLock::new();

/// Private executable entry point used to start a credential-cache daemon.
pub const DAEMON_ARGUMENT: &str = "--credential-cache-daemon";

/// Select the credential-cache executable before opening repositories or using secrets.
///
/// Executables can select themselves; library hosts must provide their packaged helper.
/// Repeating the same registration is permitted, but changing it requires a new process.
pub fn set_daemon(command: DaemonCommand) -> Result<()> {
    ensure!(
        command.executable.is_absolute(),
        "credential-cache executable must be absolute"
    );
    let installed = DAEMON.get_or_init(|| command.clone());
    ensure!(
        installed.executable == command.executable && installed.args == command.args,
        "credential-cache executable was already configured differently"
    );
    Ok(())
}

/// Return the launcher selected by the executable or library host, if any.
pub fn daemon_command() -> Option<DaemonCommand> {
    DAEMON.get().cloned()
}

/// Initialize a dedicated executable before starting its runtime, logging or UI.
///
/// Pass its absolute path and complete argument list. If invoked as a cache daemon, this
/// serves requests until shutdown and returns `true`; the caller must then return from `main`.
/// Otherwise, registers the executable for future cache requests and returns `false`.
/// Library hosts must call [`set_daemon()`] with a separate executable instead.
pub fn initialize(executable: PathBuf, args: impl IntoIterator<Item = OsString>) -> Result<bool> {
    let mut args = args.into_iter().skip(1);
    if args.next().as_deref() == Some(OsStr::new(DAEMON_ARGUMENT)) {
        let socket = args
            .next()
            .context("credential-cache daemon requires a socket path")?;
        ensure!(
            args.next().is_none(),
            "credential-cache daemon accepts only a socket path"
        );
        run_daemon(Path::new(&socket))?;
        return Ok(true);
    }
    set_daemon(DaemonCommand {
        executable,
        args: vec![DAEMON_ARGUMENT.into()],
    })?;
    Ok(false)
}

fn run_daemon(socket: &Path) -> Result<()> {
    // Like Git's daemon, use global configuration, independent of the caller's repository.
    let config = gix::config(None, &gix::open::Options::default())?;
    let ignore_sighup = config
        .boolean("credentialCache.ignoreSighup")?
        .unwrap_or(false);
    let interrupt = Arc::new(AtomicBool::new(false));
    let mut signals = Vec::new();
    for signal in signal_hook::consts::TERM_SIGNALS {
        signals.push(signal_hook::flag::register(
            *signal,
            Arc::clone(&interrupt),
        )?);
    }
    signals.push(signal_hook::flag::register(
        signal_hook::consts::SIGHUP,
        if ignore_sighup {
            Arc::new(AtomicBool::new(false))
        } else {
            Arc::clone(&interrupt)
        },
    )?);
    let result = Daemon::bind(socket).and_then(|daemon| daemon.run_with_stdio(false, &interrupt));
    for signal in signals {
        signal_hook::low_level::unregister(signal);
    }
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_requires_an_explicit_stable_executable() -> Result<()> {
        assert!(
            daemon_command().is_none(),
            "libraries do not select a host executable"
        );
        assert!(
            set_daemon(DaemonCommand {
                executable: "relative-helper".into(),
                args: Vec::new(),
            })
            .is_err(),
            "a changing working directory must not change which executable runs"
        );
        let command = DaemonCommand {
            executable: "/explicit helper".into(),
            args: vec![DAEMON_ARGUMENT.into()],
        };
        set_daemon(command.clone())?;
        set_daemon(command.clone())?;
        assert!(
            set_daemon(DaemonCommand {
                executable: "/different helper".into(),
                ..command.clone()
            })
            .is_err(),
            "later initialization cannot silently replace the selected executable"
        );
        assert_eq!(
            daemon_command().expect("registered launcher").executable,
            command.executable,
            "rejected registration preserves the launcher"
        );
        Ok(())
    }
}
