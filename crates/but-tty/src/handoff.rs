//! Terminals handed to the build that takes this process's place: one it starts, which takes the
//! masters along and says once it has them, so this one can exit, leaving the shells to it. An
//! older build hands them over by exec, which keeps the process, so the shells stay its children.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _, IntoRawFd as _, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt as _;
use std::process::{self, Command};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{Detached, Kept, Size, Status};

/// `pid:fd:taken`: the process handing them over, the descriptor of an unlinked file listing
/// them, so nothing is left on disk, and the write end of a pipe to say they were taken on,
/// which a build from before has none of. Exec keeps the pid, and a build started to take over
/// has it as its parent; another process given the variable finds it's neither.
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
    /// Absent from a build from before terminals could run a command.
    #[serde(default)]
    runs_command: bool,
}

/// Terminals listed for the build about to take over, which this process waits for.
pub struct Handing {
    /// What the build takes along, to stay open as it starts.
    pub keep: Vec<RawFd>,
    /// The pipe the build says it took them on: a byte, or the pipe's end once it exited
    /// without.
    taken: File,
    /// The pipe's other end, which the build takes along, closed here once it has.
    taken_by: OwnedFd,
}

impl Handing {
    /// Waits up to `within` for the build started with them to take the terminals, saying
    /// whether it did.
    pub fn wait_taken(self, within: Duration) -> bool {
        let Handing {
            mut taken,
            taken_by,
            ..
        } = self;
        // The build has its own now, so the pipe ends once the build exits.
        drop(taken_by);
        let mut ready = libc::pollfd {
            fd: taken.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let within = within.as_millis().try_into().unwrap_or(libc::c_int::MAX);
        // SAFETY: `ready` is a live local naming a descriptor open while `taken` is.
        if unsafe { libc::poll(&mut ready, 1, within) } != 1 {
            return false;
        }
        matches!(taken.read(&mut [0]), Ok(1))
    }
}

/// Lists `terminals` for the build `new` starts, which takes them over, returning what to keep
/// open for it and wait on.
pub fn hand_over<T: Serialize>(
    terminals: Vec<(T, Detached)>,
    new: &mut Command,
) -> anyhow::Result<Handing> {
    new.env_remove(HANDED);
    let terminals: Vec<Written<T>> = (terminals.into_iter())
        .map(|(label, detached)| {
            let (master, shell, runs_command) = detached.kept.into_raw();
            Written {
                label,
                size: detached.size,
                screen: detached.screen,
                status: detached.status,
                master,
                shell,
                runs_command,
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
    let mut ends = [0; 2];
    // SAFETY: `ends` is a live local with room for both ends.
    if unsafe { libc::pipe(ends.as_mut_ptr()) } == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    let [read, write] = ends;
    // SAFETY: `pipe` just opened both, and nothing else owns them.
    let (taken, taken_by) = unsafe { (File::from_raw_fd(read), OwnedFd::from_raw_fd(write)) };
    keep.push(write);
    new.env(HANDED, format!("{}:{fd}:{write}", process::id()));
    Ok(Handing {
        keep,
        taken,
        taken_by,
    })
}

/// What [`HANDED`] says for this process.
#[derive(Debug, PartialEq)]
struct Listing {
    /// The file listing the terminals.
    file: RawFd,
    /// The pipe to say they were taken on, when the process handing them over has one.
    taken: Option<RawFd>,
    /// The shells are this process's children, as exec keeps them, rather than its parent's.
    children: bool,
}

/// What `handed`, as [`HANDED`] says it, lists for process `own`, whose parent is `parent`: none
/// when it's another's.
fn listing(handed: &str, own: u32, parent: u32) -> Option<Listing> {
    let mut parts = handed.split(':');
    let pid: u32 = parts.next()?.parse().ok()?;
    let file = parts.next()?.parse().ok()?;
    let taken = parts.next().map(str::parse).transpose().ok()?;
    let children = match pid {
        pid if pid == own => true,
        pid if pid == parent => false,
        _ => return None,
    };
    Some(Listing {
        file,
        taken,
        children,
    })
}

/// The terminals the process this one takes over from handed over, with their labels, saying
/// on the pipe it waits on, if there is one, that they were taken.
pub fn handed<T: DeserializeOwned>() -> Vec<(T, Detached)> {
    // SAFETY: `getppid` only reads this process's parent.
    let parent = u32::try_from(unsafe { libc::getppid() }).unwrap_or(0);
    let listed = std::env::var(HANDED)
        .ok()
        .and_then(|handed| listing(&handed, process::id(), parent));
    let Some(listed) = listed else {
        return Vec::new();
    };
    let terminals = adopt(&listed);
    if let Some(taken) = listed.taken {
        // SAFETY: as the file's, by the process waiting on the other end.
        let mut taken = unsafe { File::from_raw_fd(taken) };
        taken.write_all(&[1]).ok();
    }
    terminals
}

/// The terminals `listed` lists, taken over.
fn adopt<T: DeserializeOwned>(listed: &Listing) -> Vec<(T, Detached)> {
    // SAFETY: the process handing them over kept it open for this, and nothing else took it
    // since.
    let mut file = unsafe { File::from_raw_fd(listed.file) };
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
            let kept = unsafe {
                Kept::from_raw(
                    written.master,
                    written.shell,
                    written.runs_command,
                    listed.children,
                )
            };
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

#[cfg(test)]
mod tests {
    use std::os::fd::RawFd;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{Listing, listing};
    use crate::{Detached, Pty, Size, Status};

    /// Set for the test binary started to take a terminal over.
    const TAKER: &str = "BUT_TTY_TEST_TAKER";

    const SIZE: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };

    #[test]
    fn a_build_started_to_take_over_gets_a_shell_that_answers_it() {
        if std::env::var_os(TAKER).is_some() {
            return take_over();
        }
        let (mut pty, _output) = Pty::spawn(&std::env::temp_dir(), SIZE, None).unwrap();
        let kept = pty.keep().unwrap();
        drop(pty);
        let detached = Detached {
            kept,
            size: SIZE,
            screen: String::new(),
            status: Status::default(),
        };
        let mut taker = Command::new(std::env::current_exe().unwrap());
        taker
            .args([
                "--exact",
                "handoff::tests::a_build_started_to_take_over_gets_a_shell_that_answers_it",
                "--nocapture",
            ])
            .env(TAKER, "1")
            .stdout(Stdio::piped());
        let handing = super::hand_over(vec![("main".to_owned(), detached)], &mut taker).unwrap();
        for fd in &handing.keep {
            inherited(*fd);
        }
        let taker = taker.spawn().unwrap();
        assert!(
            handing.wait_taken(Duration::from_secs(20)),
            "the build started took the terminals over and said so"
        );
        let out = taker.wait_with_output().unwrap();
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("answered main"),
            "the shell answers the build that took it over: {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    /// As the build started to take over: takes the terminal and has its shell answer.
    fn take_over() {
        let (label, detached) = super::handed::<String>()
            .pop()
            .expect("one was handed over");
        let (pty, mut output) = Pty::adopt(detached.kept).unwrap();
        pty.write(b"echo taken-$((40+2))\n").unwrap();
        let started = Instant::now();
        let mut read = Vec::new();
        while !String::from_utf8_lossy(&read).contains("taken-42") {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "the shell answered"
            );
            read.extend(output.waiting());
            std::thread::sleep(Duration::from_millis(20));
        }
        println!("answered {label}");
    }

    /// Leaves `fd` open in the process this one starts, as the app's restart does.
    fn inherited(fd: RawFd) {
        // SAFETY: `fd` is open, and this only clears its close-on-exec flag.
        unsafe { libc::fcntl(fd, libc::F_SETFD, 0) };
    }

    const OWN: u32 = 400;
    const PARENT: u32 = 300;

    #[test]
    fn a_build_started_to_take_over_takes_its_parents_terminals() {
        assert_eq!(
            listing("300:7:9", OWN, PARENT),
            Some(Listing {
                file: 7,
                taken: Some(9),
                children: false
            }),
            "its parent's shells aren't its children, and the parent waits on the pipe"
        );
    }

    #[test]
    fn exec_hands_the_terminals_to_the_process_itself() {
        assert_eq!(
            listing("400:7", OWN, PARENT),
            Some(Listing {
                file: 7,
                taken: None,
                children: true
            }),
            "a build from before execs, keeping the pid and the shells as children, with no pipe"
        );
    }

    #[test]
    fn another_process_given_the_variable_takes_nothing() {
        assert_eq!(
            listing("200:7:9", OWN, PARENT),
            None,
            "a process started in a terminal inherits it, but the terminals aren't its"
        );
    }

    #[test]
    fn a_garbled_variable_takes_nothing() {
        assert_eq!(
            listing("300:seven", OWN, PARENT),
            None,
            "no descriptor to read"
        );
    }
}
