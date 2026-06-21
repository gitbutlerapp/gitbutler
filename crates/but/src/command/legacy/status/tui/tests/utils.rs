use std::{convert::Infallible, path::PathBuf, sync::atomic::AtomicBool, time::Duration};

use but_testsupport::Sandbox;
use crossterm::event::*;
use gitbutler_operating_modes::OperatingMode;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use temp_env::with_vars;

use crate::{
    args::OutputFormat,
    command::legacy::status::{
        StatusFlags, StatusOutput, StatusRenderMode, TuiLaunchOptions, TuiOutcome, TuiRunOptions,
        build_status_context, build_status_output,
        tui::{
            App, BackstackEntry, EventPolling, Message, ReloadCause, TuiInputOutputChannel,
            render_loop_once,
        },
    },
    theme,
    tui::TerminalGuard,
    utils::{OutputChannel, WriteWithUtils},
};

pub(super) struct TestTui {
    pub(super) app: App,
    terminal: Terminal<TestBackend>,
    env: Option<Sandbox>,
    out: OutputChannel,
    mode: OperatingMode,
    width: u16,
    height: u16,
    svg_snapshot_comparison: Option<SvgSnapshotComparison>,
}

enum SvgSnapshotComparison {
    Html(PathBuf),
    Hint,
}

pub(super) struct TestTuiOptions {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) run_options: TuiRunOptions,
    pub(super) show_file_browser: bool,
}

impl Default for TestTuiOptions {
    fn default() -> Self {
        Self {
            width: 100,
            height: 20,
            run_options: Default::default(),
            show_file_browser: false,
        }
    }
}

pub(super) fn test_tui(env: Sandbox) -> TestTui {
    test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 20,
            ..Default::default()
        },
    )
}

pub(super) fn test_tui_with_options(env: Sandbox, options: TestTuiOptions) -> TestTui {
    let TestTuiOptions {
        width,
        height,
        run_options,
        show_file_browser,
    } = options;

    env.invoke_git("config user.name committer");
    env.invoke_git("config user.email committer@example.com");
    env.invoke_git("config gitoxide.commit.authorDate '2000-01-01 00:00:00 +0000'");
    env.invoke_git("config gitoxide.commit.committerDate '2000-01-01 00:00:00 +0000'");
    env.invoke_git("config gitbutler.testing.changeId 1");

    let mut ctx = env.context().expect("failed to create context");
    let mode = but_api::legacy::modes::operating_mode(&ctx)
        .expect("failed to get operating mode")
        .operating_mode;
    let mut out = OutputChannel::new(OutputFormat::Human);

    let flags = StatusFlags::all_false();
    let launch_options = TuiLaunchOptions {
        debug: false,
        ..Default::default()
    };

    let mut guard = ctx.exclusive_worktree_access();

    let format = out.format();
    let status_ctx = build_status_context(
        &mut ctx,
        guard.write_permission(),
        &mut out,
        format,
        &mode,
        flags,
        StatusRenderMode::Tui(launch_options),
    )
    .expect("failed to build status context");
    let mut lines = Vec::new();
    let mut status_output = StatusOutput::Buffer { lines: &mut lines };
    build_status_output(&ctx, &status_ctx, &mut status_output)
        .expect("failed to build status output");

    let app = App::new(lines, flags, launch_options, run_options, show_file_browser);
    let terminal =
        Terminal::new(TestBackend::new(width, height)).expect("failed to create test terminal");

    TestTui {
        app,
        terminal,
        env: Some(env),
        out,
        mode,
        width,
        height,
        svg_snapshot_comparison: None,
    }
}

