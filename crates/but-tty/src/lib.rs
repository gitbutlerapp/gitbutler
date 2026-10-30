//! A terminal without a screen: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt on a thread of its own. What it's given is in bytes and cells, what it gives
//! back is a frame of styled text to draw, and what the emulator answers the shell goes
//! straight back to it.

mod emulator;
mod ghostty;
#[cfg(unix)]
mod handoff;
mod pty;
mod vt;

use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{self, Poll};
use std::time::{Duration, Instant};

use emulator::{Emulator, Message, Shown};
use futures::channel::mpsc;
use futures::{Stream, StreamExt as _};
use parking_lot::Mutex;

pub use ghostty::{Colors, Ghostty};
#[cfg(unix)]
pub use handoff::{hand_over, handed};
#[cfg(unix)]
use pty::Kept;
pub use pty::Size;
use pty::{Output, Pty};
pub use vt::{Cursor, Frame, Row, Run, Status, Style};

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

/// The user's shell with the emulator reading what it writes on a thread of its own: this
/// sends it what the user does and reads what it last showed, so neither waits on the other.
/// Positions are on the surface: the grid at the size's whole-pixel cells.
pub struct Terminal {
    messages: mpsc::UnboundedSender<Message>,
    shown: Arc<Mutex<Shown>>,
    /// The size last asked for, which the emulator may not have reached yet.
    size: Size,
    /// The title and status as last told, to tell what changed.
    told: (String, Status),
}

/// Wakes each time the terminal has more to show, ending once the shell exits or the terminal
/// is detached.
pub struct Updates(mpsc::Receiver<()>);

impl Stream for Updates {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, task: &mut task::Context) -> Poll<Option<()>> {
        self.0.poll_next_unpin(task)
    }
}

impl Terminal {
    /// A terminal on `shell`, drawn in the defaults `colors` sets, with `background` and
    /// `foreground` where it sets none, from its first frame.
    pub fn new(
        shell: Shell,
        colors: &Colors,
        background: RgbColor,
        foreground: RgbColor,
    ) -> anyhow::Result<(Self, Updates)> {
        let Shell { pty, output, size } = shell;
        let colors = colors.clone();
        Self::start(pty, output, size, Status::default(), move |emulator| {
            emulator.set_colors(colors, background, foreground);
        })
    }

    /// Takes over a shell another terminal detached, showing what it showed. Its colours are
    /// set once they're known.
    #[cfg(unix)]
    pub fn adopt(detached: Detached) -> anyhow::Result<(Self, Updates)> {
        let Detached {
            kept,
            size,
            screen,
            status,
        } = detached;
        let (pty, output) = Pty::adopt(kept)?;
        Self::start(pty, output, size, status, move |emulator| {
            emulator.feed(screen.as_bytes());
        })
    }

    fn start(
        pty: Pty,
        output: Output,
        size: Size,
        status: Status,
        setup: impl FnOnce(&mut Emulator) + Send + 'static,
    ) -> anyhow::Result<(Self, Updates)> {
        let started = emulator::start(pty, output, size, status.clone(), setup)?;
        let title = started.shown.lock().title.clone();
        let terminal = Self {
            messages: started.messages,
            shown: started.shown,
            size,
            told: (title, status),
        };
        Ok((terminal, Updates(started.updates)))
    }

    /// Stops reading the shell, for another to adopt it, once what it wrote until now is shown.
    #[cfg(unix)]
    pub fn detach(&mut self) -> anyhow::Result<Detached> {
        let (reply, detached) = std::sync::mpsc::channel();
        self.send(Message::Detach(reply));
        detached.recv()?
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn title(&self) -> String {
        self.shown.lock().title.clone()
    }

    /// The title, if the shell set another since this was last asked.
    pub fn retitled(&mut self) -> Option<String> {
        let title = self.title();
        (title != self.told.0).then(|| {
            self.told.0 = title.clone();
            title
        })
    }

    pub fn status(&self) -> Status {
        self.shown.lock().status.clone()
    }

    /// The status, if it changed since this was last asked: what the program running reported,
    /// or idle once it's gone without saying, as being at work or blocked lasts only as long as
    /// it does.
    pub fn restatused(&mut self) -> Option<Status> {
        let status = self.status();
        (status != self.told.1).then(|| {
            self.told.1 = status.clone();
            status
        })
    }

    /// The status once the user has seen the terminal, to whom a result is then no news.
    pub fn seen(&mut self) -> Status {
        let mut shown = self.shown.lock();
        if matches!(shown.status, Status::Done | Status::Failed(_)) {
            shown.status = Status::Idle;
        }
        self.told.1 = shown.status.clone();
        shown.status.clone()
    }

    pub fn resize(&mut self, size: Size) {
        self.size = size;
        self.run(move |emulator| emulator.resize(size));
    }

    /// Sends what was typed, showing the bottom and dropping the selection.
    pub fn input(&mut self, bytes: &[u8]) {
        let bytes = bytes.to_vec();
        self.run(move |emulator| emulator.input(&bytes));
    }

    /// `key` pressed, typing `text`, as the key `unshifted` would without shift.
    pub fn key(&mut self, key: Key, mods: Mods, text: Option<String>, unshifted: Option<char>) {
        self.run(move |emulator| {
            let bytes = emulator.vt.key(key, mods, text, unshifted);
            emulator.input(&bytes);
        });
    }

    pub fn paste(&mut self, text: &str) {
        let text = text.to_owned();
        self.run(move |emulator| {
            let bytes = emulator.vt.paste(&text);
            emulator.input(&bytes);
        });
    }

    /// Whether the program running asked to be told of the mouse.
    pub fn tracks_mouse(&self) -> bool {
        self.shown.lock().tracks_mouse
    }

    /// `button` going `action` over `at`, for a program tracking the mouse.
    pub fn mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        at: mouse::Position,
        mods: Mods,
    ) {
        self.run(move |emulator| {
            let bytes = emulator.vt.mouse(action, button, at, mods);
            emulator.send(&bytes);
        });
    }

