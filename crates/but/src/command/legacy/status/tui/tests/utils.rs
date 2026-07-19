use std::{convert::Infallible, path::PathBuf, sync::Arc, time::Duration};

use but_testsupport::Sandbox;
use crossterm::event::*;
use gitbutler_operating_modes::OperatingMode;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier, Style},
};
use temp_env::with_vars;

use crate::{
    args::OutputFormat,
    command::legacy::status::{
        StatusFlags, StatusOutput, StatusRenderMode, TuiLaunchOptions, TuiOutcome, TuiRunOptions,
        build_status_context, build_status_output,
        tui::{
            App, BackstackEntry, EventPolling, Message, ReloadCause, TuiInputOutputChannel,
            copy_selection_picker::Clipboard, render_loop_once,
        },
    },
    theme::Paint,
    tui::TerminalGuard,
    utils::{OutputChannel, WriteWithUtils},
};

use super::super::{mode::Mode, render::status_layout};

pub struct TestTui {
    app: App,
    terminal: Terminal<TestBackend>,
    env: Option<Sandbox>,
    out: OutputChannel,
    mode: OperatingMode,
    width: u16,
    height: u16,
    svg_snapshot_comparison: Option<SvgSnapshotComparison>,
    clipboard_text: Arc<std::sync::Mutex<String>>,
}

enum SvgSnapshotComparison {
    Html(PathBuf),
    Hint,
}

pub struct TestTuiOptions {
    pub width: u16,
    pub height: u16,
    pub run_options: TuiRunOptions,
    pub show_file_browser: bool,
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

pub fn test_tui(env: Sandbox) -> TestTui {
    test_tui_with_options(env, TestTuiOptions::default())
}

pub fn test_tui_with_options(env: Sandbox, options: TestTuiOptions) -> TestTui {
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

    let mut ctx = env.context();
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

    let incoming_out_of_band_messages = Vec::new();
    let head_sha = super::super::operations::head_sha(&mut ctx).expect("failed to read HEAD");

    let (clipboard, clipboard_text) = Clipboard::test();

    let app = App::new(
        lines,
        flags,
        launch_options,
        run_options,
        show_file_browser,
        incoming_out_of_band_messages,
        head_sha,
        clipboard,
    );
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
        clipboard_text,
    }
}