pub(super) fn with_stable_commit_env<R>(closure: impl FnOnce() -> R) -> R {
    with_vars(
        [
            ("GIT_AUTHOR_DATE", Some("2000-01-01 00:00:00 +0000")),
            ("GIT_AUTHOR_EMAIL", Some("author@example.com")),
            ("GIT_AUTHOR_NAME", Some("author")),
            ("GIT_COMMITTER_DATE", Some("2000-01-01 00:00:00 +0000")),
            ("GIT_COMMITTER_EMAIL", Some("committer@example.com")),
            ("GIT_COMMITTER_NAME", Some("committer")),
            ("TZ", Some("UTC0")),
            ("GIT_CONFIG_COUNT", Some("5")),
            ("GIT_CONFIG_KEY_0", Some("commit.gpgsign")),
            ("GIT_CONFIG_VALUE_0", Some("false")),
            ("GIT_CONFIG_KEY_1", Some("tag.gpgsign")),
            ("GIT_CONFIG_VALUE_1", Some("false")),
            ("GIT_CONFIG_KEY_2", Some("init.defaultBranch")),
            ("GIT_CONFIG_VALUE_2", Some("main")),
            ("GIT_CONFIG_KEY_3", Some("protocol.file.allow")),
            ("GIT_CONFIG_VALUE_3", Some("always")),
            ("GIT_CONFIG_KEY_4", Some("gitbutler.testing.changeId")),
            ("GIT_CONFIG_VALUE_4", Some("1")),
        ],
        closure,
    )
}

impl TestTui {
    #[track_caller]
    pub(super) fn env(&self) -> &Sandbox {
        self.env.as_ref().unwrap()
    }

    #[track_caller]
    pub(super) fn reload(&mut self) -> TestTuiInputThenRenderResult<'_> {
        self.render_with_messages(
            None,
            Vec::from([Message::Reload(None, ReloadCause::Mutation)]),
        )
    }

    #[track_caller]
    pub(super) fn input_then_render<E>(&mut self, event: E) -> TestTuiInputThenRenderResult<'_>
    where
        E: InputEventPolling,
    {
        self.render_with_messages(event, Vec::new())
    }

    #[track_caller]
    pub(super) fn render_with_messages<E>(
        &mut self,
        event: E,
        mut messages: Vec<Message>,
    ) -> TestTuiInputThenRenderResult<'_>
    where
        E: EventPolling,
    {
        let mut other_messages = Vec::new();

        with_stable_commit_env(|| {
            let mut ctx = self.env().context().expect("failed to create context");
            let mut out = TestTuiInputOutputChannel(&mut self.out);
            render_loop_once(
                &mut self.app,
                &mut self.terminal,
                event,
                &mut messages,
                &mut other_messages,
                &AtomicBool::default(),
                &mut ctx,
                &mut out,
                &self.mode,
            )
            .unwrap();
        });

        TestTuiInputThenRenderResult(self)
    }

    #[track_caller]
    pub(super) fn recreate(mut self) -> Self {
        let env = self.env.take().expect(
            "env already removed?! This shouldn't happen, only TestTui::recreate removes the env",
        );
        self = test_tui_with_options(
            env,
            TestTuiOptions {
                width: self.width,
                height: self.height,
                ..Default::default()
            },
        );
        self
    }
}

