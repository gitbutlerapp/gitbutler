use std::convert::Infallible;

use but_testsupport::Sandbox;
use crossterm::event::*;
use gitbutler_operating_modes::OperatingMode;
use ratatui::{Terminal, backend::TestBackend, widgets::List};

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
    terminal_width: u16,
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
        terminal_width: width,
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
        let selected_line = self
            .0
            .app
            .cursor
            .selected_line(&self.0.app.status_lines)
            .expect("failed to get selected line");
        let list_item = self
            .0
            .app
            .render_status_list_item(selected_line, true)
            .into_iter()
            .next()
            .expect("selected line should render at least one list item");
        let mut terminal = Terminal::new(TestBackend::new(self.0.terminal_width, 1))
            .expect("failed to create test terminal");
        terminal
            .draw(|frame| frame.render_widget(List::new([list_item]), frame.area()))
            .expect("failed to render current line");
        let output = terminal.backend().to_string();
        let line = output
            .lines()
            .next()
            .expect("failed to get rendered current line")
            .trim_start_matches('"')
            .trim_end_matches('"')
            .trim_end();

        let actual = snapbox::IntoData::into_data(line);
        let actual = actual.render().expect("current line should render as text");

        let expected = snapbox::IntoData::into_data(expected);

        snapbox::assert_data_eq!(actual, expected);

        self
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
