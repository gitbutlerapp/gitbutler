//! The user's login shell on a pseudo-terminal, with what it writes read on its own thread.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ptr, sync, thread};

use super::{Output, PAUSE_MS, Queue, Size, queue};
use anyhow::Context as _;

/// A shell running on a pseudo-terminal, hung up when dropped unless it was kept.
pub struct Pty {
    /// Taken once the shell is kept.
    master: Option<File>,
    /// What to send the shell, written on a thread of its own, as a shell that isn't reading
    /// leaves a write waiting.
    input: sync::mpsc::Sender<Vec<u8>>,
    /// The shell, which `setsid` made lead its own process group.
    shell: libc::pid_t,
    /// The shell runs a command and exits rather than coming back to a prompt.
    runs_command: bool,
    exited: Arc<AtomicBool>,
    /// How the shell exited, as `$?` would say, once it has.
    exit_code: Arc<sync::OnceLock<i32>>,
    stop_reading: UnixStream,
    reader: Option<thread::JoinHandle<()>>,
}

/// A shell's pseudo-terminal, still open, with no one reading it. Dropped, it hangs up.
pub struct Kept {
    master: OwnedFd,
    shell: libc::pid_t,
    runs_command: bool,
}

impl Kept {
    /// Its master stays open, for an exec to take along, with the shell and whether it runs a
    /// command.
    pub fn into_raw(self) -> (RawFd, libc::pid_t, bool) {
        (self.master.into_raw_fd(), self.shell, self.runs_command)
    }

    /// # Safety
    ///
    /// `master` is open and nothing else owns it, as when [`Kept::into_raw`] gave it to the
    /// process this one was before exec.
    pub unsafe fn from_raw(
        master: RawFd,
        shell: libc::pid_t,
        runs_command: bool,
    ) -> io::Result<Self> {
        // SAFETY: as the caller promises.
        let master = unsafe { OwnedFd::from_raw_fd(master) };
        set_cloexec(&master)?;
        Ok(Self {
            master,
            shell,
            runs_command,
        })
    }
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
    /// shell in `cwd`, sized `size`, running `command` with `-c` when there is one, still
    /// interactive so it reads the same config as at a prompt. The receiver yields what the shell
    /// writes, in chunks, and ends once it exits, though jobs it left still hold the terminal.
    pub fn spawn(cwd: &Path, size: Size, command: Option<&str>) -> anyhow::Result<(Pty, Output)> {
        let (master, slave) = open(size).context("Couldn't open a pseudo-terminal")?;
        let shell = shell();
        let child = shell_command(&shell, command, cwd, &slave)?
            .spawn()
            .with_context(|| format!("Couldn't start {}", shell.display()))?;
        drop(slave);

        let shell = child.id() as libc::pid_t;
        Ok(Self::start(File::from(master), shell, command.is_some())?)
    }

    /// Takes over a shell kept by [`Pty::keep`], in this process or one it became by exec.
    pub fn adopt(kept: Kept) -> io::Result<(Pty, Output)> {
        Self::start(File::from(kept.master), kept.shell, kept.runs_command)
    }

