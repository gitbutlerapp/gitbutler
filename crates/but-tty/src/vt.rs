//! Rustty's terminal, input encoders, and the frames drawn from it.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
#[cfg(unix)]
use rustty_vt::formatter::{Format, TerminalExtra};
use rustty_vt::program_status;
use rustty_vt::query::{self, ColorScheme};
use rustty_vt::render::{CursorKey, RenderCursor, RowKey, ViewKey};
use rustty_vt::selection::DEFAULT_WORD_BOUNDARIES;
use rustty_vt::selection_gesture as gesture;
use rustty_vt::{
    Effect, EffectHandler, GridPoint, KeyEncodeOptions, KeyEvent, MouseEncodeOptions, MouseEvent,
    ScrollbackLimits, Terminal, clipboard,
};

use super::{Colors, CursorVisualStyle, Key, Mods, RgbColor, Size, Underline, mouse};

/// How a run of cells draws; its background only where it isn't the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: RgbColor,
    pub bg: Option<RgbColor>,
    pub bold: bool,
    pub underline: Underline,
    pub strikethrough: bool,
    pub faint: bool,
}

/// A row's text, in runs of cells that draw alike.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub runs: Vec<Run>,
}

/// Cells side by side that draw alike. A wide character is a run of its own, so each of a
/// run's characters takes one column but for that one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The bytes of the row's text it covers.
    pub text: Range<usize>,
    /// The column it starts at, and those it takes.
    pub columns: Range<u16>,
    pub style: Style,
    /// What its cells link to, as the program running set it (OSC 8).
    pub link: Option<Arc<str>>,
}

/// The cursor's cell, by column and row, with the text under it and the shape the program
/// last set (DECSCUSR), as nvim's bar in insert mode.
#[derive(Clone, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub text: String,
    pub style: CursorVisualStyle,
}

/// What a frame draws. A row that didn't change is the one the frame before drew.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub rows: Vec<Arc<Row>>,
    pub cursor: Option<Cursor>,
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub cursor_color: RgbColor,
}

/// What the program running says it's doing, by OSC 7501 or a progress report (OSC 9;4).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Status {
    #[default]
    Idle,
    Working,
    /// It can't go on until the user acts, for the reason given, if it gave one.
    Blocked(String),
    /// It finished, for the user to look at.
    Done,
    /// It stopped, for the reason given, if it gave one.
    Failed(String),
}

impl Status {
    /// The root record of a report; records beneath it are parts of the work.
    fn reported(report: program_status::Report) -> Option<Self> {
        if report.id.is_some() {
            return None;
        }
        Some(match report.state {
            program_status::State::Idle | program_status::State::Clear => Self::Idle,
            program_status::State::Working => Self::Working,
            program_status::State::Blocked => Self::Blocked(report.message),
            program_status::State::Done => Self::Done,
            program_status::State::Error => Self::Failed(report.message),
        })
    }

    /// Keep the outcome until a progress report removes the task.
    fn progressed(state: u8, ending: &mut Self) -> Option<Self> {
        match state {
            0 => Some(std::mem::take(ending)),
            1 | 3 => {
                if *ending == Self::Idle {
                    *ending = Self::Done;
                }
                Some(Self::Working)
            }
            2 => {
                *ending = Self::Failed(String::new());
                Some(Self::Working)
            }
            _ => None,
        }
    }
}

/// A program that never ends synchronized output must not freeze its display.
const HOLD_TIMEOUT: Duration = Duration::from_secs(1);

/// The emulator and what it's read and written with.
pub(crate) struct Vt {
    term: Terminal,
    drawer: Drawer,
    hold: Option<Hold>,
    capturing: bool,
    selecting: gesture::SelectionGesture,
    /// The synthetic click time preserves the platform's repeat classification.
    pressed: Duration,
    last_mouse_cell: Option<[u16; 2]>,
    replies: Vec<u8>,
    retitled: bool,
    reported: Option<Status>,
    reports_status: bool,
    progress_ending: Status,
    copied: Option<String>,
    size: Size,
}

impl Vt {
    pub(crate) fn new(size: Size) -> Self {
        let mut term = Terminal::with_limits(
            size.cols,
            size.rows,
            ScrollbackLimits {
                bytes: Some(10_000_000),
                lines: None,
            },
        );
        term.set_graphics_limit(0);
        term.program_status_events = true;
        term.set_pixel_size(
            u32::from(size.cols) * u32::from(size.cell_width),
            u32::from(size.rows) * u32::from(size.cell_height),
        );
        Self {
            term,
            drawer: Drawer::default(),
            hold: None,
            capturing: true,
            selecting: gesture::SelectionGesture::default(),
            pressed: Duration::ZERO,
            last_mouse_cell: None,
            replies: Vec::new(),
            retitled: false,
            reported: None,
            reports_status: false,
            progress_ending: Status::Idle,
            copied: None,
            size,
        }
    }

    pub(crate) fn key(
        &mut self,
        key: Key,
        mods: Mods,
        text: Option<String>,
        unshifted: Option<char>,
    ) -> Vec<u8> {
        self.term.encode_key_with_options(
            &KeyEvent {
                key,
                modifiers: mods,
                consumed_modifiers: Mods {
                    shift: mods.shift,
                    alt: cfg!(target_os = "macos") && mods.alt,
                    ..Mods::default()
                },
                text,
                unshifted,
                ..KeyEvent::new(key)
            },
            KeyEncodeOptions {
                macos_option_as_alt: !cfg!(target_os = "macos"),
            },
        )
    }

    /// Native synchronous callbacks observe controls before later batched text.
    pub(crate) fn write(&mut self, bytes: &[u8]) {
        let mut defaults = self.term.query_defaults.clone();
        defaults.size.get_or_insert(self.term.query_size());
        self.term.feed_with_handler(
            bytes,
            &mut Host {
                drawer: &mut self.drawer,
                hold: &mut self.hold,
                capturing: self.capturing,
                replies: &mut self.replies,
                retitled: &mut self.retitled,
                reported: &mut self.reported,
                reports_status: &mut self.reports_status,
                progress_ending: &mut self.progress_ending,
                copied: &mut self.copied,
                last_mouse_cell: &mut self.last_mouse_cell,
                defaults: &defaults,
            },
        );
    }

    pub(crate) fn size(&self) -> Size {
        self.size
    }

    pub(crate) fn title(&self) -> String {
        self.term.title.clone()
    }

    pub(crate) fn retitled(&mut self) -> Option<String> {
        std::mem::take(&mut self.retitled).then(|| self.title())
    }

    pub(crate) fn reported(&mut self) -> Option<Status> {
        self.reported.take()
    }

    pub(crate) fn reporter_gone(&mut self) {
        self.reports_status = false;
        self.progress_ending = Status::Idle;
    }

