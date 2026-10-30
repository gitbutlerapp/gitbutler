//! Terminals handed to the build exec runs in this process's place. Exec keeps the process,
//! so the shells stay its children, and the masters it's told to keep open.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::fd::{FromRawFd as _, IntoRawFd as _, RawFd};
use std::os::unix::fs::OpenOptionsExt as _;
use std::process::{self, Command};

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{Detached, Kept, Size, Status};

/// `pid:fd`, the descriptor of an unlinked file listing them, so nothing is left on disk.
/// Exec keeps the pid; another process given the variable finds it isn't its own.
pub(super) const HANDED: &str = "GITBUTLER_GPUI_TERMINALS";

/// A terminal as listed, with `label` saying where the app showed it.
#[derive(serde::Serialize, serde::Deserialize)]
struct Written<T> {
    label: T,
    size: Size,
    screen: String,
    /// Absent from a build from before terminals had one.
    #[serde(default)]
    status: Status,
    master: RawFd,
    shell: libc::pid_t,
}

/// Lists `terminals` for the build `new` runs by exec, returning the descriptors to keep open
/// across it.
pub fn hand_over<T: Serialize>(
    terminals: Vec<(T, Detached)>,
    new: &mut Command,
) -> anyhow::Result<Vec<RawFd>> {
    new.env_remove(HANDED);
    let terminals: Vec<Written<T>> = (terminals.into_iter())
        .map(|(label, detached)| {
            let (master, shell) = detached.kept.into_raw();
            Written {
                label,
                size: detached.size,
                screen: detached.screen,
                status: detached.status,
                master,
                shell,
            }
        })
        .collect();
    let mut keep: Vec<RawFd> = terminals.iter().map(|written| written.master).collect();
    let path = std::env::temp_dir().join(format!("gitbutler-gpui-{}", process::id()));
    fs::remove_file(&path).ok();
    let mut file = (OpenOptions::new().read(true).write(true))
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    fs::remove_file(&path)?;
    serde_json::to_writer(&mut file, &terminals)?;
    file.seek(SeekFrom::Start(0))?;
    let fd = file.into_raw_fd();
    keep.push(fd);
    new.env(HANDED, format!("{}:{fd}", process::id()));
    Ok(keep)
}

/// The terminals the process this one was before exec handed over, with their labels.
pub fn handed<T: DeserializeOwned>() -> Vec<(T, Detached)> {
    let ours = std::env::var(HANDED).ok().and_then(|handed| {
        let (pid, fd) = handed.split_once(':')?;
        (pid.parse() == Ok(process::id())).then(|| fd.parse().ok())?
    });
    let Some(fd) = ours else {
        return Vec::new();
    };
    // SAFETY: the process this one was before exec kept it open for this, and nothing else
    // took it since.
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut read = Vec::new();
    let terminals = (file.read_to_end(&mut read).map_err(anyhow::Error::from))
        .and_then(|_| Ok(serde_json::from_slice::<Vec<Written<T>>>(&read)?));
    let terminals = match terminals {
        Ok(terminals) => terminals,
        Err(err) => {
            tracing::error!("couldn't read the terminals handed over: {err:#}");
            return Vec::new();
        }
    };
    (terminals.into_iter())
        .filter_map(|written| {
            // SAFETY: as the file's, and each is listed once.
            let kept = unsafe { Kept::from_raw(written.master, written.shell) };
            let kept = (kept
                .inspect_err(|err| tracing::error!("couldn't adopt a terminal: {err}")))
            .ok()?;
            let detached = Detached {
                kept,
                size: written.size,
                screen: written.screen,
                status: written.status,
            };
            Some((written.label, detached))
        })
        .collect()
}