pub fn with_stable_commit_env<R>(closure: impl FnOnce() -> R) -> R {
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
    pub fn env(&self) -> &Sandbox {
        self.env.as_ref().unwrap()
    }

    #[track_caller]
    pub fn reload(&mut self) -> TestTuiInputThenRenderResult<'_> {
        self.render_with_messages(
            None,
            Vec::from([Message::Reload(None, ReloadCause::Mutation)]),
        )
    }

    #[track_caller]
    pub fn input<E>(&mut self, event: E) -> TestTuiInputThenRenderResult<'_>
    where
        E: InputEventPolling,
    {
        self.render_with_messages(event, Vec::new())
    }

    /// Lower level utility method that generally shouldn't be used. Prefer [`TestTui::input`] or
    /// [`TestTui::reload`] instead.
    #[doc(hidden)]
    #[track_caller]
    pub fn render_with_messages<E>(
        &mut self,
        event: E,
        mut messages: Vec<Message>,
    ) -> TestTuiInputThenRenderResult<'_>
    where
        E: EventPolling,
    {
        let mut other_messages = Vec::new();

        with_stable_commit_env(|| {
            let mut ctx = self.env().context();
            let mut out = TestTuiInputOutputChannel(&mut self.out);
            let mut events = Vec::with_capacity(1);
            render_loop_once(
                &mut self.app,
                &mut self.terminal,
                event,
                &mut events,
                &mut messages,
                &mut other_messages,
                &mut ctx,
                &mut out,
                &self.mode,
            )
            .unwrap();
        });

        TestTuiInputThenRenderResult(self)
    }

    #[track_caller]
    pub fn recreate(mut self) -> Self {
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
        if self.env.is_none() {
            // `TestTui::recreate` was called, in which case we'll print the state of the new tui
            // when that is dropped
            return;
        }

        // Print the state of the terminal backend on test failures. If the test succeeds then
        // cargo discards the test output. This makes it easier to debug test failures because so
        // much of it depends on getting the cursor on the right line.

        let render_result = TestTuiInputThenRenderResult(self);

        eprintln!("\nCurrent terminal state:");

        for (idx, line) in render_result.rendered_output().lines().enumerate() {
            let line = line.trim_matches('"');
            eprintln!(
                "\"{}\"",
                render_result.highlighted_debug_line(idx as u16, line)
            );
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

pub struct TestTuiInputThenRenderResult<'a>(&'a mut TestTui);

impl TestTuiInputThenRenderResult<'_> {
    #[track_caller]
    pub fn assert_rendered_contains(self, expected: &str) -> Self {
        let output = self.rendered_output();
        assert!(
            output.contains(expected),
            "expected rendered output to contain {expected:?}, got:\n{output}"
        );

        self
    }

    #[track_caller]
    #[allow(dead_code)]
    pub fn assert_rendered_not_contains(self, expected: &str) -> Self {
        let output = self.rendered_output();
        assert!(
            !output.contains(expected),
            "expected rendered output to not contain {expected:?}, got:\n{output}"
        );

        self
    }

    pub fn rendered_output(&self) -> String {
        self.0.terminal.backend().to_string()
    }

    fn highlighted_debug_line(&self, y: u16, rendered_line: &str) -> String {
        let backend = self.0.terminal.backend();
        let buffer = backend.buffer();
        let area = *buffer.area();
        if y >= area.height {
            return rendered_line.to_owned();
        }

        let mut rendered = String::new();
        let mut segment = String::new();
        let mut segment_style = None;

        colored::control::set_override(true);
        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &buffer[(x, y)];
            let style = Style::default()
                .fg(cell.fg)
                .bg(cell.bg)
                .add_modifier(cell.modifier);
            if segment_style.is_some_and(|current| current != style) {
                flush_styled_debug_line_segment(&mut rendered, &mut segment, segment_style);
                segment_style = Some(style);
            } else if segment_style.is_none() {
                segment_style = Some(style);
            }
            segment.push_str(cell.symbol());
        }
        flush_styled_debug_line_segment(&mut rendered, &mut segment, segment_style);
        colored::control::unset_override();

        rendered.trim_end().to_owned()
    }

    /// We might not be able to find the selected row for example if we're in full screen details
    /// view, where the status cursor exists but the status list is not rendered.
    fn selected_status_row(&self) -> Option<u16> {
        if matches!(&*self.0.app.mode, Mode::Details(details_mode) if details_mode.full_screen) {
            return None;
        }

        let buffer = self.0.terminal.backend().buffer();
        let terminal_area = *buffer.area();
        let main_content_area = ratatui::layout::Rect {
            height: terminal_area.height.saturating_sub(1),
            ..terminal_area
        };
        let status_inner_area = status_layout(&self.0.app, main_content_area).status_area;

        let cursor_index = self.0.app.cursor.index();
        let scroll_top = self.0.app.status_scroll.top();
        if cursor_index < scroll_top {
            return None;
        }

        let row_offset = cursor_index - scroll_top;
        if row_offset >= status_inner_area.height as usize {
            return None;
        }

        Some(status_inner_area.y + row_offset as u16)
    }

    #[track_caller]
    pub fn assert_current_line_eq(self, expected: impl snapbox::IntoData) -> Self {
        let backend = self.0.terminal.backend();
        let buffer = backend.buffer();
        let area = *buffer.area();

        let selected_row = self
            .selected_status_row()
            .expect("failed to find selected status row in rendered output");

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
    pub fn assert_rendered_term_svg_eq(self, expected: snapbox::Data) -> Self {
        let svg = backend_to_svg(self.0.terminal.backend());
        self.0.svg_snapshot_comparison = write_svg_snapshot_comparison_if_enabled(
            &expected,
            &svg,
            std::panic::Location::caller(),
        );
        snapbox::assert_data_eq!(svg, expected);
        self
    }

    pub fn take_outcome(self) -> Option<TuiOutcome> {
        self.0.app.outcome.take()
    }

    #[track_caller]
    pub fn assert_backstack_eq(self, entries: impl IntoIterator<Item = BackstackEntry>) -> Self {
        let expected = entries.into_iter().collect::<Vec<_>>();
        let actual = self.0.app.backstack.iter().copied().collect::<Vec<_>>();
        if expected != actual {
            panic!("wrong backstack\n  expected: {expected:?}\n  actual: {actual:?}");
        }
        self
    }

    #[track_caller]
    pub fn assert_copied_text_eq(self, expected: impl AsRef<str>) -> Self {
        let actual = self.0.clipboard_text.lock().unwrap();
        assert_eq!(actual.as_str(), expected.as_ref());
        drop(actual);
        self
    }
}