    pub(crate) fn copied(&mut self) -> Option<String> {
        self.copied.take()
    }

    pub(crate) fn focus(&self, focused: bool) -> Vec<u8> {
        self.term.encode_focus(focused)
    }

    pub(crate) fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    pub(crate) fn typed(&mut self) {
        self.term.screen_mut().scroll_viewport(isize::MIN);
        self.term.screen_mut().selection = None;
    }

    pub(crate) fn tracks_mouse(&self) -> bool {
        self.term.mouse_mode != 0
    }

    /// Configure defaults without discarding application color overrides.
    pub(crate) fn set_colors(
        &mut self,
        colors: &Colors,
        background: RgbColor,
        foreground: RgbColor,
    ) -> Vec<u8> {
        let background = colors.background.unwrap_or(background);
        let scheme = scheme_of(background);
        let rescheme = self.term.query_defaults.color_scheme.replace(scheme) != Some(scheme);
        let mut palette = rustty_vt::default_palette();
        for &(index, color) in &colors.palette {
            palette[usize::from(index)] = color;
        }
        self.term.set_default_colors(
            Some(colors.foreground.unwrap_or(foreground)),
            Some(background),
            colors.cursor,
            &palette,
        );
        if rescheme && self.term.modes.dec(2031) {
            scheme.encode()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn resize(&mut self, size: Size) -> Vec<u8> {
        self.size = size;
        self.hold = None;
        self.last_mouse_cell = None;
        let effects = self.term.resize_with_cell_size(
            size.cols,
            size.rows,
            Some((u32::from(size.cell_width), u32::from(size.cell_height))),
        );
        for effect in effects {
            if let Effect::Write(bytes) = effect {
                self.replies.extend_from_slice(&bytes);
            }
        }
        self.take_replies()
    }

    pub(crate) fn mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        at: mouse::Position,
        mods: Mods,
    ) -> Vec<u8> {
        self.term.encode_mouse(
            MouseEvent {
                action,
                button,
                x: f64::from(at.x),
                y: f64::from(at.y),
                modifiers: mods,
            },
            MouseEncodeOptions {
                screen_size: [
                    f64::from(self.size.cols) * f64::from(self.size.cell_width),
                    f64::from(self.size.rows) * f64::from(self.size.cell_height),
                ],
                cell_size: [
                    u32::from(self.size.cell_width.max(1)),
                    u32::from(self.size.cell_height.max(1)),
                ],
                padding: [0.; 4],
                any_button_pressed: action != mouse::Action::Release && button.is_some(),
                last_cell: Some(&mut self.last_mouse_cell),
            },
        )
    }

    fn wheel(&mut self, lines: f32, at: mouse::Position, mods: Mods) -> Vec<u8> {
        let button = if lines > 0. {
            mouse::Button::WheelUp
        } else {
            mouse::Button::WheelDown
        };
        (0..lines.abs() as usize)
            .flat_map(|_| self.mouse(mouse::Action::Press, Some(button), at, mods))
            .collect()
    }

    pub(crate) fn scroll(&mut self, lines: f32, at: mouse::Position, mods: Mods) -> Vec<u8> {
        if self.tracks_mouse() {
            return self.wheel(lines, at, mods);
        }
        if let Some(arrows) = self.alternate_scroll(lines) {
            return arrows;
        }
        self.term.screen_mut().scroll_viewport(lines as isize);
        Vec::new()
    }

    fn alternate_scroll(&mut self, lines: f32) -> Option<Vec<u8>> {
        if !(self.term.is_alternate_screen() && self.term.modes.dec(1007)) {
            return None;
        }
        let arrow = if lines > 0. { Key::Up } else { Key::Down };
        Some(
            (0..lines.abs() as usize)
                .flat_map(|_| self.key(arrow, Mods::default(), None, None))
                .collect(),
        )
    }

    fn point(&self, at: mouse::Position) -> Option<GridPoint> {
        let [x, y] = viewport(at, self.size);
        let row = self.term.screen().viewport().nth(y as usize)?;
        (x < row.cells.len() as u32).then_some(GridPoint {
            row: row.id,
            col: x as usize,
        })
    }

    pub(crate) fn select_press(
        &mut self,
        at: mouse::Position,
        repeated: bool,
    ) -> anyhow::Result<()> {
        const REPEAT: Duration = Duration::from_secs(1);
        if !repeated {
            self.pressed += 2 * REPEAT;
        }
        let point = self
            .point(at)
            .context("selection is outside the terminal")?;
        let selection = self.selecting.press(
            &mut self.term,
            gesture::Press {
                time: Some(self.pressed.as_nanos() as i128),
                point,
                xpos: f64::from(at.x),
                ypos: f64::from(at.y),
                max_distance: f64::MAX,
                repeat_interval: REPEAT.as_nanos() as u64,
                word_boundaries: DEFAULT_WORD_BOUNDARIES,
                behaviors: gesture::DEFAULT_BEHAVIORS,
            },
        );
        self.term.screen_mut().selection = selection;
        Ok(())
    }

    pub(crate) fn select_drag(
        &mut self,
        at: mouse::Position,
        rectangle: bool,
    ) -> anyhow::Result<()> {
        let point = self
            .point(at)
            .context("selection is outside the terminal")?;
        self.term.screen_mut().selection = self.selecting.drag(
            &self.term,
            gesture::Drag {
                point,
                xpos: f64::from(at.x),
                ypos: f64::from(at.y),
                rectangle,
                word_boundaries: DEFAULT_WORD_BOUNDARIES,
                geometry: geometry(self.size),
            },
        );
        Ok(())
    }

    pub(crate) fn select_release(&mut self, at: mouse::Position) {
        self.selecting.release(&self.term, self.point(at));
    }

    pub(crate) fn selected(&self) -> Option<String> {
        self.term.screen().selection_text()
    }

    pub(crate) fn select_all(&mut self) {
        self.term.screen_mut().selection = self.term.screen().select_all();
    }

    #[cfg(test)]
    pub(crate) fn autoscrolling(&self) -> bool {
        self.selecting.autoscroll() != gesture::Autoscroll::None
    }

    pub(crate) fn select_autoscroll(&mut self, at: mouse::Position, rectangle: bool) {
        let selection = self.selecting.autoscroll_tick(
            &mut self.term,
            gesture::AutoscrollTick {
                viewport: viewport(at, self.size),
                xpos: f64::from(at.x),
                ypos: f64::from(at.y),
                rectangle,
                word_boundaries: DEFAULT_WORD_BOUNDARIES,
                geometry: geometry(self.size),
            },
        );
        if selection.is_some() {
            self.term.screen_mut().selection = selection;
        }
    }

    pub(crate) fn paste(&self, text: &str) -> Vec<u8> {
        self.term.encode_paste(text)
    }

    /// The screen with its scrollback, modes and title, as what builds the same in a fresh one.
    /// An alternate screen comes without the one behind it.
    #[cfg(unix)]
    pub(crate) fn screen(&self) -> anyhow::Result<Vec<u8>> {
        let mut formatter = self.term.formatter(Format::Vt);
        formatter.extra = TerminalExtra {
            palette: false,
            ..TerminalExtra::ALL
        };
        let mut screen = b"\x1b]2;".to_vec();
        screen.extend_from_slice(self.term.title_bytes());
        screen.extend_from_slice(b"\x1b\\");
        // Serialize application overrides only; theme defaults belong to the new host.
        use rustty_vt::color::Target;
        for target in (0..=255).map(Target::Palette).chain([
            Target::Foreground,
            Target::Background,
            Target::Cursor,
        ]) {
            if let Some([r, g, b]) = self.term.color_override(target) {
                let name = match target {
                    Target::Palette(index) => index.to_string(),
                    Target::Foreground => "foreground".into(),
                    Target::Background => "background".into(),
                    Target::Cursor => "cursor".into(),
                };
                screen.extend_from_slice(
                    format!("\x1b]21;{name}=rgb:{r:02x}/{g:02x}/{b:02x}\x1b\\").as_bytes(),
                );
            }
        }
        screen.extend_from_slice(
            &formatter
                .format()
                .context("formatting terminal for reattachment")?,
        );
        Ok(screen)
    }

    pub(crate) fn frame(&mut self, chosen: &Colors) -> Arc<Frame> {
        self.end_timed_out_hold();
        if let Some(hold) = &self.hold {
            return hold.frame.clone();
        }
        self.drawer.chosen = chosen.clone();
        self.drawer.frame(&self.term)
    }

    pub(crate) fn end_timed_out_hold(&mut self) -> bool {
        if self
            .hold
            .as_ref()
            .is_some_and(|hold| hold.since.elapsed() >= HOLD_TIMEOUT)
        {
            self.hold = None;
            self.term.set_mode(true, 2026, false);
            true
        } else {
            false
        }
    }

    pub(crate) fn hold_deadline(&self) -> Option<Instant> {
        self.hold.as_ref().map(|hold| hold.since + HOLD_TIMEOUT)
    }

    pub(crate) fn set_capturing(&mut self, shown: bool) {
        self.capturing = shown;
    }
}

/// Host policy and synchronous effects; the terminal owns all protocol parsing.
struct Host<'a> {
    drawer: &'a mut Drawer,
    hold: &'a mut Option<Hold>,
    capturing: bool,
    replies: &'a mut Vec<u8>,
    retitled: &'a mut bool,
    reported: &'a mut Option<Status>,
    reports_status: &'a mut bool,
    progress_ending: &'a mut Status,
    copied: &'a mut Option<String>,
    defaults: &'a query::Defaults,
    last_mouse_cell: &'a mut Option<[u16; 2]>,
}

