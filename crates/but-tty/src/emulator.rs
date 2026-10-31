//! The emulator on a thread of its own, where what the shell writes is read, answered and
//! drawn, so a program printing without pause never holds up the UI. libghostty's terminal
//! can't leave the thread it was made on, so what the UI does to it is sent there to run.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::{FutureExt as _, StreamExt as _, future, stream};
use parking_lot::Mutex;

#[cfg(unix)]
use super::Detached;
use super::pty::{Output, Pty};
use super::vt::Vt;
use super::{Colors, Frame, RgbColor, Size, Status};

/// How long the thread works through what's waiting before it shows how far it got: about a
/// frame at 120Hz, so a program printing without pause is drawn as it goes.
const BATCH: Duration = Duration::from_millis(8);

/// Something the UI did to the terminal, run on the emulator's thread.
pub(crate) type Job = Box<dyn FnOnce(&mut Emulator) + Send>;

/// What reaches the emulator's thread, in the order it happened.
pub(crate) enum Message {
    /// What the shell wrote.
    Output(Vec<u8>),
    Run(Job),
    /// The terminal shows again, or a hold ran out, so what changed is to be drawn.
    Redraw,
    /// Stop reading the shell, for another to adopt it.
    #[cfg(unix)]
    Detach(std::sync::mpsc::Sender<anyhow::Result<Detached>>),
    /// The shell exited, or the UI let go of the terminal.
    End,
}

/// What the UI reads of the terminal, as of the last batch the emulator worked through.
pub(crate) struct Shown {
    /// The latest frame, drawn after each batch while the terminal is on screen, so the UI
    /// always draws as far as the emulator got.
    pub(crate) frame: Arc<Frame>,
    /// Whether the terminal is on screen. One out of sight is read and answered, but drawn only
    /// once it shows again.
    pub(crate) drawn: bool,
    /// Whether something changed that `frame` doesn't show, as it changed out of sight.
    pub(crate) stale: bool,
    pub(crate) title: String,
    pub(crate) status: Status,
    pub(crate) tracks_mouse: bool,
    /// What the program running last copied, until the UI puts it on the clipboard.
    pub(crate) copied: Option<String>,
    /// When the program's hold on the screen times out, if it holds it.
    pub(crate) hold_until: Option<Instant>,
}

/// The shell and the emulator reading what it writes.
pub(crate) struct Emulator {
    pty: Pty,
    pub(crate) vt: Vt,
    /// The selection colours frames draw in.
    chosen: Colors,
    /// Whether anything changed since the last frame was drawn.
    unshown: bool,
}

/// What the UI talks to an emulator's thread with.
pub(crate) struct Started {
    pub(crate) messages: mpsc::UnboundedSender<Message>,
    pub(crate) shown: Arc<Mutex<Shown>>,
    /// Woken each time there's more to show, ending with the thread.
    pub(crate) updates: mpsc::Receiver<()>,
}

/// Starts the emulator's thread on `pty`, sized `size` and set up by `setup` there.
pub(crate) fn start(
    pty: Pty,
    output: Output,
    size: Size,
    status: Status,
    setup: impl FnOnce(&mut Emulator) + Send + 'static,
) -> anyhow::Result<Started> {
    let (messages, received) = mpsc::unbounded();
    let (wake, updates) = mpsc::channel(0);
    let shown = Arc::new(Mutex::new(Shown {
        frame: Arc::default(),
        drawn: true,
        stale: false,
        title: String::new(),
        status,
        tracks_mouse: false,
        copied: None,
        hold_until: None,
    }));
    let (ready, started) = std::sync::mpsc::channel();
    let showing = shown.clone();
    std::thread::Builder::new()
        .name("terminal".into())
        .spawn(move || {
            let vt = match Vt::new(size) {
                Ok(vt) => vt,
                Err(err) => {
                    ready.send(Err(err)).ok();
                    return;
                }
            };
            let mut emulator = Emulator {
                pty,
                vt,
                chosen: Colors::default(),
                unshown: true,
            };
            setup(&mut emulator);
            emulator.publish(&showing);
            if ready.send(Ok(())).is_ok() {
                run(emulator, output, received, &showing, wake);
            }
        })?;
    started.recv()??;
    Ok(Started {
        messages,
        shown,
        updates,
    })
}