    fn start(master: File, shell: libc::pid_t, runs_command: bool) -> io::Result<(Pty, Output)> {
        let (queue, output) = queue();
        let (stop_reading, stopped) = UnixStream::pair()?;
        let reader = master.try_clone()?;
        let ended = queue.clone();
        let reader = thread::Builder::new()
            .name("pty reader".into())
            .spawn(move || read_until_hangup(reader, stopped, queue))?;

        let exited = Arc::new(AtomicBool::new(false));
        let waited = exited.clone();
        let exit_code = Arc::new(sync::OnceLock::new());
        let code = exit_code.clone();
        thread::Builder::new()
            .name("pty waiter".into())
            .spawn(move || {
                if let Some(exit_code) = wait_for(shell) {
                    code.set(exit_code).ok();
                }
                waited.store(true, Ordering::SeqCst);
                ended.close();
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
            master: Some(master),
            input,
            shell,
            runs_command,
            exited,
            exit_code,
            stop_reading,
            reader: Some(reader),
        };
        Ok((pty, output))
    }

    /// Stops reading, leaving what the shell writes from now on to whoever adopts it. What was
    /// read before is all in the receiver once this returns, and nothing reaches the shell
    /// after.
    pub fn keep(&mut self) -> io::Result<Kept> {
        (&self.stop_reading).write_all(&[0])?;
        if let Some(reader) = self.reader.take() {
            reader.join().ok();
        }
        let master = self.master.take().ok_or(io::ErrorKind::NotFound)?;
        Ok(Kept {
            master: master.into(),
            shell: self.shell,
            runs_command: self.runs_command,
        })
    }

    /// Whether the shell has the terminal back at its prompt, rather than a job it's running.
    /// Never while it runs a command, which has no prompt to come back to: the shell runs a
    /// simple command in its own place, keeping its process group, and fish runs every one in
    /// that group.
    pub fn shell_in_front(&self) -> bool {
        !self.runs_command
            && (self.master.as_ref())
                // SAFETY: `master` is open while it's borrowed.
                .is_some_and(|master| unsafe { libc::tcgetpgrp(master.as_raw_fd()) } == self.shell)
    }

    /// The exit code of the command the shell ran, once it exited with one other than 0. Never
    /// for a shell at a prompt, as the user typed what made it exit.
    pub fn failed(&self) -> Option<i32> {
        let code = *self.exit_code.get()?;
        (self.runs_command && code != 0).then_some(code)
    }

    /// Sends input to the shell, once it reads what was sent before.
    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        if self.master.is_none() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        (self.input.send(bytes.to_vec())).map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    pub fn resize(&self, size: Size) -> io::Result<()> {
        let master = self.master.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let winsize = size.winsize();
        // SAFETY: `master` is open while it's borrowed, and `winsize` outlives the call.
        check(unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) })?;
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // The waiter reaps the shell once it has exited, and the group's id is free again then.
        let Some(master) = &self.master else {
            return;
        };
        if self.exited.load(Ordering::SeqCst) {
            return;
        }
        // As a terminal hanging up does: the job in front, which the shell runs in a group of
        // its own, then the shell.
        // SAFETY: `master` is open while it's borrowed.
        let front = unsafe { libc::tcgetpgrp(master.as_raw_fd()) };
        for group in [front, self.shell] {
            if group > 0 {
                // SAFETY: a plain signal to a group on this terminal, whose leader isn't reaped.
                unsafe { libc::killpg(group, libc::SIGHUP) };
            }
        }
    }
}