impl EffectHandler for Host<'_> {
    fn effect(&mut self, effect: Effect) {
        match effect {
            Effect::Write(bytes) => self.replies.extend_from_slice(&bytes),
            Effect::Title(_) => *self.retitled = true,
            Effect::ProgramStatus(report) => {
                if let Some(status) = Status::reported(report) {
                    *self.reported = Some(status);
                    *self.reports_status = true;
                }
            }
            Effect::Progress { state, .. } if !*self.reports_status => {
                if let Some(status) = Status::progressed(state, self.progress_ending) {
                    *self.reported = Some(status);
                }
            }
            Effect::ProgramStatusReset | Effect::ProgressReset => {
                *self.reports_status = false;
                *self.progress_ending = Status::Idle;
                *self.reported = Some(Status::Idle);
            }
            _ => {}
        }
    }

    fn synchronized_output(&mut self, terminal: &Terminal, enabled: bool) {
        *self.hold = (enabled && self.capturing).then(|| Hold {
            frame: self.drawer.frame(terminal),
            since: Instant::now(),
        });
    }
    fn mouse_tracking_changed(&mut self, _: u16, _: u16) {
        *self.last_mouse_cell = None;
    }
    fn color_scheme(&mut self) -> Option<ColorScheme> {
        self.defaults.color_scheme
    }
    fn device_attributes(&mut self) -> Option<query::DeviceAttributes> {
        self.defaults.device_attributes.clone()
    }
    fn size(&mut self) -> Option<query::Size> {
        self.defaults
            .size_reports
            .then_some(self.defaults.size)
            .flatten()
    }
    fn enquiry(&mut self) -> Vec<u8> {
        self.defaults.enquiry.clone()
    }
    fn xtversion(&mut self) -> Vec<u8> {
        self.defaults.xtversion.clone()
    }
    fn clipboard_read_enabled(&self) -> bool {
        false
    }
    fn clipboard_write(&mut self, write: &clipboard::Write) -> clipboard::WriteResult {
        let text = write
            .contents
            .iter()
            .find(|content| content.mime.starts_with(b"text/plain"))
            .map(|content| String::from_utf8_lossy(&content.data).into_owned());
        match text {
            Some(text) => {
                *self.copied = Some(text);
                clipboard::WriteResult::Success { remember: false }
            }
            None => clipboard::WriteResult::Unsupported,
        }
    }
}

#[derive(Default)]
struct Drawer {
    chosen: Colors,
    drawn: HashMap<u64, (RowKey, Arc<Row>)>,
    last: Arc<Frame>,
    seen: Option<(ViewKey, Colors)>,
    palette: Vec<RgbColor>,
    cursor_seen: Option<CursorKey>,
    #[cfg(test)]
    extracted_cells: usize,
}

struct Hold {
    frame: Arc<Frame>,
    since: Instant,
}

