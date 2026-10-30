//! What a terminal the app opens starts from: the user's shell on a pseudo-terminal, and the
//! colours and size their Ghostty config sets.

mod ghostty;
mod pty;
mod vt;

use std::path::Path;

use futures::channel::mpsc;

pub use ghostty::{Colors, Ghostty};
#[cfg(unix)]
pub use pty::Kept;
pub use pty::{Pty, Size};
pub(crate) use vt::Vt;
pub use vt::{Cursor, Style};

/// A shell just started: where to send it input, what it writes, and how to show it.
pub struct Opened {
    pub pty: Pty,
    pub output: mpsc::UnboundedReceiver<Vec<u8>>,
    pub ghostty: Ghostty,
    pub size: Size,
}

/// Start the user's shell in `cwd`, sized `size`, reading their Ghostty config alongside.
pub fn open(cwd: &Path, size: Size) -> anyhow::Result<Opened> {
    let (pty, output) = Pty::spawn(cwd, size)?;
    Ok(Opened {
        pty,
        output,
        ghostty: Ghostty::load(),
        size,
    })
}

/// Takes over a shell the process this one was before exec kept, sized as it was.
#[cfg(unix)]
pub fn adopt(kept: Kept, size: Size) -> anyhow::Result<Opened> {
    let (pty, output) = Pty::adopt(kept)?;
    Ok(Opened {
        pty,
        output,
        ghostty: Ghostty::load(),
        size,
    })
}

#[cfg(unix)]
pub use handoff::{Handed, hand_over, handed};

/// Terminals handed to the build exec runs in this process's place. Exec keeps the process,
/// so the shells stay its children, and the masters it's told to keep open.
#[cfg(unix)]
mod handoff {
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read as _, Seek as _, SeekFrom};
    use std::os::fd::{FromRawFd as _, IntoRawFd as _, RawFd};
    use std::os::unix::fs::OpenOptionsExt as _;
    use std::process::{self, Command};

    use super::{Kept, Size};
    use crate::backend::Checkout;

    /// `pid:fd`, the descriptor of an unlinked file listing them, so nothing is left on disk.
    /// Exec keeps the pid; another process given the variable finds it isn't its own.
    pub(in crate::backend) const HANDED: &str = "GITBUTLER_GPUI_TERMINALS";

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Written {
        handed: Handed,
        master: RawFd,
        shell: libc::pid_t,
    }

    /// Where a terminal was open, and what it showed.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Handed {
        pub project: String,
        pub checkout: Checkout,
        pub id: u64,
        pub size: Size,
        pub screen: String,
    }

    /// Lists `terminals` for the build `new` runs by exec, returning the descriptors to keep
    /// open across it.
    pub fn hand_over(
        terminals: Vec<(Handed, Kept)>,
        new: &mut Command,
    ) -> anyhow::Result<Vec<RawFd>> {
        new.env_remove(HANDED);
        let terminals: Vec<Written> = (terminals.into_iter())
            .map(|(handed, kept)| {
                let (master, shell) = kept.into_raw();
                Written {
                    handed,
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

    /// The terminals the process this one was before exec handed over, with their shells.
    pub fn handed() -> Vec<(Handed, Kept)> {
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
            .and_then(|_| Ok(serde_json::from_slice::<Vec<Written>>(&read)?));
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
                (kept.inspect_err(|err| tracing::error!("couldn't adopt a terminal: {err}")))
                    .ok()
                    .map(|kept| (written.handed, kept))
            })
            .collect()
    }
}
