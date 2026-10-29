//! The user's login shell on a pseudo-terminal, with what it writes read on its own thread.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ptr, sync, thread};

use anyhow::Context as _;
use futures::channel::mpsc;

/// A shell running on a pseudo-terminal, hung up when dropped.
pub struct Pty {
    master: File,
    /// What to send the shell, written on a thread of its own, as a shell that isn't reading
    /// leaves a write waiting.
    input: sync::mpsc::Sender<Vec<u8>>,
    /// The shell's process group, which `setsid` made it lead.
    group: libc::pid_t,
    exited: Arc<AtomicBool>,
}

/// A grid size, with the pixel size of one cell for programs that ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl Size {
    fn winsize(self) -> libc::winsize {
        libc::winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: self.cols.saturating_mul(self.cell_width),
            ws_ypixel: self.rows.saturating_mul(self.cell_height),
        }
    }
}

impl Pty {
    /// Starts the user's shell ($SHELL, else /bin/zsh on macOS and /bin/sh elsewhere) as a login
    /// shell in `cwd`, sized `size`. The receiver yields what the shell writes, in chunks, and
    /// ends once it exits, though jobs it left still hold the terminal.
    pub fn spawn(
        cwd: &Path,
        size: Size,
    ) -> anyhow::Result<(Pty, mpsc::UnboundedReceiver<Vec<u8>>)> {
        let (master, slave) = open(size).context("Couldn't open a pseudo-terminal")?;
        let shell = shell();
        let child = shell_command(&shell, cwd, &slave)?
            .spawn()
            .with_context(|| format!("Couldn't start {}", shell.display()))?;
        drop(slave);

        let master = File::from(master);
        let group = child.id() as libc::pid_t;
        let (sender, receiver) = mpsc::unbounded();
        let reader = master.try_clone()?;
        let ended = sender.clone();
        thread::Builder::new()
            .name("pty reader".into())
            .spawn(move || read_until_hangup(reader, sender))?;

        let exited = Arc::new(AtomicBool::new(false));
        let waited = exited.clone();
        let mut child = child;
        thread::Builder::new()
            .name("pty waiter".into())
            .spawn(move || {
                child.wait().ok();
                waited.store(true, Ordering::SeqCst);
                ended.close_channel();
            })?;

        let (input, queued) = sync::mpsc::channel::<Vec<u8>>();
        let writer = master.try_clone()?;
        thread::Builder::new()
            .name("pty writer".into())
            .spawn(move || {
                for bytes in queued {
                    if (&writer).write_all(&bytes).is_err() {
                        return;
                    }
                }
            })?;

        let pty = Pty {
            master,
            input,
            group,
            exited,
        };
        Ok((pty, receiver))
    }

    /// Sends input to the shell, once it reads what was sent before.
    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        (self.input.send(bytes.to_vec())).map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    pub fn resize(&self, size: Size) -> io::Result<()> {
        let winsize = size.winsize();
        // SAFETY: the master is open while `self` is, and `winsize` outlives the call.
        check(unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) })?;
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // The waiter reaps the shell once it has exited, and the group's id is free again then.
        if self.exited.load(Ordering::SeqCst) {
            return;
        }
        // As a terminal hanging up does: the job in front, which the shell runs in a group of
        // its own, then the shell.
        // SAFETY: the master is open while `self` is.
        let front = unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) };
        for group in [front, self.group] {
            if group > 0 {
                // SAFETY: a plain signal to a group on this terminal, whose leader isn't reaped.
                unsafe { libc::killpg(group, libc::SIGHUP) };
            }
        }
    }
}

fn open(size: Size) -> io::Result<(OwnedFd, OwnedFd)> {
    let (mut master, mut slave) = (-1, -1);
    let mut winsize = size.winsize();
    // SAFETY: each pointer is to a live local or null, which `openpty` takes as unset.
    check(unsafe {
        libc::openpty(
            &raw mut master,
            &raw mut slave,
            ptr::null_mut(),
            ptr::null_mut::<libc::termios>(),
            &raw mut winsize,
        )
    })?;
    // SAFETY: `openpty` succeeded, so both are open descriptors nothing else owns.
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    set_cloexec(&master)?;
    set_cloexec(&slave)?;
    Ok((master, slave))
}

fn shell() -> OsString {
    std::env::var_os("SHELL")
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "/bin/zsh"
            } else {
                "/bin/sh"
            }
            .into()
        })
}

fn shell_command(shell: &OsString, cwd: &Path, slave: &OwnedFd) -> io::Result<Command> {
    let mut command = Command::new(shell);
    command
        .arg("-l")
        .current_dir(cwd)
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env("TERM_PROGRAM", "GitButler")
        .env_remove("TERMINFO")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?));
    // SAFETY: between fork and exec it makes only async-signal-safe calls, and allocates
    // nothing.
    unsafe {
        command.pre_exec(|| {
            check(libc::setsid())?;
            check(libc::ioctl(0, libc::TIOCSCTTY as _, 0))?;
            Ok(())
        });
    }
    Ok(command)
}

fn read_until_hangup(mut master: File, sender: mpsc::UnboundedSender<Vec<u8>>) {
    let mut buffer = vec![0; 64 * 1024];
    loop {
        match master.read(&mut buffer) {
            Ok(0) => return,
            Ok(n) => {
                let read = buffer.get(..n).unwrap_or_default().to_vec();
                if sender.unbounded_send(read).is_err() {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

fn set_cloexec(fd: &OwnedFd) -> io::Result<()> {
    // SAFETY: `fd` is open for as long as it's borrowed.
    let flags = check(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) })?;
    // SAFETY: as above.
    check(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) })?;
    Ok(())
}

fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}
