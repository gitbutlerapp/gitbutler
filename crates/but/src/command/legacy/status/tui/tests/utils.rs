use std::convert::Infallible;

use but_testsupport::Sandbox;
use crossterm::event::*;
use gitbutler_operating_modes::OperatingMode;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};

use crate::{
    args::OutputFormat,
    command::legacy::status::{
        StatusFlags, StatusOutput, StatusRenderMode, build_status_context, build_status_output,
        tui::{App, EventPolling, Message, render_loop_once},
    },
    tui::TerminalGuard,
    utils::OutputChannel,
};

pub(super) struct TestTui {
    pub(super) app: App,
    terminal: Terminal<TestBackend>,
    pub(super) env: Sandbox,
    out: OutputChannel,
    mode: OperatingMode,
    async_runtime: tokio::runtime::Runtime,
}

pub(super) fn test_tui(env: Sandbox) -> TestTui {
    test_tui_with_size(env, 100, 20)
}

pub(super) fn test_tui_with_size(env: Sandbox, width: u16, height: u16) -> TestTui {
    let async_runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("failed to build async runtime");

    env.invoke_git("config user.name committer");
    env.invoke_git("config user.email committer@example.com");

    let mut ctx = env.context().expect("failed to create context");
    let mode = but_api::legacy::modes::operating_mode(&ctx)
        .expect("failed to get operating mode")
        .operating_mode;
    let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::Human);

    let flags = StatusFlags::all_false();
    let debug = false;

    let status_ctx = async_runtime
        .block_on(build_status_context(
            &mut ctx,
            &mut out,
            &mode,
            flags,
            StatusRenderMode::Tui { debug },
        ))
        .expect("failed to build status context");
    let mut lines = Vec::new();
    let mut status_output = StatusOutput::Buffer { lines: &mut lines };
    build_status_output(&mut ctx, &status_ctx, &mut status_output)
        .expect("failed to build status output");

    let app = App::new(lines, flags, debug);
    let terminal =
        Terminal::new(TestBackend::new(width, height)).expect("failed to create test terminal");

    TestTui {
        app,
        terminal,
        env,
        out,
        mode,
        async_runtime,
    }
}

impl TestTui {
    pub(super) fn input_then_render<E>(&mut self, event: E) -> TestTuiInputThenRenderResult<'_>
    where
        E: EventPolling,
    {
        self.render_with_messages(event, Vec::from([Message::Reload(None)]))
    }

    pub(super) fn render_with_messages<E>(
        &mut self,
        event: E,
        mut messages: Vec<Message>,
    ) -> TestTuiInputThenRenderResult<'_>
    where
        E: EventPolling,
    {
        let mut ctx = self.env.context().expect("failed to create context");
        let mut other_messages = Vec::new();

        self.async_runtime
            .block_on(render_loop_once(
                &mut self.app,
                &mut self.terminal,
                event,
                &mut messages,
                &mut other_messages,
                &mut ctx,
                &mut self.out,
                &self.mode,
            ))
            .unwrap();

        TestTuiInputThenRenderResult(self)
    }
}

impl TerminalGuard for Terminal<TestBackend> {
    type Backend = TestBackend;

    type SuspendGuard<'a> = ();

    fn suspend(&mut self) -> anyhow::Result<Self::SuspendGuard<'_>> {
        Ok(())
    }

    fn terminal_mut(&mut self) -> &mut Terminal<Self::Backend> {
        self
    }
}

pub(super) struct TestTuiInputThenRenderResult<'a>(&'a mut TestTui);

impl TestTuiInputThenRenderResult<'_> {
    #[track_caller]
    pub(super) fn assert_rendered_eq(self, expected: snapbox::Data) -> Self {
        snapbox::assert_data_eq!(self.0.terminal.backend().to_string(), expected);

        self
    }

    #[track_caller]
    pub(super) fn assert_rendered_contains(self, expected: &str) -> Self {
        let output = self.0.terminal.backend().to_string();
        assert!(
            output.contains(expected),
            "expected rendered output to contain {expected:?}, got:\n{output}"
        );

        self
    }

    #[track_caller]
    pub(super) fn assert_current_line_eq(self, expected: impl snapbox::IntoData) -> Self {
        let backend = self.0.terminal.backend();
        let buffer = backend.buffer();
        let area = *buffer.area();
        let selected_bg = super::super::CURSOR_BG;

        let selected_row = (area.y..area.y.saturating_add(area.height))
            .find(|&y| {
                (area.x..area.x.saturating_add(area.width))
                    .any(|x| buffer[(x, y)].bg == selected_bg)
            })
            .unwrap_or_else(|| {
                panic!("failed to find selected row in rendered output:\n{backend}")
            });

        let mut line = String::new();
        for x in area.x..area.x.saturating_add(area.width) {
            line.push_str(buffer[(x, selected_row)].symbol());
        }
        let line = line.trim_end();

        let actual = snapbox::IntoData::into_data(line);
        let actual = actual.render().expect("current line should render as text");

        let expected = snapbox::IntoData::into_data(expected);

        snapbox::assert_data_eq!(actual, expected);

        self
    }

    #[track_caller]
    pub(super) fn assert_rendered_term_svg_eq(self, expected: snapbox::Data) -> Self {
        let ansi = backend_to_ansi(self.0.terminal.backend());
        snapbox::assert_data_eq!(ansi, expected);
        self
    }
}