fn open(size: Size) -> io::Result<(OwnedFd, OwnedFd)> {
    // One at a time: on macOS, `openpty` fails now and then with os error -6 while another
    // thread opens one too.
    static OPENING: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _opening = OPENING.lock();
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

fn shell_command(
    shell: &OsString,
    run: Option<&str>,
    cwd: &Path,
    slave: &OwnedFd,
) -> io::Result<Command> {
    let mut command = Command::new(shell);
    command.arg("-l");
    if let Some(run) = run {
        // `-c` alone makes it non-interactive, and zsh skips `.zshrc`, where aliases and
        // functions a command may use are set up.
        command.arg("-i").arg("-c").arg(run);
    }
    command
        .current_dir(cwd)
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        // Ghostty's, the emulator's, as Claude Code reports progress only to terminals it knows.
        .env("TERM_PROGRAM", "ghostty")
        .env("TERM_PROGRAM_VERSION", "1.3.2")
        .env_remove("TERMINFO")
        .env_remove(crate::handoff::HANDED)
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

fn read_until_hangup(mut master: File, stopped: UnixStream, queue: Queue) {
    let mut buffer = vec![0; 64 * 1024];
    let mut ready = [stopped.as_raw_fd(), master.as_raw_fd()].map(|fd| libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    });
    loop {
        let full = queue.full();
        let (watched, timeout) = match full {
            true => (1, libc::c_int::from(PAUSE_MS)),
            false => (2, -1),
        };
        // SAFETY: `ready` is two live entries, for descriptors open throughout.
        match check(unsafe { libc::poll(ready.as_mut_ptr(), watched, timeout) }) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        }
        if ready[0].revents != 0 {
            return;
        }
        if full {
            continue;
        }
        match master.read(&mut buffer) {
            Ok(0) => return,
            Ok(n) => {
                let read = buffer.get(..n).unwrap_or_default().to_vec();
                if queue.send(read).is_err() {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

/// Reaps `shell`, returning its exit code as `$?` would say, or returns at once with none if
/// another waiter already has.
fn wait_for(shell: libc::pid_t) -> Option<i32> {
    let mut status = 0;
    // SAFETY: `status` is a live local.
    while unsafe { libc::waitpid(shell, &mut status, 0) } == -1 {
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return None;
        }
    }
    if libc::WIFEXITED(status) {
        Some(libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        Some(128 + libc::WTERMSIG(status))
    } else {
        None
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::super::MOST_QUEUED;
    use super::{Ordering, Output, Pty, Size};

    fn wait_for_text(output: &mut Output, text: &str) -> String {
        let started = Instant::now();
        let mut read = Vec::new();
        while !String::from_utf8_lossy(&read).contains(text) {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "{text:?} never came, only {:?}",
                String::from_utf8_lossy(&read)
            );
            let chunk = output.waiting();
            if chunk.is_empty() {
                std::thread::sleep(Duration::from_millis(20));
            }
            read.extend(chunk);
        }
        String::from_utf8_lossy(&read).into_owned()
    }

    const SIZE: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };

    #[test]
    fn a_shell_outrunning_its_reader_waits_and_nothing_is_lost_when_its_kept() {
        let (mut pty, mut output) = Pty::spawn(&std::env::temp_dir(), SIZE, None).unwrap();
        const LAST: u32 = 400_000;
        pty.write(format!("echo; seq 1 {LAST}; echo done-$((1+1))\n").as_bytes())
            .unwrap();
        let started = Instant::now();
        while output.queued.load(Ordering::SeqCst) < MOST_QUEUED {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "the queue never filled"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(300));
        let queued = output.queued.load(Ordering::SeqCst);
        assert!(
            queued < MOST_QUEUED + 64 * 1024,
            "the reader stops within a read of the limit, not at {queued}"
        );

        let kept = pty.keep().unwrap();
        let mut read = String::from_utf8_lossy(&output.waiting()).into_owned();
        drop(pty);
        let (_pty, mut output) = Pty::adopt(kept).unwrap();
        read.push_str(&wait_for_text(&mut output, "done-2"));

        let lines: Vec<u32> = read.lines().filter_map(|l| l.trim().parse().ok()).collect();
        assert_eq!(
            lines,
            (1..=LAST).collect::<Vec<_>>(),
            "every line arrives once, in order, across the pause and the keep"
        );
    }

    /// All `command` writes, run in place of a prompt, once its output ends.
    fn run_to_end(command: &str) -> String {
        let (_pty, output) = Pty::spawn(&std::env::temp_dir(), SIZE, Some(command)).unwrap();
        let (ended, read) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let chunks = futures::executor::block_on(futures::StreamExt::collect::<Vec<_>>(output));
            ended.send(chunks.concat()).ok();
        });
        let read = read
            .recv_timeout(Duration::from_secs(20))
            .expect("the output ends once the command exits, with no prompt left waiting");
        String::from_utf8_lossy(&read).into_owned()
    }

    #[test]
    fn a_command_runs_in_the_shell_in_place_of_a_prompt() {
        assert!(
            run_to_end("echo ran-$((40+2))").contains("ran-42"),
            "the shell expanded and ran the command"
        );
    }

    #[test]
    fn a_command_runs_in_an_interactive_shell() {
        assert!(
            run_to_end("case $- in *i*) echo interactive-shell;; esac")
                .contains("interactive-shell"),
            "the shell reads the config it reads at a prompt, as zsh does .zshrc only when interactive"
        );
    }

    #[test]
    fn a_kept_shell_answers_whoever_adopts_it() {
        let (pty, mut output) = Pty::spawn(&std::env::temp_dir(), SIZE, None).unwrap();
        pty.write(b"echo $$-$((40+2))\n").unwrap();
        let before = wait_for_text(&mut output, "-42");

        let mut pty = pty;
        let kept = pty.keep().unwrap();
        drop(pty);
        let (pty, mut output) = Pty::adopt(kept).unwrap();
        pty.write(b"echo $$-$((50+8))\n").unwrap();
        let after = wait_for_text(&mut output, "-58");

        let pid = |read: &str, sum: &str| {
            let end = read.find(&format!("-{sum}\r")).unwrap();
            read[..end]
                .rsplit(|c: char| !c.is_ascii_digit())
                .next()
                .unwrap()
                .to_owned()
        };
        assert_eq!(
            pid(&before, "42"),
            pid(&after, "58"),
            "the same shell answers"
        );
    }
}