impl Drawer {
    fn frame(&mut self, term: &Terminal) -> Arc<Frame> {
        let screen = term.screen();
        let seen = (screen.render_key(), self.chosen.clone());
        let (foreground, background) = if term.modes.dec(5) {
            (term.background, term.foreground)
        } else {
            (term.foreground, term.background)
        };
        let mut frame = Frame {
            background,
            foreground,
            cursor_color: term.cursor_color.unwrap_or(foreground),
            ..Frame::default()
        };
        let full =
            self.seen.as_ref().is_none_or(|previous| {
                !previous.0.same_row_layout(&seen.0) || previous.1 != seen.1
            }) || (self.last.foreground, self.last.background) != (foreground, background)
                || self.palette != term.palette;
        let selected_bg = self.chosen.selection_background.unwrap_or(foreground);
        let selected_fg = self.chosen.selection_foreground.unwrap_or(background);
        let view = screen.render_view();
        let cursor = view.cursor();
        let cursor_now = cursor.map(|cursor| cursor.key());
        let draw_cursor = |cursor: RenderCursor<'_>, text: &str| Cursor {
            x: cursor.x as u16,
            y: cursor.y as u16,
            text: text.to_owned(),
            style: cursor.shape,
        };
        let mut drawn_now = HashMap::with_capacity(self.drawn.len());
        for (y, rendered) in view.rows().enumerate() {
            let key = rendered.key();
            let row = rendered.row;
            let cursor = cursor.filter(|cursor| cursor.y == y);
            if !full
                && let Some((previous_key, previous)) = self.drawn.get(&key.id)
                && *previous_key == key
            {
                let previous = previous.clone();
                drawn_now.insert(key.id, (key, previous.clone()));
                frame.rows.push(previous);
                if let Some(cursor) = cursor {
                    if self.cursor_seen == cursor_now {
                        frame.cursor = self.last.cursor.clone();
                    } else if let Some(text) = cursor.text() {
                        #[cfg(test)]
                        {
                            self.extracted_cells += 1;
                        }
                        frame.cursor = Some(draw_cursor(cursor, text.as_str()));
                    }
                }
                continue;
            }
            #[cfg(test)]
            {
                self.extracted_cells += row.cells.len();
            }
            let mut drawn = Row::default();
            let mut after_wide = false;
            for (x, cell) in row.cells.iter().enumerate() {
                if cell.width() == 0 || cell.spacer_head() {
                    continue;
                }
                let wide = cell.width() == 2;
                let text = row.text(x);
                let text = if text.is_empty() { " " } else { text.as_str() };
                let attributes = row.style(x);
                let mut fg = term.resolve_color(attributes.foreground, foreground);
                let mut bg = term.resolve_color(attributes.background, background);
                if attributes.inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if rendered
                    .selection
                    .as_ref()
                    .is_some_and(|range| range.contains(&x))
                {
                    (fg, bg) = (selected_fg, selected_bg);
                }
                if attributes.invisible {
                    fg = bg;
                }
                let style = Style {
                    fg,
                    bg: (bg != background).then_some(bg),
                    bold: attributes.bold,
                    underline: attributes.underline,
                    strikethrough: attributes.strikethrough,
                    faint: attributes.faint,
                };
                if let Some(cursor) = cursor.filter(|cursor| cursor.x == x) {
                    frame.cursor = Some(draw_cursor(cursor, text));
                }
                let link = row
                    .hyperlink(x)
                    .and_then(|link| std::str::from_utf8(link.uri_bytes()).ok())
                    .filter(|uri| !uri.is_empty())
                    .map(
                        |uri| match drawn.runs.last().and_then(|run| run.link.as_ref()) {
                            Some(last) if last.as_ref() == uri => last.clone(),
                            _ => Arc::from(uri),
                        },
                    );
                let start = drawn.text.len();
                drawn.text.push_str(text);
                let end = (drawn.text.len(), x as u16 + u16::from(cell.width()));
                match drawn.runs.last_mut() {
                    Some(last)
                        if last.style == style && last.link == link && !wide && !after_wide =>
                    {
                        (last.text.end, last.columns.end) = end;
                    }
                    _ => drawn.runs.push(Run {
                        text: start..end.0,
                        columns: x as u16..end.1,
                        style,
                        link,
                    }),
                }
                after_wide = wide;
            }
            let drawn = match self.drawn.remove(&key.id) {
                Some((_, previous)) if previous.as_ref() == &drawn => previous,
                _ => Arc::new(drawn),
            };
            drawn_now.insert(key.id, (key, drawn.clone()));
            frame.rows.push(drawn);
        }
        self.cursor_seen = cursor_now;
        self.drawn = drawn_now;
        self.seen = Some(seen);
        self.palette.clone_from(&term.palette);
        // Eq lets Arc<Row> compare shared pointers without reading cached text.
        if *self.last != frame {
            self.last = Arc::new(frame);
        }
        self.last.clone()
    }
}

fn scheme_of([r, g, b]: RgbColor) -> ColorScheme {
    let brightness = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    if brightness > 127.5 {
        ColorScheme::Light
    } else {
        ColorScheme::Dark
    }
}

fn viewport(at: mouse::Position, size: Size) -> [u32; 2] {
    let index = |at: f32, cell: u16, count: u16| {
        u32::from(((at / f32::from(cell.max(1))).max(0.) as u16).min(count.saturating_sub(1)))
    };
    [
        index(at.x, size.cell_width, size.cols),
        index(at.y, size.cell_height, size.rows),
    ]
}