impl Drop for TestTui {
    fn drop(&mut self) {
        use colored::Colorize;

        if self.env.is_none() {
            // `TestTui::recreate` was called, in which case we'll print the state of the new tui
            // when that is dropped
            return;
        }

        // Print the state of the terminal backend on test failures. If the test succeeds then
        // cargo discards the test output. This makes it easier to debug test failures because so
        // much of it depends on getting the cursor on the right line.

        let render_result = TestTuiInputThenRenderResult(self);
        let selected_row = render_result.selected_row().map(|row| row as usize);

        eprintln!("\nCurrent terminal state:");

        for (idx, line) in render_result.rendered_output().lines().enumerate() {
            let line = line.trim_matches('"');
            if selected_row.is_some_and(|row| row == idx) {
                colored::control::set_override(true);
                eprintln!(
                    "\"{}\"",
                    line.on_custom_color(colored::CustomColor {
                        r: 69,
                        g: 71,
                        b: 90
                    })
                );
                colored::control::unset_override();
            } else {
                eprintln!("\"{line}\"");
            }
        }

        match &self.svg_snapshot_comparison {
            Some(SvgSnapshotComparison::Html(path)) => eprintln!(
                "\nSVG snapshot comparison written to:\n  {}\n",
                path.display()
            ),
            Some(SvgSnapshotComparison::Hint) => eprintln!(
                "\nHint: set GITBUTLER_TUI_SVG_SNAPSHOT_HTML=1 to write an HTML comparison for SVG snapshot mismatches.\n"
            ),
            None => {}
        }
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
    pub(super) fn assert_rendered_contains(self, expected: &str) -> Self {
        let output = self.rendered_output();
        assert!(
            output.contains(expected),
            "expected rendered output to contain {expected:?}, got:\n{output}"
        );

        self
    }

    #[track_caller]
    #[allow(dead_code)]
    pub(super) fn assert_rendered_not_contains(self, expected: &str) -> Self {
        let output = self.rendered_output();
        assert!(
            !output.contains(expected),
            "expected rendered output to not contain {expected:?}, got:\n{output}"
        );

        self
    }

    pub(super) fn rendered_output(&self) -> String {
        self.0.terminal.backend().to_string()
    }

    /// We might not be able to find the selected row for example if we're in full screen details
    /// view.
    fn selected_row(&self) -> Option<u16> {
        let backend = self.0.terminal.backend();
        let buffer = backend.buffer();
        let area = *buffer.area();
        let selected_bg = theme::get()
            .selection_highlight
            .bg
            .expect("background must be set on selection_highlight");

        (area.y..area.y.saturating_add(area.height)).find(|&y| {
            (area.x..area.x.saturating_add(area.width)).any(|x| buffer[(x, y)].bg == selected_bg)
        })
    }

    #[track_caller]
    pub(super) fn assert_current_line_eq(self, expected: impl snapbox::IntoData) -> Self {
        let backend = self.0.terminal.backend();
        let buffer = backend.buffer();
        let area = *buffer.area();

        let selected_row = self
            .selected_row()
            .expect("failed to find selected row in rendered output");

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
        let svg = backend_to_svg(self.0.terminal.backend());
        self.0.svg_snapshot_comparison = write_svg_snapshot_comparison_if_enabled(
            &expected,
            &svg,
            std::panic::Location::caller(),
        );
        snapbox::assert_data_eq!(svg, expected);
        self
    }

    pub(super) fn take_outcome(self) -> Option<TuiOutcome> {
        self.0.app.outcome.take()
    }

    #[track_caller]
    pub(super) fn assert_backstack_eq(
        self,
        entries: impl IntoIterator<Item = BackstackEntry>,
    ) -> Self {
        let expected = entries.into_iter().collect::<Vec<_>>();
        let actual = self.0.app.backstack.iter().copied().collect::<Vec<_>>();
        if expected != actual {
            panic!("wrong backstack\n  expected: {expected:?}\n  actual: {actual:?}");
        }
        self
    }
}

fn write_svg_snapshot_comparison_if_enabled(
    expected: &snapbox::Data,
    actual_svg: &str,
    caller: &std::panic::Location<'_>,
) -> Option<SvgSnapshotComparison> {
    let expected_svg = expected.render()?;

    if expected_svg == actual_svg {
        return None;
    }

    if std::env::var_os("GITBUTLER_TUI_SVG_SNAPSHOT_HTML").is_none() {
        return Some(SvgSnapshotComparison::Hint);
    }

    match write_svg_snapshot_comparison_html(&expected_svg, actual_svg, caller) {
        Ok(path) => Some(SvgSnapshotComparison::Html(path)),
        Err(err) => {
            eprintln!("\nFailed to write SVG snapshot comparison HTML: {err}\n");
            None
        }
    }
}

fn write_svg_snapshot_comparison_html(
    expected_svg: &str,
    actual_svg: &str,
    caller: &std::panic::Location<'_>,
) -> std::io::Result<PathBuf> {
    let dir = tempfile::Builder::new()
        .prefix(&format!(
            "gitbutler-tui-svg-snapshot-{}-",
            svg_snapshot_file_stem(caller)
        ))
        .tempdir()?;

    let path = dir.path().join("comparison.html");
    std::fs::write(
        &path,
        format!(
            r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>Status TUI SVG snapshot mismatch</title>
<style>
body {{ font-family: sans-serif; background: #111; color: #eee; }}
.grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }}
.panel {{ background: #222; padding: 12px; overflow: auto; border: 1px solid #444; }}
h2 {{ margin-top: 0; }}
svg {{ background: black; }}
</style>
</head>
<body>
<h1>Status TUI SVG snapshot mismatch</h1>
<div class="grid">
  <section class="panel">
    <h2>Expected snapshot</h2>
    {expected_svg}
  </section>
  <section class="panel">
    <h2>Actual render</h2>
    {actual_svg}
  </section>
</div>
</body>
</html>
"#
        ),
    )?;

    let kept_dir = dir.keep();
    Ok(kept_dir.join("comparison.html"))
}

fn svg_snapshot_file_stem(caller: &std::panic::Location<'_>) -> String {
    let file = caller
        .file()
        .rsplit_once('/')
        .map_or_else(|| caller.file(), |(_, file)| file);
    let raw = format!("{file}-{}", caller.line());

    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn backend_to_svg(backend: &TestBackend) -> String {
    const CELL_WIDTH: u16 = 8;
    const CELL_HEIGHT: u16 = 18;
    const PADDING: u16 = 10;
    const FONT_SIZE: u16 = 14;

    let buffer = backend.buffer();
    let area = *buffer.area();

    let width = area.width * CELL_WIDTH + PADDING * 2;
    let height = area.height * CELL_HEIGHT + PADDING * 2;

    let default_fg = (0xcc, 0xcc, 0xcc);
    let default_bg = (0x00, 0x00, 0x00);

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
    ));
    svg.push_str(&format!(
        "  <rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"#000000\" />\n"
    ));

    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &buffer[(x, y)];
            let bg = color_to_rgb(cell.bg, default_bg);
            if bg != default_bg {
                let rect_x = PADDING + (x - area.x) * CELL_WIDTH;
                let rect_y = PADDING + (y - area.y) * CELL_HEIGHT;
                svg.push_str(&format!(
                    "  <rect x=\"{rect_x}\" y=\"{rect_y}\" width=\"{CELL_WIDTH}\" height=\"{CELL_HEIGHT}\" fill=\"{}\" />\n",
                    rgb_hex(bg)
                ));
            }
        }
    }