fn backend_to_ansi(backend: &TestBackend) -> String {
    let buffer = backend.buffer();
    let area = *buffer.area();

    let mut out = String::new();

    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &buffer[(x, y)];
            out.push_str("\x1b[");
            out.push_str(&ansi_sgr_for_cell(cell));
            out.push('m');
            out.push_str(cell.symbol());
        }
        out.push_str("\x1b[0m\n");
    }

    out
}

fn ansi_sgr_for_cell(cell: &ratatui::buffer::Cell) -> String {
    let mut codes = Vec::new();

    codes.push("0".to_owned());

    if cell.modifier.contains(Modifier::BOLD) {
        codes.push("1".to_owned());
    }
    if cell.modifier.contains(Modifier::DIM) {
        codes.push("2".to_owned());
    }
    if cell.modifier.contains(Modifier::ITALIC) {
        codes.push("3".to_owned());
    }
    if cell.modifier.contains(Modifier::UNDERLINED) {
        codes.push("4".to_owned());
    }
    if cell.modifier.contains(Modifier::REVERSED) {
        codes.push("7".to_owned());
    }
    if cell.modifier.contains(Modifier::CROSSED_OUT) {
        codes.push("9".to_owned());
    }

    codes.push(ansi_color_code(cell.fg, true));
    codes.push(ansi_color_code(cell.bg, false));

    codes.join(";")
}

fn ansi_color_code(color: Color, is_foreground: bool) -> String {
    let (base, bright_base) = if is_foreground { (30, 90) } else { (40, 100) };

    match color {
        Color::Reset => {
            if is_foreground {
                "39".to_owned()
            } else {
                "49".to_owned()
            }
        }
        Color::Black => base.to_string(),
        Color::Red => (base + 1).to_string(),
        Color::Green => (base + 2).to_string(),
        Color::Yellow => (base + 3).to_string(),
        Color::Blue => (base + 4).to_string(),
        Color::Magenta => (base + 5).to_string(),
        Color::Cyan => (base + 6).to_string(),
        Color::Gray => (base + 7).to_string(),
        Color::DarkGray => bright_base.to_string(),
        Color::LightRed => (bright_base + 1).to_string(),
        Color::LightGreen => (bright_base + 2).to_string(),
        Color::LightYellow => (bright_base + 3).to_string(),
        Color::LightBlue => (bright_base + 4).to_string(),
        Color::LightMagenta => (bright_base + 5).to_string(),
        Color::LightCyan => (bright_base + 6).to_string(),
        Color::White => (bright_base + 7).to_string(),
        Color::Rgb(r, g, b) => {
            let prefix = if is_foreground { 38 } else { 48 };
            format!("{prefix};2;{r};{g};{b}")
        }
        Color::Indexed(idx) => {
            let prefix = if is_foreground { 38 } else { 48 };
            format!("{prefix};5;{idx}")
        }
    }
}

impl<const N: usize, T> EventPolling for [T; N]
where
    T: EventPolling<Error = Infallible>,
{
    type Error = Infallible;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(self.into_iter().flat_map(|inner| {
            let Ok(iter) = inner.poll();
            iter
        }))
    }
}

impl EventPolling for Option<Event> {
    type Error = Infallible;

    fn poll(mut self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(self.take())
    }
}

impl EventPolling for KeyCode {
    type Error = Infallible;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok([Event::Key(KeyEvent {
            code: self,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })])
    }
}

impl EventPolling for (KeyModifiers, KeyCode) {
    type Error = Infallible;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok([Event::Key(KeyEvent {
            code: self.1,
            modifiers: self.0,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })])
    }
}

impl EventPolling for char {
    type Error = Infallible;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        KeyCode::Char(self).poll()
    }
}

impl EventPolling for &str {
    type Error = Infallible;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(self.chars().map(KeyCode::Char).map(|code| {
            Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })
        }))
    }
}