    /// The wheel turned `lines` up, or down if negative, over `at`.
    pub fn scroll(&mut self, lines: f32, at: mouse::Position, mods: Mods) {
        self.run(move |emulator| {
            let bytes = emulator.vt.scroll(lines, at, mods);
            emulator.send(&bytes);
        });
    }

    /// A left press over `at`, `repeated` when the platform counts it the next click in a row.
    pub fn select_press(&mut self, at: mouse::Position, repeated: bool) {
        self.run(move |emulator| {
            emulator.vt.select_press(at, repeated).ok();
        });
    }

    /// The press dragged to `at`; a block between it and where it began if `rectangle`.
    pub fn select_drag(&mut self, at: mouse::Position, rectangle: bool) {
        self.run(move |emulator| {
            emulator.vt.select_drag(at, rectangle).ok();
        });
    }

    pub fn select_release(&mut self, at: mouse::Position) {
        self.run(move |emulator| {
            emulator.vt.select_release(at).ok();
        });
    }

    /// Scrolls a row the way the drag went past the top or bottom, if it did, taking the
    /// selection on to `at`.
    pub fn select_autoscroll(&mut self, at: mouse::Position, rectangle: bool) {
        self.run(move |emulator| {
            emulator.vt.select_autoscroll(at, rectangle).ok();
        });
    }

    pub fn select_all(&mut self) {
        self.run(|emulator| emulator.vt.select_all());
    }

    /// What's selected, as Ghostty copies it, once the emulator gets to the question.
    pub fn selected(&self) -> Option<String> {
        let (reply, selected) = std::sync::mpsc::channel();
        self.run(move |emulator| {
            reply.send(emulator.vt.selected()).ok();
        });
        selected.recv().ok().flatten()
    }

    /// The defaults `colors` sets, with `background` and `foreground` where it sets none, and
    /// its selection colours for what's selected.
    pub fn set_colors(&mut self, colors: &Colors, background: RgbColor, foreground: RgbColor) {
        let colors = colors.clone();
        self.run(move |emulator| emulator.set_colors(colors, background, foreground));
    }

    /// What to draw, as far as the emulator got: while the program updates the screen in one
    /// go, the frame from before, so a half-drawn one never shows.
    pub fn frame(&self) -> Arc<Frame> {
        self.shown.lock().frame.clone()
    }

    /// Whether the terminal is on screen. Out of sight, what the shell writes is read and
    /// answered but not drawn, which costs a laptop's battery for nothing; shown again, it's
    /// drawn at once.
    pub fn set_shown(&mut self, shown: bool) {
        let mut state = self.shown.lock();
        if state.drawn == shown {
            return;
        }
        let stale = shown && state.stale;
        state.drawn = shown;
        drop(state);
        self.run(move |emulator| emulator.vt.set_capturing(shown));
        if stale {
            self.send(Message::Redraw);
        }
    }

    /// How long until the screen is drawn as it is though the program holds it, if it does: it
    /// may never say it's done.
    pub fn hold_left(&self) -> Option<Duration> {
        let until = self.shown.lock().hold_until;
        until.map(|until| until.saturating_duration_since(Instant::now()))
    }

    /// Draws the screen again, as when a hold has timed out.
    pub fn redraw(&self) {
        self.send(Message::Redraw);
    }

    fn run(&self, job: impl FnOnce(&mut Emulator) + Send + 'static) {
        self.send(Message::Run(Box::new(job)));
    }

