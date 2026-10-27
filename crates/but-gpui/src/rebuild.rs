//! Build and restart, in the status bar of a client `cargo run` started: builds it again, from
//! the worktree picked when the repository has several, with cargo's output in a modal as it
//! comes, then restarts on the new binary in place. A failed build leaves the app as it was,
//! with the output on screen. Hidden, the modal keeps building.

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use gpui::{
    App, ClipboardItem, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, FontWeight,
    HighlightStyle, Hsla, ScrollStrategy, SharedString, StyledText, Task, UniformListScrollHandle,
    Window, div, prelude::*, uniform_list,
};

use crate::app::{AppView, BuildAndRestart};
use crate::backend::{
    self,
    rebuild::{Build, Printed, Rebuild},
};
use crate::palette::Choice;
use crate::state::Store;
use crate::theme::Theme;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{ButtonStyle, button, dp, duration, h_flex, muted, v_flex};

/// How tall a line of cargo's output is.
const LINE_HEIGHT: f32 = 18.;

impl AppView {
    /// Build and restart, for the status bar, or Building… while it builds.
    pub(crate) fn build_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let building =
            (self.building.as_ref()).is_some_and(|(output, _)| output.read(cx).running());
        let (label, tooltip) = match building {
            true => ("Building…", "Show the build"),
            false => (
                "Build and restart",
                "Build the app with cargo, and restart on it",
            ),
        };
        button("build-and-restart", label)
            .small()
            .style(ButtonStyle::Gray)
            .tooltip(tooltip)
            .on_click(cx.listener(|this, _, window, cx| {
                this.build_and_restart(&BuildAndRestart, window, cx)
            }))
    }

    /// Build the client again and restart on it, from the worktree picked when the repository
    /// has more than one with the client. While a build runs, show it instead.
    pub fn build_and_restart(
        &mut self,
        _: &BuildAndRestart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(rebuild) = self.rebuild.clone() else {
            return;
        };
        if self.show_running_build(window, cx) {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let listed = {
                let rebuild = rebuild.clone();
                backend::run(move || rebuild.sources()).await
            };
            // Without the list, the checkout running is still there to build.
            let mut sources = (listed)
                .inspect_err(|err| tracing::warn!("couldn't list the worktrees: {err:#}"))
                .unwrap_or_default();
            // The one running first, for Enter.
            sources.sort_by_key(|source| source.dir != rebuild.checkout());
            this.update_in(cx, |app, window, cx| {
                if sources.len() < 2 {
                    let dir = rebuild.checkout().to_owned();
                    return app.start_build(rebuild, dir, window, cx);
                }
                let choices = (sources.into_iter())
                    .map(|source| {
                        let folder = match source.dir.file_name() {
                            Some(name) => name.to_string_lossy().into_owned(),
                            None => source.dir.display().to_string(),
                        };
                        let detail = match source.dir == rebuild.checkout() {
                            true => format!("{folder} · running"),
                            false => folder.clone(),
                        };
                        let rebuild = rebuild.clone();
                        Choice::new(source.branch.unwrap_or(folder), move |app, window, cx| {
                            app.start_build(rebuild, source.dir, window, cx)
                        })
                        .detail(detail)
                    })
                    .collect();
                app.open_picker(None, "Build from worktree", "Build", choices, window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Show the build under way, if there is one. Another would wait on cargo's lock for it,
    /// and take its place here, leaving it out of sight and past cancelling.
    fn show_running_build(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let running = (self.building.as_ref())
            .map(|(output, _)| output.clone())
            .filter(|output| output.read(cx).running());
        let Some(output) = running else {
            return false;
        };
        self.open_modal(output, window, cx);
        true
    }

    /// Build the client in checkout `dir` and show it building.
    fn start_build(
        &mut self,
        rebuild: Rebuild,
        dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Asked for twice while the worktrees were listed, as by a double click.
        if self.show_running_build(window, cx) {
            return;
        }
        let store = self.store.clone();
        let output = cx.new(|cx| BuildOutput::new(rebuild, dir, store, cx));
        let ended = cx.subscribe_in(
            &output,
            window,
            |app, output, _: &BuildEnded, window, cx| {
                // Failed out of sight: show why, unless that would close another modal.
                if app.modal.is_none() {
                    app.open_modal(output.clone(), window, cx);
                }
                cx.notify();
            },
        );
        self.building = Some((output.clone(), ended));
        self.open_modal(output, window, cx);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Building,
    /// Built, restarting once the operations running finish.
    Restarting,
    Failed,
    Cancelled,
}

/// A build failed or was cancelled, leaving the app as it was.
pub struct BuildEnded;

/// A build and what cargo printed, shown in a modal.
pub struct BuildOutput {
    rebuild: Rebuild,
    /// The checkout it builds from.
    dir: PathBuf,
    store: gpui::Entity<Store>,
    state: State,
    lines: Vec<Line>,
    /// Cargo's progress bar as last drawn, under the lines while it builds, as in a terminal.
    progress: Option<Line>,
    /// The build, once cargo has started, to cancel.
    build: Option<Build>,
    started: Instant,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    _run: Task<()>,
    /// Counts the time up while it builds.
    _tick: Task<()>,
}

impl EventEmitter<DismissEvent> for BuildOutput {}
impl EventEmitter<BuildEnded> for BuildOutput {}

impl Focusable for BuildOutput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl BuildOutput {
    fn new(
        rebuild: Rebuild,
        dir: PathBuf,
        store: gpui::Entity<Store>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut output = Self {
            rebuild,
            dir,
            store,
            state: State::Building,
            lines: Vec::new(),
            progress: None,
            build: None,
            started: Instant::now(),
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            _run: Task::ready(()),
            _tick: Task::ready(()),
        };
        output.run(cx);
        output
    }

    /// Whether it's building, or built and restarting.
    pub fn running(&self) -> bool {
        matches!(self.state, State::Building | State::Restarting)
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        self.state = State::Building;
        self.build = None;
        self.started = Instant::now();
        self.lines = vec![Line::plain(format!(
            "cargo {}",
            self.rebuild.args().join(" ")
        ))];
        self.progress = None;
        cx.notify();
        self._tick = cx.spawn(async |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let building = this.update(cx, |this, cx| {
                    cx.notify();
                    this.state == State::Building
                });
                if !matches!(building, Ok(true)) {
                    return;
                }
            }
        });
        let (rebuild, dir, store) = (self.rebuild.clone(), self.dir.clone(), self.store.clone());
        self._run = cx.spawn(async move |this, cx| {
            let build = match backend::run(move || rebuild.start(&dir)).await {
                Ok(build) => build,
                Err(err) => {
                    this.update(cx, |this, cx| this.ended(State::Failed, Some(err), cx))
                        .ok();
                    return;
                }
            };
            let going = this.update(cx, |this, _| {
                // Cancelled while cargo started.
                if this.state == State::Cancelled {
                    build.cancel();
                }
                this.build = Some(build.clone());
                this.state == State::Building
            });
            if !matches!(going, Ok(true)) {
                return;
            }
            let (sender, lines) = futures::channel::mpsc::unbounded();
            let finished =
                backend::run(move || build.finish(|printed| _ = sender.unbounded_send(printed)));
            let shown = async {
                let mut lines = lines.ready_chunks(256);
                while let Some(lines) = lines.next().await {
                    if this.update(cx, |this, cx| this.add(lines, cx)).is_err() {
                        return;
                    }
                }
            };
            let (built, ()) = futures::join!(finished, shown);
            let built = match built {
                Ok(built) => built,
                Err(err) => {
                    this.update(cx, |this, cx| this.ended(State::Failed, Some(err), cx))
                        .ok();
                    return;
                }
            };
            let restarting = this.update(cx, |this, cx| {
                let building = this.state == State::Building;
                if building {
                    this.state = State::Restarting;
                    this.progress = None;
                    cx.notify();
                }
                building
            });
            if !matches!(restarting, Ok(true)) {
                return;
            }
            // What was started meanwhile finishes first, as quitting would stop it partway.
            while store.update(cx, |store, _| store.unfinished() > 0) {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
            // The quit handlers save what the app remembers and write out the log, as
            // quitting would, before the new binary takes this process's place.
            cx.update(|cx| {
                cx.shutdown();
                let err = built.restart();
                tracing::error!("couldn't restart on the new build: {err:#}");
                cx.quit();
            });
        });
    }

    fn add(&mut self, printed: Vec<Printed>, cx: &mut Context<Self>) {
        // A cancelled build's compilers can still be printing.
        if self.state != State::Building {
            return;
        }
        for printed in printed {
            match printed {
                Printed::Line(line) => self.lines.push(Line::from_terminal(&line)),
                // Drawn blank, it's cleared.
                Printed::Progress(bar) => {
                    self.progress =
                        Some(Line::from_terminal(&bar)).filter(|bar| !bar.text.trim().is_empty())
                }
            }
        }
        self.scroll_to_end();
        cx.notify();
    }

    /// The lines, then the progress bar while it shows.
    fn row(&self, ix: usize) -> Option<&Line> {
        match ix.checked_sub(self.lines.len()) {
            None => self.lines.get(ix),
            Some(0) => self.progress.as_ref(),
            Some(_) => None,
        }
    }

    fn rows(&self) -> usize {
        self.lines.len() + usize::from(self.progress.is_some())
    }

    fn scroll_to_end(&self) {
        (self.scroll).scroll_to_item(self.rows().saturating_sub(1), ScrollStrategy::Bottom);
    }

    /// Stop with `state`, saying why on the last line, unless it stopped already.
    fn ended(&mut self, state: State, why: Option<anyhow::Error>, cx: &mut Context<Self>) {
        if self.state != State::Building {
            return;
        }
        self.state = state;
        self.progress = None;
        if let Some(why) = why {
            self.lines.push(Line::plain(format!("{why:#}")));
        }
        self.scroll_to_end();
        cx.emit(BuildEnded);
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(build) = &self.build {
            build.cancel();
        }
        self.ended(State::Cancelled, None, cx);
    }

    fn copy(&self, cx: &mut Context<Self>) {
        let lines: Vec<&str> = self.lines.iter().map(|line| line.text.as_ref()).collect();
        let text = lines.join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

impl Render for BuildOutput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let elapsed = duration(self.started.elapsed().as_secs() as i64);
        let waiting = self.store.read(cx).unfinished();
        let (status, color) = match self.state {
            State::Building => (format!("Building… {elapsed}"), theme.text_2),
            State::Restarting if waiting > 0 => (
                "Built, restarting once the operations running finish…".to_owned(),
                theme.text_2,
            ),
            State::Restarting => ("Built, restarting…".to_owned(), theme.text_2),
            State::Failed => ("Build failed".to_owned(), theme.danger),
            State::Cancelled => ("Build cancelled".to_owned(), theme.text_2),
        };
        let folder = match self.dir.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => self.dir.display().to_string(),
        };
        let header = v_flex()
            .flex_none()
            .px(dp(20.))
            .py(dp(14.))
            .gap(dp(2.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .text_size(dp(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!("Build and restart from {folder}")),
            )
            .child(muted(status, theme).text_color(color));
        let list = uniform_list(
            "build-output",
            self.rows(),
            cx.processor(|this, range: Range<usize>, _, cx| {
                let theme = Theme::get(cx);
                (range.filter_map(|ix| this.row(ix)))
                    .map(|line| {
                        let styles = line.styles.iter().map(|(range, style)| {
                            let style = HighlightStyle {
                                color: style.color.map(|color| terminal_color(color, theme)),
                                font_weight: style.bold.then_some(FontWeight::SEMIBOLD),
                                ..Default::default()
                            };
                            (range.clone(), style)
                        });
                        div()
                            .w_full()
                            .h(dp(LINE_HEIGHT))
                            .px(dp(20.))
                            .whitespace_nowrap()
                            .child(StyledText::new(line.text.clone()).with_highlights(styles))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full()
        .py(dp(8.));
        let scroll = Scroll::Div(self.scroll.0.borrow().base_handle.clone());
        let body = scrolled(list, "build-output-scrollbar", scroll, theme)
            .bg(theme.bg_2)
            .font_family(theme.mono_font.clone())
            .text_size(dp(12.));
        let actions = match self.state {
            State::Building | State::Restarting => h_flex()
                .gap(dp(6.))
                .child(
                    button("build-hide", "Hide")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .tooltip("Keep building, and restart once it's built")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .child(
                    button("build-cancel", "Cancel build")
                        .small()
                        .disabled_because((self.state == State::Restarting).then_some("It's built"))
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                ),
            State::Failed | State::Cancelled => h_flex()
                .gap(dp(6.))
                .child(
                    button("build-close", "Close")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .child(
                    button("build-again", "Build again")
                        .small()
                        .style(ButtonStyle::Pop)
                        .on_click(cx.listener(|this, _, _, cx| this.run(cx))),
                ),
        };
        let footer = h_flex()
            .flex_none()
            .px(dp(20.))
            .py(dp(12.))
            .gap(dp(6.))
            .border_t_1()
            .border_color(theme.border_3)
            .child(
                button("build-copy", "Copy output")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
            )
            .child(div().flex_1())
            .child(actions);
        modal(ModalSize::Large, theme)
            .h(dp(560.))
            .track_focus(&self.focus)
            .child(header)
            .child(body)
            .child(footer)
    }
}

/// A line of cargo's output, with the colours and bold its terminal codes asked for.
struct Line {
    text: SharedString,
    /// Byte ranges of `text` and how they're drawn, where that isn't plain.
    styles: Vec<(Range<usize>, Style)>,
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Style {
    /// One of the terminal's eight colours, by its number: 1 red, 2 green, and so on.
    color: Option<u8>,
    bold: bool,
}

impl Line {
    fn plain(text: String) -> Self {
        Self {
            text: text.into(),
            styles: Vec::new(),
        }
    }

    /// `raw` without its terminal codes, keeping what their colours and bold covered. Other
    /// codes, as a link's, are left out.
    fn from_terminal(raw: &str) -> Self {
        let mut text = String::with_capacity(raw.len());
        let mut styles = Vec::new();
        let (mut style, mut since) = (Style::default(), 0);
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            match c {
                '\x1b' => match chars.next() {
                    // A control sequence, up to its final byte; `m` sets the style.
                    Some('[') => {
                        let mut params = String::new();
                        let mut last = None;
                        for c in chars.by_ref() {
                            if ('\x40'..='\x7e').contains(&c) {
                                last = Some(c);
                                break;
                            }
                            params.push(c);
                        }
                        let next = match last {
                            Some('m') => style.with(&params),
                            _ => style,
                        };
                        if next != style {
                            if style != Style::default() && since < text.len() {
                                styles.push((since..text.len(), style));
                            }
                            (style, since) = (next, text.len());
                        }
                    }
                    // An operating system command, as a hyperlink, ended by BEL or ESC \.
                    Some(']') => {
                        while let Some(c) = chars.next() {
                            if c == '\x07' {
                                break;
                            }
                            if c == '\x1b' {
                                chars.next();
                                break;
                            }
                        }
                    }
                    _ => {}
                },
                '\r' => {}
                c => text.push(c),
            }
        }
        if style != Style::default() && since < text.len() {
            styles.push((since..text.len(), style));
        }
        Self {
            text: text.into(),
            styles,
        }
    }
}

impl Style {
    /// This style changed by a Select Graphic Rendition's `params`. Rustc writes its bright
    /// colours as the first sixteen of 256, which draw as the eight here; colours past those,
    /// and backgrounds, draw plain.
    fn with(mut self, params: &str) -> Self {
        // An empty parameter is 0, a reset.
        let mut codes = params
            .split(';')
            .map(|code| code.parse::<u8>().unwrap_or(0));
        while let Some(code) = codes.next() {
            match code {
                0 => self = Self::default(),
                1 => self.bold = true,
                22 => self.bold = false,
                30..=37 => self.color = Some(code - 30),
                90..=97 => self.color = Some(code - 90),
                39 => self.color = None,
                38 | 48 => {
                    let color = match codes.next() {
                        Some(5) => codes.next().filter(|n| *n < 16).map(|n| n % 8),
                        // Red, green and blue.
                        Some(2) => {
                            codes.nth(2);
                            None
                        }
                        _ => None,
                    };
                    if code == 38 {
                        self.color = color;
                    }
                }
                _ => {}
            }
        }
        self
    }
}

/// The theme's colour for terminal colour `color`, read on `bg_2`.
fn terminal_color(color: u8, theme: &Theme) -> Hsla {
    match color {
        0 => theme.text_3,
        1 => theme.danger,
        2 => theme.safe,
        3 => theme.warn,
        4 => theme.blue,
        5 => theme.purple,
        6 => theme.pop,
        _ => theme.text_1,
    }
}

#[cfg(test)]
mod tests {
    use super::{Line, Style};

    fn styles(line: &Line) -> Vec<(&str, Style)> {
        (line.styles.iter())
            .filter_map(|(range, style)| Some((line.text.get(range.clone())?, *style)))
            .collect()
    }

    #[test]
    fn cargos_status_is_bold_green() {
        let line = Line::from_terminal("\x1b[1m\x1b[92m   Compiling\x1b[0m but-api v0.0.0");
        assert_eq!(line.text.as_ref(), "   Compiling but-api v0.0.0");
        let green = Style {
            color: Some(2),
            bold: true,
        };
        assert_eq!(
            styles(&line),
            [("   Compiling", green)],
            "bright green draws as green, and the reset ends it"
        );
    }

    #[test]
    fn rustcs_256_colour_error_is_red() {
        let line = Line::from_terminal(
            "\x1b[0m\x1b[1m\x1b[38;5;9merror[E0308]\x1b[0m\x1b[0m\x1b[1m: mismatched types\x1b[0m",
        );
        assert_eq!(line.text.as_ref(), "error[E0308]: mismatched types");
        let red = Style {
            color: Some(1),
            bold: true,
        };
        let bold = Style {
            color: None,
            bold: true,
        };
        assert_eq!(
            styles(&line),
            [("error[E0308]", red), (": mismatched types", bold)],
            "colour 9 of 256 is bright red, and resets between styles leave no empty runs"
        );
    }

    #[test]
    fn links_and_other_codes_are_left_out() {
        let line = Line::from_terminal("\x1b]8;;file:///src/x.rs\x1b\\x.rs\x1b]8;;\x07:1\x1b[K\r");
        assert_eq!(line.text.as_ref(), "x.rs:1");
        assert!(
            line.styles.is_empty(),
            "codes that aren't colours draw nothing"
        );
    }
}