    for y in area.y..area.y.saturating_add(area.height) {
        let text_y = PADDING + (y - area.y + 1) * CELL_HEIGHT - 4;

        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &buffer[(x, y)];
            let symbol = cell.symbol();
            if symbol.is_empty() || symbol == " " {
                continue;
            }

            let mut fg = color_to_rgb(cell.fg, default_fg);
            let mut bg = color_to_rgb(cell.bg, default_bg);
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }

            let text_x = PADDING + (x - area.x) * CELL_WIDTH;
            let style = {
                let mut style = format!("fill:{};", rgb_hex(fg));
                if cell.modifier.contains(Modifier::BOLD) {
                    style.push_str("font-weight:bold;");
                }
                if cell.modifier.contains(Modifier::DIM) {
                    style.push_str("opacity:0.75;");
                }
                if cell.modifier.contains(Modifier::ITALIC) {
                    style.push_str("font-style:italic;");
                }
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    style.push_str("text-decoration:underline;");
                }
                if cell.modifier.contains(Modifier::CROSSED_OUT) {
                    style.push_str("text-decoration:line-through;");
                }
                style
            };

            svg.push_str(&format!(
                "  <text x=\"{text_x}\" y=\"{text_y}\" style=\"{style}\" font-family=\"Menlo, Monaco, 'Courier New', monospace\" font-size=\"{FONT_SIZE}\" xml:space=\"preserve\">{}</text>\n",
                escape_xml(symbol)
            ));
        }
    }

    svg.push_str("</svg>\n");
    svg
}