    fn send(&self, message: Message) {
        self.messages.unbounded_send(message).ok();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Colors, RgbColor, Shell, Size, Status, Terminal};

    const SIZE: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };

    const BLACK: RgbColor = RgbColor { r: 0, g: 0, b: 0 };

    fn started() -> Terminal {
        let shell = Shell::spawn(&std::env::temp_dir(), SIZE).unwrap();
        Terminal::new(shell, &Colors::default(), BLACK, BLACK)
            .unwrap()
            .0
    }

    /// Waits for `done` to hold of `terminal`, which the emulator gets to on its own thread.
    fn until(terminal: &mut Terminal, what: &str, mut done: impl FnMut(&mut Terminal) -> bool) {
        let started = Instant::now();
        while !done(terminal) {
            assert!(started.elapsed() < Duration::from_secs(20), "{what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn what_the_shell_prints_is_drawn() {
        let mut terminal = started();
        terminal.input(b"printf 'printed-%s\\n' 42\n");
        until(
            &mut terminal,
            "the shell's output should be drawn",
            |terminal| {
                let frame = terminal.frame();
                // After the prompt, as the shell echoed it before it was ready.
                (frame.rows.iter()).any(|row| row.text.trim_end().ends_with("printed-42"))
            },
        );
    }

    #[test]
    fn out_of_sight_it_reads_the_shell_but_draws_only_once_shown() {
        let mut terminal = started();
        terminal.set_shown(false);
        // The status after the text, so once it's read the text is too.
        terminal.input(br"printf 'printed-%s\n' 42; printf '\033]7501;state=done\033\\'");
        terminal.input(b"\n");
        until(
            &mut terminal,
            "the shell should be read out of sight",
            |terminal| terminal.status() == Status::Done,
        );
        let drawn = |terminal: &mut Terminal| {
            let frame = terminal.frame();
            (frame.rows.iter()).any(|row| row.text.trim_end().ends_with("printed-42"))
        };
        assert!(!drawn(&mut terminal), "out of sight, nothing is drawn");
        terminal.set_shown(true);
        until(&mut terminal, "shown again, it should be drawn", drawn);
    }

    #[test]
    fn out_of_sight_a_hold_still_times_out() {
        let mut terminal = started();
        terminal.input(br"printf '\033[?2026h'");
        terminal.input(b"\n");
        until(
            &mut terminal,
            "the program should hold the screen",
            |terminal| terminal.hold_left().is_some(),
        );
        terminal.set_shown(false);
        std::thread::sleep(Duration::from_millis(1100));
        terminal.redraw();
        until(
            &mut terminal,
            "the hold should end though nothing is drawn, or the UI is woken to redraw forever",
            |terminal| terminal.hold_left().is_none(),
        );
    }

    #[test]
    fn letting_go_of_a_terminal_ends_its_thread() {
        let shell = Shell::spawn(&std::env::temp_dir(), SIZE).unwrap();
        let (terminal, updates) = Terminal::new(shell, &Colors::default(), BLACK, BLACK).unwrap();
        drop(terminal);
        let (ended, done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            futures::executor::block_on(futures::StreamExt::for_each(updates, |()| async {}));
            ended.send(()).ok();
        });
        assert!(
            done.recv_timeout(Duration::from_secs(10)).is_ok(),
            "its updates end with the emulator's thread, which hangs up the shell"
        );
    }

    #[test]
    fn a_result_stays_through_a_handover_until_it_is_seen() {
        let mut terminal = started();
        terminal.input(br"printf '\033]7501;state=done\033\\'");
        terminal.input(b"\n");
        until(
            &mut terminal,
            "the program should say it's done",
            |terminal| terminal.status() == Status::Done,
        );

        let detached = terminal.detach().unwrap();
        let (mut adopted, _updates) = Terminal::adopt(detached).unwrap();
        assert_eq!(
            adopted.status(),
            Status::Done,
            "programs only say what changed, so the next build can't learn it again"
        );
        assert_eq!(adopted.seen(), Status::Idle, "seen, a result is no news");
    }

    #[test]
    fn a_program_killed_while_working_leaves_the_terminal_idle() {
        let mut terminal = started();
        terminal.input(br#"sh -c "printf '\033]7501;state=working\033\\'; sleep 1; kill -9 \$\$""#);
        terminal.input(b"\n");

        let mut told = Vec::new();
        until(
            &mut terminal,
            "it should say idle after working",
            |terminal| {
                told.extend(terminal.restatused());
                told.last() == Some(&Status::Idle)
            },
        );
        assert_eq!(
            told,
            [Status::Working, Status::Idle],
            "the shell taking the terminal back ends what the program said"
        );
    }
}
