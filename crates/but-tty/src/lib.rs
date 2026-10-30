//! A terminal without a screen: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt. What it's given is in bytes and cells, what it gives back is a frame of
//! styled text to draw, and what the emulator answers the shell goes straight back to it.

mod ghostty;
#[cfg(unix)]
mod handoff;
mod pty;
mod vt;

use std::path::Path;

pub use ghostty::{Colors, Ghostty};
#[cfg(unix)]
pub use handoff::{hand_over, handed};
#[cfg(unix)]
use pty::Kept;
use pty::Pty;
pub use pty::{Output, Size};
use vt::Vt;
pub use vt::{Cursor, Frame, Row, Status, Style};

pub use libghostty_vt::key::{Key, Mods};
pub use libghostty_vt::mouse;
pub use libghostty_vt::style::{RgbColor, Underline};

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
    kept: Kept,
    size: Size,
    screen: String,
    status: Status,
}

/// The user's shell with the emulator reading what it writes. Positions are on the surface:
/// the grid at the size's whole-pixel cells.
pub struct Terminal {
    pty: Pty,
    vt: Vt,
    status: Status,
}

impl Terminal {
    pub fn new(shell: Shell) -> anyhow::Result<(Self, Output)> {
        let Shell { pty, output, size } = shell;
        let vt = Vt::new(size)?;
        let status = Status::default();
        Ok((Self { pty, vt, status }, output))
    }

    /// Takes over a shell another terminal detached, showing what it showed.
    #[cfg(unix)]
    pub fn adopt(detached: Detached) -> anyhow::Result<(Self, Output)> {
        let Detached {
            kept,
            size,
            screen,
            status,
        } = detached;
        let (pty, output) = Pty::adopt(kept)?;
        let (mut terminal, output) = Self::new(Shell { pty, output, size })?;
        terminal.feed(screen.as_bytes());
        terminal.status = status;
        Ok((terminal, output))
    }

    /// Stops reading the shell, for another to adopt it, once what it wrote to `output` until
    /// now is shown.
    #[cfg(unix)]
    pub fn detach(&mut self, output: &mut Output) -> anyhow::Result<Detached> {
        let kept = self.pty.keep()?;
        let left = output.waiting();
        self.feed(&left);
        self.restatused();
        let screen = String::from_utf8_lossy(&self.vt.screen()?).into_owned();
        Ok(Detached {
            kept,
            size: self.vt.size(),
            screen,
            status: self.status.clone(),
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

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// The status, if it changed since this was last asked: what the program running reported,
    /// or idle once it's gone without saying, as being at work or blocked lasts only as long as
    /// it does.
    pub fn restatused(&mut self) -> Option<Status> {
        let reported = self.vt.reported().unwrap_or_else(|| self.status.clone());
        let status = match reported {
            Status::Working | Status::Blocked(_) if self.pty.shell_in_front() => Status::Idle,
            status => status,
        };
        (status != self.status).then(|| {
            self.status = status.clone();
            status
        })
    }

    /// The status once the user has seen the terminal, to whom a result is then no news.
    pub fn seen(&mut self) -> Status {
        if matches!(self.status, Status::Done | Status::Failed(_)) {
            self.status = Status::Idle;
        }
        self.status.clone()
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

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Shell, Size, Status, Terminal};

    const SIZE: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };

    #[test]
    fn a_result_stays_through_a_handover_until_it_is_seen() {
        let shell = Shell::spawn(&std::env::temp_dir(), SIZE).unwrap();
        let (mut terminal, mut output) = Terminal::new(shell).unwrap();
        terminal.feed(b"\x1b]7501;state=done\x07");
        terminal.restatused();

        let detached = terminal.detach(&mut output).unwrap();
        let (mut adopted, _output) = Terminal::adopt(detached).unwrap();
        assert_eq!(
            adopted.status(),
            &Status::Done,
            "programs only say what changed, so the next build can't learn it again"
        );
        assert_eq!(adopted.seen(), Status::Idle, "seen, a result is no news");
    }

    #[test]
    fn a_program_killed_while_working_leaves_the_terminal_idle() {
        let shell = Shell::spawn(&std::env::temp_dir(), SIZE).unwrap();
        let (mut terminal, mut output) = Terminal::new(shell).unwrap();
        terminal.input(br#"sh -c "printf '\033]7501;state=working\033\\'; sleep 1; kill -9 \$\$""#);
        terminal.input(b"\n");

        let started = Instant::now();
        let mut told = Vec::new();
        while told.last() != Some(&Status::Idle) {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "it should say idle after working, but said {told:?}"
            );
            terminal.feed(&output.waiting());
            told.extend(terminal.restatused());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            told,
            [Status::Working, Status::Idle],
            "the shell taking the terminal back ends what the program said"
        );
    }
}