fn flush_styled_debug_line_segment(
    rendered: &mut String,
    segment: &mut String,
    style: Option<Style>,
) {
    if let Some(style) = style {
        rendered.push_str(&style.paint(&*segment).to_string());
        segment.clear();
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
.panel {{ background: #222; padding: 12px; overflow: auto; border: 1px solid #444; }}
.controls {{ display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }}
.controls input {{ width: min(420px, 60vw); }}
.controls span {{ min-width: 8ch; }}
.controls span:last-child {{ text-align: right; }}
.overlay {{ position: relative; display: inline-block; background: black; line-height: 0; }}
.overlay svg {{ display: block; background: black; }}
.overlay svg + svg {{ position: absolute; inset: 0; opacity: 0.5; pointer-events: none; }}
h2 {{ margin-top: 0; }}
svg {{ background: black; }}
</style>
</head>
<body>
<h1>Status TUI SVG snapshot mismatch</h1>
<section class="panel">
  <h2>Overlay comparison</h2>
  <div class="controls">
    <span>Expected</span>
    <input id="actual-opacity" aria-label="Blend between expected and actual render" type="range" min="0" max="100" value="50">
    <span>Actual</span>
  </div>
  <div class="overlay">
    {expected_svg}
    {actual_svg}
  </div>
</section>
<script>
const actualSvg = document.querySelector('.overlay svg + svg');
const opacityInput = document.querySelector('#actual-opacity');
function updateActualOpacity() {{
  actualSvg.style.opacity = opacityInput.value / 100;
}}
opacityInput.addEventListener('input', updateActualOpacity);
updateActualOpacity();
</script>
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
        let mut x = area.x;
        let row_end = area.x.saturating_add(area.width);
        while x < row_end {
            let bg = color_to_rgb(buffer[(x, y)].bg, default_bg);
            if bg == default_bg {
                x += 1;
                continue;
            }

            let run_start = x;
            x += 1;
            while x < row_end && color_to_rgb(buffer[(x, y)].bg, default_bg) == bg {
                x += 1;
            }

            let rect_x = PADDING + (run_start - area.x) * CELL_WIDTH;
            let rect_y = PADDING + (y - area.y) * CELL_HEIGHT;
            let rect_width = (x - run_start) * CELL_WIDTH;
            svg.push_str(&format!(
                "  <rect x=\"{rect_x}\" y=\"{rect_y}\" width=\"{rect_width}\" height=\"{CELL_HEIGHT}\" fill=\"{}\" />\n",
                rgb_hex(bg)
            ));
        }
    }

    svg.push_str(&format!(
        "  <g font-family=\"Menlo, Monaco, 'Courier New', monospace\" font-size=\"{FONT_SIZE}\" xml:space=\"preserve\">\n"
    ));
    for y in area.y..area.y.saturating_add(area.height) {
        let text_y = PADDING + (y - area.y + 1) * CELL_HEIGHT - 4;
        let row_end = area.x.saturating_add(area.width);
        let mut x = area.x;

        while x < row_end {
            let cell = &buffer[(x, y)];
            let symbol = cell.symbol();
            if symbol.is_empty() || symbol == " " {
                x += 1;
                continue;
            }

            let style = svg_text_style(cell.fg, cell.bg, cell.modifier, default_fg, default_bg);
            let mut positions = vec![(PADDING + (x - area.x) * CELL_WIDTH).to_string()];
            let mut symbols = symbol.to_owned();
            let can_join = symbol.chars().count() == 1;
            x += 1;

            if can_join {
                while x < row_end {
                    let next = &buffer[(x, y)];
                    let next_symbol = next.symbol();
                    if next_symbol.is_empty()
                        || next_symbol == " "
                        || next_symbol.chars().count() != 1
                        || svg_text_style(next.fg, next.bg, next.modifier, default_fg, default_bg)
                            != style
                    {
                        break;
                    }

                    positions.push((PADDING + (x - area.x) * CELL_WIDTH).to_string());
                    symbols.push_str(next_symbol);
                    x += 1;
                }
            }

            let positions = positions.join(" ");
            svg.push_str(&format!(
                "    <text x=\"{positions}\" y=\"{text_y}\" style=\"{style}\">{}</text>\n",
                escape_xml(&symbols)
            ));
        }
    }
    svg.push_str("  </g>\n</svg>\n");
    svg
}

fn svg_text_style(
    foreground: Color,
    background: Color,
    modifier: Modifier,
    default_foreground: (u8, u8, u8),
    default_background: (u8, u8, u8),
) -> String {
    let mut foreground = color_to_rgb(foreground, default_foreground);
    let mut background = color_to_rgb(background, default_background);
    if modifier.contains(Modifier::REVERSED) {
        std::mem::swap(&mut foreground, &mut background);
    }

    let mut style = format!("fill:{};", rgb_hex(foreground));
    if modifier.contains(Modifier::BOLD) {
        style.push_str("font-weight:bold;");
    }
    if modifier.contains(Modifier::DIM) {
        style.push_str("opacity:0.75;");
    }
    if modifier.contains(Modifier::ITALIC) {
        style.push_str("font-style:italic;");
    }
    if modifier.contains(Modifier::UNDERLINED) {
        style.push_str("text-decoration:underline;");
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        style.push_str("text-decoration:line-through;");
    }
    style
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

    fn poll_into(self, timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        for inner in self {
            inner.poll_into(timeout, events)?;
        }
        Ok(())
    }
}

impl EventPolling for Option<Event> {
    type Error = Infallible;

    fn poll_into(mut self, _timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        events.extend(self.take());
        Ok(())
    }
}

pub trait InputEventPolling: EventPolling {}

impl<const N: usize, T> InputEventPolling for [T; N] where
    T: InputEventPolling + EventPolling<Error = Infallible>
{
}

impl InputEventPolling for KeyCode {}

impl EventPolling for KeyCode {
    type Error = Infallible;

    fn poll_into(self, _timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        events.push(Event::Key(KeyEvent {
            code: self,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
        Ok(())
    }
}

impl InputEventPolling for (KeyModifiers, KeyCode) {}

impl EventPolling for (KeyModifiers, KeyCode) {
    type Error = Infallible;

    fn poll_into(self, _timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        events.push(Event::Key(KeyEvent {
            code: self.1,
            modifiers: self.0,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
        Ok(())
    }
}

impl InputEventPolling for (KeyModifiers, char) {}

impl EventPolling for (KeyModifiers, char) {
    type Error = Infallible;

    fn poll_into(self, _timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        events.push(Event::Key(KeyEvent {
            code: KeyCode::Char(self.1),
            modifiers: self.0,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
        Ok(())
    }
}

impl InputEventPolling for char {}

impl EventPolling for char {
    type Error = Infallible;

    fn poll_into(self, timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        KeyCode::Char(self).poll_into(timeout, events)
    }
}

impl InputEventPolling for &str {}

impl EventPolling for &str {
    type Error = Infallible;

    fn poll_into(self, _timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        events.extend(self.chars().map(KeyCode::Char).map(|code| {
            Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })
        }));
        Ok(())
    }
}

impl InputEventPolling for String {}

impl EventPolling for String {
    type Error = Infallible;

    fn poll_into(self, timeout: Duration, events: &mut Vec<Event>) -> Result<(), Self::Error> {
        self.as_str().poll_into(timeout, events)
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