fn geometry(size: Size) -> gesture::Geometry {
    gesture::Geometry {
        columns: u32::from(size.cols),
        cell_width: u32::from(size.cell_width.max(1)),
        padding_left: 0,
        screen_height: u32::from(size.rows) * u32::from(size.cell_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustty_vt::Selection;

    const SIZE: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };
    const DARK: RgbColor = [0x29, 0x29, 0x29];
    const LIGHT: RgbColor = [0xff, 0xff, 0xff];

    #[test]
    fn reports_on_the_root_record_are_the_status() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b]7501;state=working:app=claude-code\x07");
        assert_eq!(vt.reported(), Some(Status::Working));
        assert_eq!(vt.reported(), None, "a report is told once");

        vt.write(b"\x1b]7501;state=done:id=build\x07\x1b]7501;state=clear:id=build\x07");
        assert_eq!(vt.reported(), None, "a part of the work isn't the status");

        vt.write(b"\x1b]7501;state=blocked:kind=permission:msg=QXBwbHk/\x07");
        assert_eq!(vt.reported(), Some(Status::Blocked("Apply?".into())));

        vt.write(b"\x1b]7501;state=error\x07");
        assert_eq!(vt.reported(), Some(Status::Failed(String::new())));

        vt.write(b"\x1b]7501;state=done\x07\x1bc");
        assert_eq!(vt.reported(), Some(Status::Idle), "a full reset forgets it");
    }

    #[test]
    fn progress_reports_are_the_status_too() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b]9;4;0\x07");
        assert_eq!(vt.reported(), Some(Status::Idle), "nothing was under way");

        vt.write(b"\x1b]9;4;3\x07");
        assert_eq!(vt.reported(), Some(Status::Working), "pi's, while it works");

        vt.write(b"\x1b]9;4;0;\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Done),
            "Claude Code's, at a turn's end"
        );

        vt.write(b"\x1b]9;4;0;\x07\x1b]9;4;3;\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Working),
            "and at the next one's start"
        );

        vt.write(b"\x1b]9;4;1;40\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Working),
            "and with how far it got"
        );

        vt.write(b"\x1b]9;4;2\x07\x1b]9;4;1;60\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Working),
            "an error, as it goes on"
        );

        vt.write(b"\x1b]9;4;0\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Failed(String::new())),
            "work with an error ends failed"
        );

        vt.write(b"\x1b]9;4;3\x07\x1b]9;4;0\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Done),
            "the error was the last work's"
        );
    }

    #[test]
    fn a_program_reporting_its_status_is_heard_over_its_progress() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b]9;4;3\x07\x1b]7501;state=blocked\x07\x1b]9;4;3\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Blocked(String::new())),
            "the progress it goes on reporting doesn't say it works"
        );

        vt.write(b"\x1b]7501;state=done\x07\x1b]9;4;0\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Done),
            "nor does its removal after say it's idle"
        );

        vt.reporter_gone();
        vt.write(b"\x1b]9;4;3\x07");
        assert_eq!(
            vt.reported(),
            Some(Status::Working),
            "the next program is heard by its progress"
        );
    }

    #[test]
    fn programs_can_write_but_cannot_read_the_clipboard() {
        let mut vt = Vt::new(SIZE);
        // "copied!" in base64, as nvim and tmux send it.
        vt.write(b"\x1b]52;c;Y29waWVkIQ==\x07");
        assert_eq!(
            vt.copied().as_deref(),
            Some("copied!"),
            "OSC 52 writes the clipboard"
        );
        assert_eq!(vt.copied(), None, "a copy is told once");
        vt.write(b"\x1b]52;p;cHJpbWFyeQ==\x07");
        assert_eq!(
            vt.copied().as_deref(),
            Some("primary"),
            "the primary selection is the clipboard too"
        );
        vt.write(b"\x1b]52;c;?\x07");
        assert_eq!(
            (vt.take_replies(), vt.copied()),
            (Vec::new(), None),
            "what the user copied stays theirs"
        );
    }

    #[test]
    fn a_program_asking_is_told_when_the_background_turns_light_or_dark() {
        let mut vt = Vt::new(SIZE);
        assert_eq!(
            vt.set_colors(&Colors::default(), DARK, LIGHT),
            b"",
            "unasked, nothing is told"
        );
        vt.write(b"\x1b[?2031h");
        assert_eq!(
            vt.set_colors(&Colors::default(), LIGHT, DARK),
            b"\x1b[?997;2n",
            "asked (2031), a light background is told"
        );
        assert_eq!(
            vt.set_colors(&Colors::default(), LIGHT, DARK),
            b"",
            "and nothing more while it stays light"
        );
        vt.write(b"\x1b[?996n");
        assert_eq!(
            vt.take_replies(),
            b"\x1b[?997;2n",
            "queries use the current host background"
        );
        let colors = Colors {
            background: Some(DARK),
            ..Colors::default()
        };
        assert_eq!(
            vt.set_colors(&colors, LIGHT, DARK),
            b"\x1b[?997;1n",
            "a theme's own dark background is told, over the app's light one"
        );
    }

    #[test]
    fn a_program_asking_for_in_band_size_reports_is_told_each_resize() {
        let mut vt = Vt::new(SIZE);
        let wider = Size { cols: 100, ..SIZE };
        assert_eq!(vt.resize(wider), b"", "unasked, the shell has SIGWINCH");
        vt.write(b"\x1b[?2048h");
        vt.take_replies();
        assert_eq!(
            String::from_utf8(vt.resize(SIZE)).unwrap(),
            "\x1b[48;24;80;384;640t",
            "nvim asks for 2048 and redraws only once it reads the new rows, columns and pixels"
        );
    }

    fn first_row(vt: &mut Vt) -> String {
        let frame = vt.frame(&Colors::default());
        frame.rows[0].text.trim_end().to_owned()
    }

    #[test]
    fn a_synchronized_update_shows_the_frame_from_before_until_it_is_done() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"before");
        vt.write(b"\x1b[?2026h\x1b[2J\x1b[Hhalf");
        assert_eq!(
            first_row(&mut vt),
            "before",
            "a redraw half done never shows"
        );
        vt.write(b" done\x1b[?2026l");
        assert_eq!(
            first_row(&mut vt),
            "half done",
            "it shows at once when done"
        );

        vt.write(b"\x1b[?2026h\x1b[2J\x1b[Hnext\x1b[?2026l\x1b[?2026h\x1b[2J\x1b[Hpart");
        assert_eq!(
            first_row(&mut vt),
            "next",
            "a redraw done between two frames shows, though the next began before them"
        );
        vt.resize(Size { cols: 100, ..SIZE });
        assert_eq!(
            (vt.hold_deadline(), first_row(&mut vt).as_str()),
            (None, "part"),
            "a resize ends the hold, so the screen shows as it is, as nvim redraws for it"
        );
    }

    #[test]
    fn out_of_sight_a_hold_keeps_no_frame() {
        let mut vt = Vt::new(SIZE);
        vt.set_capturing(false);
        vt.write(b"before\x1b[?2026hafter");
        assert_eq!(
            (vt.hold_deadline(), first_row(&mut vt).as_str()),
            (None, "beforeafter"),
            "a terminal out of sight draws nothing, so there's no frame to keep"
        );
    }

    #[test]
    fn a_hold_never_ended_times_out() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"before\x1b[?2026hafter");
        assert!(vt.hold_deadline().is_some(), "the program holds the screen");
        let mut hold = vt.hold.take().unwrap();
        hold.since -= HOLD_TIMEOUT;
        vt.hold = Some(hold);
        assert_eq!(
            first_row(&mut vt),
            "beforeafter",
            "a program that never says it's done is drawn as it is once the hold times out"
        );
        assert_eq!(vt.hold_deadline(), None, "and the hold is over");
    }

    #[test]
    fn a_key_without_an_unshifted_codepoint_does_not_take_the_last_keys() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b[>1u");
        vt.key(Key::Char('a'), Mods::default(), Some("a".into()), Some('a'));
        let sent = vt.key(
            Key::Char(' '),
            Mods {
                shift: true,
                ..Mods::default()
            },
            Some(" ".into()),
            None,
        );
        assert_eq!(
            String::from_utf8(sent).unwrap(),
            " ",
            "shift+space after an a types a space, not the a's kitty report with shift"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn option_types_what_the_layout_puts_there_under_the_kitty_protocol() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b[>1u");
        let hash = vt.key(
            Key::Char('3'),
            Mods {
                alt: true,
                ..Mods::default()
            },
            Some("#".into()),
            Some('3'),
        );
        assert_eq!(
            hash, b"#",
            "a British layout's hash is typed, not reported as alt+3"
        );
        let shifted = Mods {
            alt: true,
            shift: true,
            ..Mods::default()
        };
        let ring = vt.key(Key::Char('a'), shifted, Some("\u{c5}".into()), Some('a'));
        assert_eq!(
            String::from_utf8(ring).unwrap(),
            "\u{c5}",
            "option and shift together type too"
        );
        let word_left = vt.key(
            Key::Left,
            Mods {
                alt: true,
                ..Mods::default()
            },
            None,
            None,
        );
        assert_eq!(
            String::from_utf8(word_left).unwrap(),
            "\x1b[1;3D",
            "option on a key without text stays a modifier"
        );
    }

    #[test]
    fn wheel_reports_to_a_program_tracking_the_mouse() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b[?1003h\x1b[?1006h");
        let at = mouse::Position { x: 20., y: 40. };
        let up = vt.wheel(2., at, Mods::default());
        let down = vt.wheel(-1., at, Mods::default());
        assert_eq!(
            (up, down),
            (
                b"\x1b[<64;3;3M\x1b[<64;3;3M".to_vec(),
                b"\x1b[<65;3;3M".to_vec()
            ),
            "two notches up then one down over cell 3,3 are SGR wheel buttons 64 and 65"
        );
    }

    #[test]
    fn a_drag_reports_its_press_each_cell_it_crosses_and_its_release() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b[?1002h\x1b[?1006h");
        let cell = |col: f32, row: f32| mouse::Position {
            x: col * 8. + 4.,
            y: row * 16. + 8.,
        };
        let left = Some(mouse::Button::Left);
        let mut send = |action, button, at| {
            String::from_utf8(vt.mouse(action, button, at, Mods::default())).unwrap()
        };
        let sent = [
            send(mouse::Action::Move, None, cell(1., 1.)),
            send(mouse::Action::Press, left, cell(2., 1.)),
            send(mouse::Action::Move, left, cell(5., 1.)),
            send(mouse::Action::Move, left, cell(5.2, 1.)),
            send(mouse::Action::Release, left, cell(5., 1.)),
        ];
        assert_eq!(
            sent,
            ["", "\x1b[<0;3;2M", "\x1b[<32;6;2M", "", "\x1b[<0;6;2m"],
            "button tracking (1002) hears no motion without a button, and a drag only as it \
             reaches another cell"
        );
    }

    #[test]
    fn selecting_a_wide_spacer_highlights_its_glyph_and_only_redraws_affected_rows() {
        let mut vt = Vt::new(Size {
            cols: 8,
            rows: 3,
            ..SIZE
        });
        vt.write("漢\r\nsecond\r\nthird".as_bytes());
        let before = vt.frame(&Colors::default());
        vt.drawer.extracted_cells = 0;
        let spacer = vt.term.screen().point(0, 1).unwrap();
        vt.term.screen_mut().selection = Some(Selection {
            start: spacer,
            end: spacer,
            rectangular: false,
        });
        let selected = vt.frame(&Colors::default());
        let glyph = &selected.rows[0].runs[0];
        assert_eq!(
            (glyph.columns.clone(), glyph.style.fg, glyph.style.bg),
            (0..2, selected.background, Some(selected.foreground)),
            "selecting the trailing half highlights the whole wide glyph"
        );
        assert_eq!(
            std::mem::take(&mut vt.drawer.extracted_cells),
            8,
            "only the row gaining a selection is extracted"
        );
        assert!(
            Arc::ptr_eq(&before.rows[1], &selected.rows[1])
                && Arc::ptr_eq(&before.rows[2], &selected.rows[2]),
            "unselected rows retain their rendered allocations"
        );

        vt.term.screen_mut().selection = Some(Selection {
            start: vt.term.screen().point(1, 0).unwrap(),
            end: vt.term.screen().point(1, 3).unwrap(),
            rectangular: false,
        });
        let moved = vt.frame(&Colors::default());
        assert_eq!(
            (
                moved.rows[0].runs[0].style.bg,
                moved.rows[1].runs[0].style.bg
            ),
            (None, Some(moved.foreground)),
            "moving the selection clears the old highlight and draws the new one"
        );
        assert_eq!(
            vt.drawer.extracted_cells, 16,
            "only the rows gaining and losing selection are extracted"
        );
        assert!(
            Arc::ptr_eq(&selected.rows[2], &moved.rows[2]),
            "the unrelated row is still reused"
        );
    }

    #[test]
    fn runs_take_their_columns_and_a_wide_character_has_one_of_its_own() {
        let mut vt = Vt::new(SIZE);
        vt.write("ab漢cd".as_bytes());
        let frame = vt.frame(&Colors::default());
        let row = &frame.rows[0];
        let runs: Vec<_> = (row.runs.iter())
            .map(|run| (row.text[run.text.clone()].trim_end(), run.columns.clone()))
            .collect();
        assert_eq!(
            runs,
            [("ab", 0..2), ("漢", 2..4), ("cd", 4..80)],
            "the wide character takes two columns, so what follows it starts two on"
        );
    }

    /// The text of each run on the first row with what it links to, without the blanks after.
    fn links(vt: &mut Vt) -> Vec<(String, Option<String>)> {
        let frame = vt.frame(&Colors::default());
        let row = &frame.rows[0];
        (row.runs.iter())
            .map(|run| {
                let text = row.text[run.text.clone()].trim_end().to_owned();
                (text, run.link.as_deref().map(str::to_owned))
            })
            .filter(|(text, _)| !text.is_empty())
            .collect()
    }

    #[test]
    fn hyperlinks_split_runs_and_stop_at_the_closing_sequence() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"see \x1b]8;;https://a.example\x1b\\one\x1b]8;;https://b.example\x1b\\two\x1b]8;;\x1b\\ end");
        assert_eq!(
            links(&mut vt),
            [
                ("see".to_owned(), None),
                ("one".to_owned(), Some("https://a.example".to_owned())),
                ("two".to_owned(), Some("https://b.example".to_owned())),
                (" end".to_owned(), None),
            ],
            "adjacent links and unlinked text remain separate runs in the same style"
        );
    }

    #[test]
    fn reverse_video_swaps_default_colors() {
        let mut vt = Vt::new(SIZE);
        vt.set_colors(&Colors::default(), DARK, LIGHT);
        vt.write(b"text");
        vt.frame(&Colors::default());
        vt.write(b"\x1b[?5h");
        let frame = vt.frame(&Colors::default());
        assert_eq!(
            (frame.background, frame.foreground, frame.cursor_color),
            (LIGHT, DARK, DARK),
            "reverse video applies to the entire display's defaults"
        );
        assert_eq!(
            frame.rows[0].runs[0].style.fg, DARK,
            "default text uses the reversed foreground"
        );
    }

    #[test]
    fn renewed_mouse_tracking_reports_the_current_cell() {
        let mut vt = Vt::new(SIZE);
        let at = mouse::Position { x: 4., y: 8. };
        vt.write(b"\x1b[?1003h\x1b[?1006h");
        let first = vt.mouse(mouse::Action::Move, None, at, Mods::default());
        assert!(!first.is_empty(), "the first motion reports its cell");
        vt.write(b"\x1b[?1003l\x1b[?1003h");
        assert_eq!(
            vt.mouse(mouse::Action::Move, None, at, Mods::default()),
            first,
            "a new mouse-tracking session has not seen the previous cell"
        );
    }

    #[test]
    fn rows_are_drawn_again_in_colours_a_program_sets() {
        let mut vt = Vt::new(SIZE);
        let (black, white) = ([0, 0, 0], [255, 255, 255]);
        vt.set_colors(&Colors::default(), black, white);
        vt.write(b"hello\r\n");
        vt.frame(&Colors::default());
        vt.write(b"\x1b]10;rgb:ff/00/00\x07");
        let frame = vt.frame(&Colors::default());
        assert_eq!(
            frame.rows[0].runs[0].style.fg,
            [255, 0, 0],
            "text in the default colour turns red with it, though OSC 10 marks no row"
        );
    }

    #[test]
    fn the_wheel_sends_arrows_to_an_alternate_screen() {
        let mut vt = Vt::new(SIZE);
        assert_eq!(
            vt.alternate_scroll(1.),
            None,
            "the primary screen scrolls its own history"
        );
        vt.write(b"\x1b[?1049h");
        assert_eq!(
            vt.alternate_scroll(2.),
            Some(b"\x1b[A\x1b[A".to_vec()),
            "an alternate screen asks for arrows (1007) unless it says otherwise, as in Ghostty"
        );
        vt.write(b"\x1b[?1h");
        assert_eq!(
            vt.alternate_scroll(-1.),
            Some(b"\x1bOB".to_vec()),
            "in application cursor mode the arrows are the application's"
        );
        vt.write(b"\x1b[?1007l");
        assert_eq!(vt.alternate_scroll(1.), None, "unless it turns them off");
    }

    /// The text of each run on the first row drawn on a background of its own.
    fn highlighted(vt: &mut Vt) -> Vec<String> {
        let frame = vt.frame(&Colors::default());
        let row = &frame.rows[0];
        (row.runs.iter())
            .filter(|run| run.style.bg.is_some())
            .map(|run| row.text[run.text.clone()].trim_end().to_owned())
            .collect()
    }

    #[test]
    fn a_double_click_dragged_selects_by_word() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"hello brave new world");
        let over = |col: f32| mouse::Position {
            x: col * 8. + 4.,
            y: 8.,
        };
        vt.select_press(over(7.), false).unwrap();
        vt.select_release(over(7.));
        assert_eq!(
            highlighted(&mut vt),
            Vec::<String>::new(),
            "a click selects nothing"
        );
        vt.select_press(over(7.), true).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            ["brave"],
            "a second click in a row takes the word"
        );
        vt.select_drag(over(18.), false).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            ["brave new world"],
            "dragging from it takes whole words up to the one under the pointer"
        );
        assert_eq!(
            vt.selected().as_deref(),
            Some("brave new world"),
            "copying exposes the drag selection"
        );
        vt.select_release(over(18.));
        vt.select_press(over(7.), false).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            Vec::<String>::new(),
            "a click a while later is a first click again, and clears it"
        );
    }

    #[test]
    fn a_selection_dragged_above_the_top_scrolls_up_a_row_a_tick() {
        let mut vt = Vt::new(SIZE);
        for n in 0..60 {
            vt.write(format!("line {n}\r\n").as_bytes());
        }
        let row = |row: f32| mouse::Position {
            x: 4.,
            y: row * 16. + 8.,
        };
        let first_row = |vt: &mut Vt| {
            vt.frame(&Colors::default()).rows[0]
                .text
                .trim_end()
                .to_owned()
        };
        assert_eq!(
            first_row(&mut vt),
            "line 37",
            "the screen shows the last lines"
        );
        vt.select_press(row(5.), false).unwrap();
        vt.select_drag(row(-1.), false).unwrap();
        assert!(vt.autoscrolling(), "above the top it asks to scroll");
        for _ in 0..3 {
            vt.select_autoscroll(row(-1.), false);
        }
        assert_eq!(
            first_row(&mut vt),
            "line 34",
            "three ticks scroll three rows"
        );
        assert_eq!(
            highlighted(&mut vt),
            ["line 34"],
            "the selection follows to the top"
        );
        vt.select_release(row(-1.));
        assert!(!vt.autoscrolling(), "letting go stops it");
    }

    #[test]
    fn the_host_explicitly_rejects_graphics_before_answering_the_next_query() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b_Ga=T,i=1,s=1,v=1,f=24;AQID\x1b\\ok\x1b[c");
        assert_eq!(
            vt.take_replies(),
            b"\x1b_Gi=1;ENOTSUP: graphics disabled\x1b\\\x1b[?62;22c",
            "graphics clients receive an error through the host's ordered reply path"
        );
        assert_eq!(
            vt.term.graphics().limit,
            0,
            "this host has no image renderer"
        );
        assert!(
            vt.term.graphics().images.is_empty() && vt.term.graphics().placements.is_empty(),
            "rejected graphics retain no images or placements"
        );
        assert_eq!(first_row(&mut vt), "ok", "following text still renders");
    }

    #[test]
    fn cached_frames_reuse_unchanged_rows_through_rewrites_scrolls_and_resets() {
        let mut vt = Vt::new(Size {
            cols: 8,
            rows: 3,
            ..SIZE
        });
        vt.write(b"first\r\nsecond");
        let before = vt.frame(&Colors::default());
        assert_eq!(
            std::mem::take(&mut vt.drawer.extracted_cells),
            24,
            "the first frame extracts all 8 by 3 cells"
        );
        vt.mouse(
            mouse::Action::Move,
            None,
            mouse::Position { x: 4., y: 8. },
            Mods::default(),
        );
        assert!(
            Arc::ptr_eq(&before, &vt.frame(&Colors::default())),
            "an unchanged screen reuses its frame"
        );
        assert_eq!(
            vt.drawer.extracted_cells, 0,
            "cached cells need no extraction"
        );
        vt.write(b"\x1b[2;1Hrewrite");
        let after = vt.frame(&Colors::default());
        assert_eq!(
            std::mem::take(&mut vt.drawer.extracted_cells),
            8,
            "only the rewritten row is extracted"
        );
        assert!(
            Arc::ptr_eq(&before.rows[0], &after.rows[0]),
            "the preceding row is reused"
        );
        assert!(
            Arc::ptr_eq(&before.rows[2], &after.rows[2]),
            "the following row is reused"
        );
        assert_eq!(
            after.rows[1].text.trim_end(),
            "rewrite",
            "the changed row shows the new text"
        );
        assert!(
            Arc::ptr_eq(&after, &vt.frame(&Colors::default())),
            "the next unchanged frame reuses the rewritten screen"
        );
        assert_eq!(vt.drawer.extracted_cells, 0, "the new frame is cached too");
        // The hold callback is read-only and can also reuse every unchanged cell.
        vt.write(b"\x1b[?2026h\x1b[?2026h\x1b[?2026l");
        assert_eq!(
            vt.drawer.extracted_cells, 0,
            "repeated holds reuse the unchanged cells"
        );
        vt.write(b"\x1b[2;1Hrewrite");
        assert!(
            Arc::ptr_eq(&after, &vt.frame(&Colors::default())),
            "rewriting identical content retains the frame despite new row revisions"
        );
        vt.write(b"\r\nthird\r\nfourth");
        let scrolled = vt.frame(&Colors::default());
        assert!(
            Arc::ptr_eq(&after.rows[1], &scrolled.rows[0]),
            "scrolling reuses the same row at its new position"
        );
        assert_eq!(
            scrolled.rows[2].text.trim_end(),
            "fourth",
            "newly visible text is drawn"
        );
        vt.write(b"\x1bcabc");
        vt.frame(&Colors::default());
        vt.write(b"\x1bcxyz");
        assert_eq!(
            first_row(&mut vt),
            "xyz",
            "resets never reuse a prior screen's rows"
        );
    }

    #[test]
    fn cached_rows_follow_native_mutations_and_cursor_changes_without_redrawing_other_rows() {
        let mut vt = Vt::new(SIZE);
        vt.write("漢\r\nsecond".as_bytes());
        let fresh = vt.frame(&Colors::default());
        let cursor = fresh.cursor.as_ref().unwrap();
        assert_eq!(
            (cursor.style, cursor.text.as_str()),
            (CursorVisualStyle::Block, " "),
            "a fresh blank cursor contains a space"
        );
        vt.drawer.extracted_cells = 0;
        vt.term.screen_mut().set_cell_text(2, 0, "x");
        assert_eq!(
            vt.frame(&Colors::default()).rows[2].text.trim_end(),
            "x",
            "direct screen mutations invalidate the rendered row"
        );
        assert_eq!(
            std::mem::take(&mut vt.drawer.extracted_cells),
            80,
            "only the directly mutated row is extracted"
        );
        vt.write(b"\x1b[6 q");
        let blank = vt.frame(&Colors::default());
        let cursor = blank.cursor.as_ref().unwrap();
        assert_eq!(
            (cursor.style, cursor.text.as_str()),
            (CursorVisualStyle::Bar, " "),
            "a new cursor shape over cached cells retains the blank text"
        );
        vt.drawer.extracted_cells = 0;
        vt.write(b"\x1b[H");
        let cursor = vt.frame(&Colors::default()).cursor.clone().unwrap();
        assert_eq!(
            (cursor.x, cursor.y, cursor.style, cursor.text.as_str()),
            (0, 0, CursorVisualStyle::Bar, "漢"),
            "cursor movement and shape changes retain the cached cell's text"
        );
        assert!(
            std::mem::take(&mut vt.drawer.extracted_cells) <= 1,
            "a cursor change needs at most its own cell"
        );
        vt.write(b"\x1b[?25l");
        assert!(
            vt.frame(&Colors::default()).cursor.is_none(),
            "the program can hide its cursor"
        );
        assert_eq!(
            vt.drawer.extracted_cells, 0,
            "hiding the cursor extracts no cells"
        );
        vt.write(b"\x1b[?25h\x1b[1;2H");
        assert!(
            vt.frame(&Colors::default()).cursor.is_none(),
            "a wide spacer has no cursor text"
        );
        vt.drawer.extracted_cells = 0;
        vt.frame(&Colors::default());
        assert_eq!(
            vt.drawer.extracted_cells, 0,
            "the spacer cursor state is cached"
        );
    }

    #[cfg(unix)]
    #[test]
    fn handoff_preserves_content_and_application_overrides_while_accepting_the_new_theme() {
        let size = Size {
            cols: 20,
            rows: 4,
            ..SIZE
        };
        let mut vt = Vt::new(size);
        vt.set_colors(
            &Colors {
                palette: vec![(1, [200, 0, 0]), (2, [0, 200, 0])],
                ..Colors::default()
            },
            DARK,
            LIGHT,
        );
        vt.write(b"scrollback\r\none\r\ntwo\r\nthree\r\n");
        vt.write(b"\x1b]2;working\x07\x1b]4;1;rgb:12/34/56\x07\x1b]10;rgb:23/45/67\x07\x1b[31m\x1b]8;;https://example.com\x07link\x1b]8;;\x07\x1b[32mgreen\x1b[0m\x1b[?2004h\x1b[>1u\x1b[6 q");
        let mut fresh = Vt::new(size);
        fresh.write(&vt.screen().unwrap());
        fresh.set_colors(
            &Colors {
                palette: vec![(1, [100, 0, 0]), (2, [0, 100, 0])],
                ..Colors::default()
            },
            [9, 8, 7],
            [6, 5, 4],
        );
        let text = fresh.term.formatter(Format::Plain).format().unwrap();
        assert_eq!(
            text,
            vt.term.formatter(Format::Plain).format().unwrap(),
            "all text survives handoff"
        );
        assert!(
            text.starts_with(b"scrollback\n"),
            "the serialized content includes history"
        );
        assert_eq!(
            (
                fresh.term.title_bytes(),
                fresh.term.modes.dec(2004),
                fresh.term.screen().kitty_keyboard.current()
            ),
            (
                b"working".as_slice(),
                true,
                vt.term.screen().kitty_keyboard.current()
            ),
            "the title, paste mode and negotiated keyboard protocol survive handoff"
        );
        assert_eq!(
            (fresh.term.foreground, fresh.term.background),
            ([0x23, 0x45, 0x67], [9, 8, 7]),
            "application overrides survive while unset colors follow the receiving theme"
        );
        let frame = fresh.frame(&Colors::default());
        let row = &frame.rows[3];
        assert_eq!(
            (row.runs[0].style.fg, row.runs[1].style.fg),
            ([0x12, 0x34, 0x56], [0, 100, 0]),
            "rendered palette colors preserve the application override and accept the new theme default"
        );
        assert_eq!(
            row.runs[0].link.as_deref(),
            Some("https://example.com"),
            "rendered text keeps its hyperlink"
        );
        let cursor = frame.cursor.as_ref().unwrap();
        assert_eq!(
            (cursor.x, cursor.y, cursor.style),
            (9, 3, CursorVisualStyle::Bar),
            "the cursor retains its position and shape"
        );
    }

    #[test]
    fn synchronous_host_retains_ordered_status_geometry_and_identity_replies() {
        let mut vt = Vt::new(SIZE);
        vt.write(b"\x1b]7501;?\x1b\\\x1b[18t\x1b[14t\x1b[16t\x1b[?2048h\x1b[>0q\x1b[c");
        assert_eq!(vt.take_replies(), b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\\x1b[8;24;80t\x1b[4;384;640t\x1b[6;16;8t\x1b[48;24;80;384;640t\x1bP>|ghostty 0.1.0\x1b\\\x1b[?62;22c", "status, geometry and identity replies follow query order");
    }
}
