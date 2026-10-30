//! A terminal without a screen: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt. What it's given is in bytes and cells, what it gives back is a frame of
//! styled text to draw, and what the emulator answers the shell goes straight back to it.

mod ghostty;
mod pty;
mod vt;

use std::path::Path;

use futures::channel::mpsc;
use libghostty_vt::key::{Key, Mods};
use libghostty_vt::mouse;
use libghostty_vt::style::RgbColor;

pub use ghostty::{Colors, Ghostty};
#[cfg(unix)]
pub use pty::Kept;
use pty::Pty;
pub use pty::Size;
use vt::Vt;
pub use vt::{Cursor, Frame, Style};

/// What the shell writes, in chunks, ending once it exits.
pub type Output = mpsc::UnboundedReceiver<Vec<u8>>;

/// A shell just started, with nothing emulating it yet, so it can start off the UI thread.
pub struct Shell {
    pty: Pty,
    output: Output,
    size: Size,
}

impl Shell {
    /// Starts the user's shell in `cwd`, sized `size`.
    pub fn spawn(cwd: &Path, size: Size) -> anyhow::Result<Self> {
        let (pty, output) = Pty::spawn(cwd, size)?;
        Ok(Self { pty, output, size })
    }
}

/// A shell another terminal stopped reading, with what it showed until then.
#[cfg(unix)]
pub struct Detached {
    pub(crate) kept: Kept,
    pub(crate) size: Size,
    pub(crate) screen: String,
}

/// The user's shell with the emulator reading what it writes. Positions are on the surface:
/// the grid at the size's whole-pixel cells.
pub struct Terminal {
    pty: Pty,
    vt: Vt,
}

impl Terminal {
    pub fn new(shell: Shell) -> anyhow::Result<(Self, Output)> {
        let Shell { pty, output, size } = shell;
        let vt = Vt::new(size)?;
        Ok((Self { pty, vt }, output))
    }

    /// Takes over a shell another terminal detached, showing what it showed.
    #[cfg(unix)]
    pub fn adopt(detached: Detached) -> anyhow::Result<(Self, Output)> {
        let Detached { kept, size, screen } = detached;
        let (pty, output) = Pty::adopt(kept)?;
        let (mut terminal, output) = Self::new(Shell { pty, output, size })?;
        terminal.feed(screen.as_bytes());
        Ok((terminal, output))
    }

    /// Stops reading the shell, for another to adopt it, once what it wrote to `output` until
    /// now is shown.
    #[cfg(unix)]
    pub fn detach(&mut self, output: &mut Output) -> anyhow::Result<Detached> {
        let kept = self.pty.keep()?;
        let left: Vec<u8> = std::iter::from_fn(|| output.try_recv().ok())
            .flatten()
            .collect();
        self.feed(&left);
        let screen = String::from_utf8_lossy(&self.vt.screen()?).into_owned();
        Ok(Detached {
            kept,
            size: self.vt.size(),
            screen,
        })
    }

    pub fn size(&self) -> Size {
        self.vt.size()
    }

    pub fn title(&self) -> String {
        self.vt.title()
    }

    /// Shows what the shell wrote, returning the title it set, if it set one.
    pub fn feed(&mut self, output: &[u8]) -> Option<String> {
        self.vt.write(output);
        let replies = self.vt.take_replies();
        self.send(&replies);
        self.vt.retitled()
    }

    pub fn resize(&mut self, size: Size) {
        if size == self.vt.size() {
            return;
        }
        self.vt.resize(size);
        self.pty.resize(size).ok();
    }

    /// Sends what was typed, showing the bottom and dropping the selection.
    pub fn input(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.vt.typed();
        self.send(bytes);
    }

    /// `key` pressed, typing `text`, as the key `unshifted` would without shift.
    pub fn key(&mut self, key: Key, mods: Mods, text: Option<String>, unshifted: Option<char>) {
        let bytes = self.vt.key(key, mods, text, unshifted);
        self.input(&bytes);
    }

    pub fn paste(&mut self, text: &str) {
        let bytes = self.vt.paste(text);
        self.input(&bytes);
    }

    /// Whether the program running asked to be told of the mouse.
    pub fn tracks_mouse(&self) -> bool {
        self.vt.tracks_mouse()
    }

    /// `button` going `action` over `at`, for a program tracking the mouse.
    pub fn mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        at: mouse::Position,
        mods: Mods,
    ) {
        let bytes = self.vt.mouse(action, button, at, mods);
        self.send(&bytes);
    }

    /// The wheel turned `lines` up, or down if negative, over `at`.
    pub fn scroll(&mut self, lines: f32, at: mouse::Position, mods: Mods) {
        let bytes = self.vt.scroll(lines, at, mods);
        self.send(&bytes);
    }

    /// A left press over `at`, `repeated` when the platform counts it the next click in a row.
    pub fn select_press(&mut self, at: mouse::Position, repeated: bool) {
        self.vt.select_press(at, repeated).ok();
    }

    /// The press dragged to `at`; a block between it and where it began if `rectangle`.
    pub fn select_drag(&mut self, at: mouse::Position, rectangle: bool) {
        self.vt.select_drag(at, rectangle).ok();
    }

    pub fn select_release(&mut self, at: mouse::Position) {
        self.vt.select_release(at).ok();
    }

    /// Whether the selection's drag went past the top or bottom, for the viewport to follow.
    pub fn autoscrolling(&self) -> bool {
        self.vt.autoscrolling()
    }

    /// Scrolls a row the way the drag went past, taking the selection on to `at`, and returns
    /// whether it's still past.
    pub fn select_autoscroll(&mut self, at: mouse::Position, rectangle: bool) -> bool {
        self.vt.select_autoscroll(at, rectangle).is_ok() && self.vt.autoscrolling()
    }

    pub fn select_all(&mut self) {
        self.vt.select_all();
    }

    /// What's selected, as Ghostty copies it.
    pub fn selected(&self) -> Option<String> {
        self.vt.selected()
    }

    /// The defaults `colors` sets, with `background` and `foreground` where it sets none.
    pub fn set_colors(&mut self, colors: &Colors, background: RgbColor, foreground: RgbColor) {
        self.vt.set_colors(colors, background, foreground);
    }

    /// What to draw, with what's selected in `chosen`'s selection colours.
    pub fn frame(&mut self, chosen: &Colors) -> Frame {
        self.vt.frame(chosen).unwrap_or_default()
    }

    fn send(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.pty.write(bytes).ok();
        }
    }
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
