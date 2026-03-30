use std::convert::Infallible;

use but_testsupport::Sandbox;
use crossterm::event::*;
use gitbutler_operating_modes::OperatingMode;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use temp_env::with_var;

use crate::{
    args::OutputFormat,
    command::legacy::status::{
        StatusFlags, StatusOutput, StatusRenderMode, TuiLaunchOptions, build_status_context,
        build_status_output,
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
    let options = TuiLaunchOptions {
        debug: false,
        ..Default::default()
    };

    let status_ctx = async_runtime
        .block_on(build_status_context(
            &mut ctx,
            &mut out,
            &mode,
            flags,
            StatusRenderMode::Tui(options),
        ))
        .expect("failed to build status context");
    let mut lines = Vec::new();
    let mut status_output = StatusOutput::Buffer { lines: &mut lines };
    build_status_output(&mut ctx, &status_ctx, &mut status_output)
        .expect("failed to build status output");

    let app = App::new(lines, flags, options);
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

        with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
            with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
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
            });
        });

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
        let svg = backend_to_svg(self.0.terminal.backend());
        snapbox::assert_data_eq!(svg, expected);
        self
    }
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
            let normalize_hash_start = short_hash_start_for_cell(buffer, area, x, y);
            let normalize_hash_cell = normalize_hash_start.is_some();
            let normalize_hash_tail =
                normalize_hash_start.is_some_and(|start| x >= start.saturating_add(2));
            let normalize_volatile_id_cell = volatile_id_hex_prefix_cell(buffer, area, x, y);
            let symbol = if normalize_hash_cell || normalize_volatile_id_cell {
                "0"
            } else {
                symbol
            };

            let style = if normalize_hash_tail {
                format!("fill:{};opacity:0.75;", rgb_hex(default_fg))
            } else {
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

fn is_single_ascii_hex(symbol: &str) -> bool {
    symbol.chars().count() == 1 && symbol.chars().next().is_some_and(|c| c.is_ascii_hexdigit())
}

fn is_blue_bold_hex_cell(cell: &ratatui::buffer::Cell) -> bool {
    matches!(cell.fg, Color::Blue)
        && cell.modifier.contains(Modifier::BOLD)
        && is_single_ascii_hex(cell.symbol())
}

fn short_hash_start_for_cell(
    buffer: &ratatui::buffer::Buffer,
    area: ratatui::layout::Rect,
    x: u16,
    y: u16,
) -> Option<u16> {
    if !is_single_ascii_hex(buffer[(x, y)].symbol()) {
        return None;
    }

    let row_start = area.x;
    let row_end = area.x.saturating_add(area.width);
    let search_start = x.saturating_sub(6).max(row_start);

    for start in search_start..=x {
        let Some(end) = start.checked_add(6) else {
            continue;
        };
        if end >= row_end {
            continue;
        }

        if !is_blue_bold_hex_cell(&buffer[(start, y)])
            || !is_blue_bold_hex_cell(&buffer[(start.saturating_add(1), y)])
        {
            continue;
        }

        if (start..=end).all(|cell_x| is_single_ascii_hex(buffer[(cell_x, y)].symbol())) {
            return Some(start);
        }
    }

    None
}

fn volatile_id_hex_prefix_cell(
    buffer: &ratatui::buffer::Buffer,
    area: ratatui::layout::Rect,
    x: u16,
    y: u16,
) -> bool {
    if !is_single_ascii_hex(buffer[(x, y)].symbol()) {
        return false;
    }

    let row_start = area.x;
    let row_end = area.x.saturating_add(area.width);
    let search_start = x.saturating_sub(1).max(row_start);

    for start in search_start..=x {
        let Some(end) = start.checked_add(4) else {
            continue;
        };
        if end >= row_end {
            continue;
        }

        let first = &buffer[(start, y)];
        let second = &buffer[(start.saturating_add(1), y)];
        let colon = &buffer[(start.saturating_add(2), y)];
        let v = &buffer[(start.saturating_add(3), y)];
        let o = &buffer[(start.saturating_add(4), y)];

        let is_blue_bold = |cell: &ratatui::buffer::Cell| {
            matches!(cell.fg, Color::Blue) && cell.modifier.contains(Modifier::BOLD)
        };

        if is_blue_bold_hex_cell(first)
            && is_blue_bold_hex_cell(second)
            && is_blue_bold(colon)
            && colon.symbol() == ":"
            && is_blue_bold(v)
            && v.symbol() == "v"
            && is_blue_bold(o)
            && o.symbol() == "o"
            && x <= start.saturating_add(1)
        {
            return true;
        }
    }

    false
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