/// Works through what reaches the thread a batch at a time, showing each, until the shell
/// exits, the UI lets go or it's detached.
fn run(
    mut emulator: Emulator,
    output: Output,
    received: mpsc::UnboundedReceiver<Message>,
    shown: &Mutex<Shown>,
    mut wake: mpsc::Sender<()>,
) {
    let end = || stream::once(future::ready(Message::End));
    let output = output.map(Message::Output).chain(end());
    let mut messages = stream::select(output, received.chain(end()));
    futures::executor::block_on(async {
        while let Some(first) = messages.next().await {
            let started = Instant::now();
            let mut next = Some(first);
            while let Some(message) = next.take() {
                match message {
                    Message::Output(bytes) => emulator.feed(&bytes),
                    Message::Run(job) => {
                        job(&mut emulator);
                        emulator.unshown = true;
                    }
                    Message::Redraw => {}
                    #[cfg(unix)]
                    Message::Detach(reply) => {
                        reply.send(emulator.detach(&mut messages, shown)).ok();
                        return;
                    }
                    Message::End => return,
                }
                if started.elapsed() < BATCH {
                    next = messages.next().now_or_never().flatten();
                }
            }
            // A wake already waiting covers this one.
            if emulator.publish(shown) {
                wake.try_send(()).ok();
            }
        }
    });
}

impl Emulator {
    /// Shows what the shell wrote, answering what it asked.
    pub(crate) fn feed(&mut self, output: &[u8]) {
        self.vt.write(output);
        let replies = self.vt.take_replies();
        self.send(&replies);
        self.unshown = true;
    }

    pub(crate) fn resize(&mut self, size: Size) {
        if size == self.vt.size() {
            return;
        }
        let report = self.vt.resize(size);
        self.pty.resize(size).ok();
        self.send(&report);
    }

    /// Sends what was typed, showing the bottom and dropping the selection.
    pub(crate) fn input(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.vt.typed();
        self.send(bytes);
    }

    /// The defaults `colors` sets, with `background` and `foreground` where it sets none, and
    /// its selection colours for frames.
    pub(crate) fn set_colors(
        &mut self,
        colors: Colors,
        background: RgbColor,
        foreground: RgbColor,
    ) {
        let report = self.vt.set_colors(&colors, background, foreground);
        self.send(&report);
        self.chosen = colors;
    }

    pub(crate) fn send(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.pty.write(bytes).ok();
        }
    }

    /// Updates what the UI reads, drawing a frame if something changed while the terminal is on
    /// screen, and returns whether anything it reads did. A program's status lasts only as long
    /// as it does: once the shell has the terminal back, being at work or blocked is over.
    fn publish(&mut self, shown: &Mutex<Shown>) -> bool {
        // Drawn or not, so a hold out of sight doesn't stay due forever.
        let hold_ended = self.vt.end_timed_out_hold();
        let title = self.vt.retitled();
        let reported = self.vt.reported();
        let shell_in_front = self.pty.shell_in_front();
        if shell_in_front {
            self.vt.reporter_gone();
        }
        let tracks_mouse = self.vt.tracks_mouse();
        let copied = self.vt.copied();
        let due = self.unshown || hold_ended;
        let (draw, mut changed) = {
            let mut shown = shown.lock();
            let mut changed = false;
            if let Some(title) = title {
                changed |= shown.title != title;
                shown.title = title;
            }
            let status = match reported.unwrap_or_else(|| shown.status.clone()) {
                Status::Working | Status::Blocked(_) if shell_in_front => Status::Idle,
                status => status,
            };
            changed |= shown.status != status;
            shown.status = status;
            changed |= shown.tracks_mouse != tracks_mouse;
            shown.tracks_mouse = tracks_mouse;
            if copied.is_some() {
                shown.copied = copied;
                changed = true;
            }
            let draw = due && shown.drawn;
            changed |= shown.stale != (due && !draw);
            shown.stale = due && !draw;
            (draw, changed)
        };
        let frame = draw.then(|| self.vt.frame(&self.chosen).unwrap_or_default());
        let hold_until = self.vt.hold_deadline();
        let mut shown = shown.lock();
        if let Some(frame) = frame {
            self.unshown = false;
            changed |= !Arc::ptr_eq(&frame, &shown.frame);
            shown.frame = frame;
        }
        changed |= shown.hold_until != hold_until;
        shown.hold_until = hold_until;
        changed
    }

    /// Stops reading the shell, once what it wrote until now is shown, for another to adopt.
    #[cfg(unix)]
    fn detach(
        &mut self,
        messages: &mut (impl futures::Stream<Item = Message> + Unpin),
        shown: &Mutex<Shown>,
    ) -> anyhow::Result<Detached> {
        let kept = self.pty.keep()?;
        // The reader has stopped, so all it read is waiting.
        while let Some(Some(message)) = messages.next().now_or_never() {
            if let Message::Output(bytes) = message {
                self.feed(&bytes);
            }
        }
        self.publish(shown);
        let screen = String::from_utf8_lossy(&self.vt.screen()?).into_owned();
        Ok(Detached {
            kept,
            size: self.vt.size(),
            screen,
            status: shown.lock().status.clone(),
        })
    }
}