fn color_to_rgb(color: Color, default: (u8, u8, u8)) -> (u8, u8, u8) {
    match color {
        Color::Reset => default,
        Color::Black => (0x00, 0x00, 0x00),
        Color::Red => (0xaa, 0x00, 0x00),
        Color::Green => (0x00, 0xaa, 0x00),
        Color::Yellow => (0xaa, 0x55, 0x00),
        Color::Blue => (0x00, 0x00, 0xaa),
        Color::Magenta => (0xaa, 0x00, 0xaa),
        Color::Cyan => (0x00, 0xaa, 0xaa),
        Color::Gray => (0xaa, 0xaa, 0xaa),
        Color::DarkGray => (0x55, 0x55, 0x55),
        Color::LightRed => (0xff, 0x55, 0x55),
        Color::LightGreen => (0x55, 0xff, 0x55),
        Color::LightYellow => (0xff, 0xff, 0x55),
        Color::LightBlue => (0x55, 0x55, 0xff),
        Color::LightMagenta => (0xff, 0x55, 0xff),
        Color::LightCyan => (0x55, 0xff, 0xff),
        Color::White => (0xff, 0xff, 0xff),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(idx) => xterm_256_to_rgb(idx),
    }
}

fn xterm_256_to_rgb(idx: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];

    match idx {
        0..=15 => BASE[idx as usize],
        16..=231 => {
            let i = idx - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = i % 6;
            let to_channel = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (to_channel(r), to_channel(g), to_channel(b))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn rgb_hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl<const N: usize, T> EventPolling for [T; N]
where
    T: EventPolling<Error = Infallible>,
{
    type Error = Infallible;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(self.into_iter().flat_map(move |inner| {
            let Ok(iter) = inner.poll(timeout);
            iter
        }))
    }
}

impl EventPolling for Option<Event> {
    type Error = Infallible;

    fn poll(mut self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(self.take())
    }
}

pub(super) trait InputEventPolling: EventPolling {}

impl<const N: usize, T> InputEventPolling for [T; N] where
    T: InputEventPolling + EventPolling<Error = Infallible>
{
}

impl InputEventPolling for KeyCode {}

impl EventPolling for KeyCode {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok([Event::Key(KeyEvent {
            code: self,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })])
    }
}

impl InputEventPolling for (KeyModifiers, KeyCode) {}

impl EventPolling for (KeyModifiers, KeyCode) {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok([Event::Key(KeyEvent {
            code: self.1,
            modifiers: self.0,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })])
    }
}

impl InputEventPolling for (KeyModifiers, char) {}

impl EventPolling for (KeyModifiers, char) {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok([Event::Key(KeyEvent {
            code: KeyCode::Char(self.1),
            modifiers: self.0,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })])
    }
}

impl InputEventPolling for char {}

impl EventPolling for char {
    type Error = Infallible;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        KeyCode::Char(self).poll(timeout)
    }
}

impl InputEventPolling for &str {}

impl EventPolling for &str {
    type Error = Infallible;

    fn poll(self, _timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
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

impl InputEventPolling for String {}

impl EventPolling for String {
    type Error = Infallible;

    fn poll(self, timeout: Duration) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        Ok(self.as_str().poll(timeout)?.into_iter().collect::<Vec<_>>())
    }
}

struct TestTuiInputOutputChannel<'a>(&'a mut OutputChannel);

impl crate::command::legacy::status::tui::private::Sealed for TestTuiInputOutputChannel<'_> {}

impl std::fmt::Write for TestTuiInputOutputChannel<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_str(s)
    }
}

impl WriteWithUtils for TestTuiInputOutputChannel<'_> {
    fn truncate_if_unpaged(&self, text: &str, max_width: usize) -> String {
        self.0.truncate_if_unpaged(text, max_width)
    }

    fn is_paged(&self) -> bool {
        self.0.is_paged()
    }
}

impl TuiInputOutputChannel for TestTuiInputOutputChannel<'_> {
    fn prompt_single_line(&mut self, _prompt: &str) -> anyhow::Result<Option<String>> {
        panic!("cannot get input in tests")
    }
}
