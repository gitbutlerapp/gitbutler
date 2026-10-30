//! The emulator: libghostty-vt's terminal, with the encoders that turn keys, the mouse and
//! pastes into what the shell reads, and the frames drawn from it.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use libghostty_vt::fmt::{Format, Formatter, FormatterOptions};
use libghostty_vt::key::{self, Key, Mods};
use libghostty_vt::mouse;
use libghostty_vt::render::{CellIterator, Dirty, RowIterator};
use libghostty_vt::screen::{CellWide, Screen};
use libghostty_vt::selection::gesture::{self, Gesture};
use libghostty_vt::style::{RgbColor, Underline};
use libghostty_vt::terminal::{
    Mode, Point as GridPoint, PointCoordinate, ProgramStatus, ProgramStatusState, ScrollViewport,
};
use libghostty_vt::{RenderState, paste};

use super::{Colors, Size};

/// How a run of cells draws; its background only where it isn't the default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub fg: RgbColor,
    pub bg: Option<RgbColor>,
    pub bold: bool,
    pub underline: Underline,
    pub strikethrough: bool,
    pub faint: bool,
}

/// A row's text, with how each run of it draws by the bytes it covers.
#[derive(Default)]
pub struct Row {
    pub text: String,
    pub runs: Vec<(Range<usize>, Style)>,
}

/// The cursor's cell, by column and row, with the text under it.
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub text: String,
}

/// What a frame draws.
#[derive(Default)]
pub struct Frame {
    pub rows: Vec<Row>,
    pub cursor: Option<Cursor>,
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub cursor_color: RgbColor,
}

/// What the program running says it's doing, by OSC 7501.
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
    fn reported(report: ProgramStatus<'_>) -> Option<Self> {
        if !report.id().is_empty() {
            return None;
        }
        let message = || String::from_utf8_lossy(report.message()).into_owned();
        match report.state().ok()? {
            ProgramStatusState::Idle | ProgramStatusState::Clear => Some(Self::Idle),
            ProgramStatusState::Working => Some(Self::Working),
            ProgramStatusState::Blocked => Some(Self::Blocked(message())),
            ProgramStatusState::Done => Some(Self::Done),
            ProgramStatusState::Error => Some(Self::Failed(message())),
            _ => None,
        }
    }
}

/// The emulator and what it's read and written with.
pub(crate) struct Vt {
    term: libghostty_vt::Terminal<'static, 'static>,
    state: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    keys: key::Encoder<'static>,
    key: key::Event<'static>,
    mouse: mouse::Encoder<'static>,
    pointer: mouse::Event<'static>,
    selecting: Gesture<'static>,
    press: gesture::PressEvent<'static>,
    drag: gesture::DragEvent<'static>,
    release: gesture::ReleaseEvent<'static>,
    tick: gesture::AutoscrollTickEvent<'static>,
    /// When the gesture was last told a press was, by clicks rather than the clock.
    pressed: Duration,
    /// What the emulator answers the shell's queries with, to send it.
    replies: Rc<Cell<Vec<u8>>>,
    retitled: Rc<Cell<bool>>,
    reported: Rc<Cell<Option<Status>>>,
    size: Size,
}

impl Vt {
    pub(crate) fn new(size: Size) -> libghostty_vt::error::Result<Self> {
        use libghostty_vt::terminal::{
            ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
            PrimaryDeviceAttributes, SecondaryDeviceAttributes, TertiaryDeviceAttributes,
        };
        let mut term = libghostty_vt::Terminal::new(size.cols, size.rows)?;
        // Ghostty's default `scrollback-limit`.
        term.set_scrollback_max_bytes(Some(10_000_000))?;
        let replies = Rc::new(Cell::new(Vec::new()));
        let retitled = Rc::new(Cell::new(false));
        let reported = Rc::new(Cell::new(None));
        let (sent, titled, heard) = (replies.clone(), retitled.clone(), reported.clone());
        term.on_pty_write(move |_, bytes| {
            let mut all = sent.take();
            all.extend_from_slice(bytes);
            sent.set(all);
        })?
        .on_title_changed(move |_| titled.set(true))?
        // Set, it has the terminal tell programs that ask that it listens.
        .on_program_status(move |_, report| {
            if let Some(status) = Status::reported(report) {
                heard.set(Some(status));
            }
        })?
        // Shells and editors wait on the answer to which terminal this is.
        .on_device_attributes(|_| {
            Some(DeviceAttributes {
                primary: PrimaryDeviceAttributes::new(
                    ConformanceLevel::VT220,
                    &[DeviceAttributeFeature::ANSI_COLOR],
                ),
                secondary: SecondaryDeviceAttributes {
                    device_type: DeviceType::VT220,
                    firmware_version: 0,
                    rom_cartridge: 0,
                },
                tertiary: TertiaryDeviceAttributes { unit_id: 0 },
            })
        })?;
        let mut mouse = mouse::Encoder::new()?;
        mouse.set_track_last_cell(true).set_size(encoder_size(size));
        Ok(Self {
            term,
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            keys: key::Encoder::new()?,
            key: key::Event::new()?,
            mouse,
            pointer: mouse::Event::new()?,
            selecting: Gesture::new()?,
            press: gesture::PressEvent::new()?,
            drag: gesture::DragEvent::new()?,
            release: gesture::ReleaseEvent::new()?,
            tick: gesture::AutoscrollTickEvent::new()?,
            pressed: Duration::ZERO,
            replies,
            retitled,
            reported,
            size,
        })
    }

    pub(crate) fn key(
        &mut self,
        key: Key,
        mods: Mods,
        text: Option<String>,
        unshifted: Option<char>,
    ) -> Vec<u8> {
        self.keys.set_options_from_terminal(&self.term);
        let event = &mut self.key;
        event
            .set_action(key::Action::Press)
            .set_key(key)
            .set_mods(mods)
            .set_utf8(text)
            .set_unshifted_codepoint(unshifted.unwrap_or('\0'));
        let mut bytes = Vec::new();
        self.keys.encode_to_vec(&self.key, &mut bytes).ok();
        bytes
    }

    /// Takes what the program wrote, and the mouse tracking it may have asked for with it.
    /// Taking that clears which cell motion was last told of, so it's taken here rather than
    /// with each motion.
    // ponytail: a program that writes as it's told of motion is told of each move within a cell;
    // take the tracking per event once Ghostty's setopt_from_terminal keeps the cell, as setopt
    // does, when nothing changed.
    pub(crate) fn write(&mut self, bytes: &[u8]) {
        self.term.vt_write(bytes);
        self.mouse.set_options_from_terminal(&self.term);
    }

    pub(crate) fn size(&self) -> Size {
        self.size
    }

    /// The title the shell set, lossily as text.
    pub(crate) fn title(&self) -> String {
        String::from_utf8_lossy(self.term.title().unwrap_or_default()).into_owned()
    }

    /// The title, if the shell set one since this was last asked.
    pub(crate) fn retitled(&mut self) -> Option<String> {
        self.retitled.take().then(|| self.title())
    }

    /// What the program running last reported, if it did since this was last asked.
    pub(crate) fn reported(&mut self) -> Option<Status> {
        self.reported.take()
    }

    /// What the emulator answered the shell's queries with since this was last asked.
    pub(crate) fn take_replies(&mut self) -> Vec<u8> {
        self.replies.take()
    }

    /// Typing shows the bottom, and drops the selection.
    pub(crate) fn typed(&mut self) {
        self.term.scroll_viewport(ScrollViewport::Bottom);
        self.term.set_selection(None).ok();
    }

    pub(crate) fn tracks_mouse(&self) -> bool {
        self.term.is_mouse_tracking().unwrap_or(false)
    }

    /// The defaults `colors` sets, with `background` and `foreground` where it sets none.
    pub(crate) fn set_colors(
        &mut self,
        colors: &Colors,
        background: RgbColor,
        foreground: RgbColor,
    ) {
        let term = &mut self.term;
        term.set_default_bg_color(Some(colors.background.unwrap_or(background)))
            .ok();
        term.set_default_fg_color(Some(colors.foreground.unwrap_or(foreground)))
            .ok();
        term.set_default_cursor_color(colors.cursor).ok();
        term.set_default_color_palette(None).ok();
        if let Ok(mut palette) = term.default_color_palette() {
            for &(index, color) in &colors.palette {
                if let Some(entry) = palette.0.get_mut(usize::from(index)) {
                    *entry = color;
                }
            }
            term.set_default_color_palette(Some(palette)).ok();
        }
    }

    /// Sizes the emulator and, so motion is told of once a cell, the mouse encoder only when
    /// the size changes.
    pub(crate) fn resize(&mut self, size: Size) {
        self.size = size;
        let cell = (u32::from(size.cell_width), u32::from(size.cell_height));
        (self.term)
            .resize(size.cols, size.rows, cell.0, cell.1)
            .ok();
        self.mouse.set_size(encoder_size(size));
    }

    /// `button` going `action` over `at` on the grid, for a program tracking the mouse; nothing
    /// for what it doesn't track, or motion within the cell it was last told of.
    pub(crate) fn mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        at: mouse::Position,
        mods: Mods,
    ) -> Vec<u8> {
        (self.mouse).set_any_button_pressed(action != mouse::Action::Release && button.is_some());
        (self.pointer)
            .set_action(action)
            .set_button(button)
            .set_mods(mods)
            .set_position(at);
        let mut bytes = Vec::new();
        self.mouse.encode_to_vec(&self.pointer, &mut bytes).ok();
        bytes
    }

    /// The wheel turned `lines` up, or down if negative, over `at` on the grid, for a program
    /// tracking the mouse.
    fn wheel(&mut self, lines: f32, at: mouse::Position, mods: Mods) -> Vec<u8> {
        let button = if lines > 0. {
            mouse::Button::Four
        } else {
            mouse::Button::Five
        };
        (0..lines.abs() as usize)
            .flat_map(|_| self.mouse(mouse::Action::Press, Some(button), at, mods))
            .collect()
    }

    /// The wheel turned `lines` up, or down if negative, over `at` on the surface: told to a
    /// program tracking the mouse, sent as arrows to an alternate screen that asks for them,
    /// or else scrolling back through what scrolled off.
    pub(crate) fn scroll(&mut self, lines: f32, at: mouse::Position, mods: Mods) -> Vec<u8> {
        if self.tracks_mouse() {
            return self.wheel(lines, at, mods);
        }
        if let Some(arrows) = self.alternate_scroll(lines) {
            return arrows;
        }
        (self.term).scroll_viewport(ScrollViewport::Delta(-(lines as isize)));
        Vec::new()
    }

    /// The arrows the wheel turning `lines` up, or down if negative, sends a program on the
    /// alternate screen that asks for them (1007), as less and man do, or none.
    fn alternate_scroll(&mut self, lines: f32) -> Option<Vec<u8>> {
        let alternate = matches!(self.term.active_screen(), Ok(Screen::Alternate));
        if !(alternate && self.term.mode(Mode::ALT_SCROLL).unwrap_or(false)) {
            return None;
        }
        let arrow = if lines > 0. {
            Key::ArrowUp
        } else {
            Key::ArrowDown
        };
        let arrows = (0..lines.abs() as usize)
            .flat_map(|_| self.key(arrow, Mods::empty(), None, None))
            .collect();
        Some(arrows)
    }

    /// A left press over `at` on the surface, `repeated` when the platform counts it the next
    /// click in a row. Those are given the gesture at the time of the click before and others
    /// a gap later, so double and triple clicks keep to the system's speed, as Ghostty's do.
    pub(crate) fn select_press(
        &mut self,
        at: mouse::Position,
        repeated: bool,
    ) -> libghostty_vt::error::Result<()> {
        const REPEAT: Duration = Duration::from_secs(1);
        if !repeated {
            self.pressed += 2 * REPEAT;
        }
        (self.press)
            .set_position(f64::from(at.x), f64::from(at.y))?
            .set_time(self.pressed)?
            .set_repeat_interval(REPEAT)?;
        let cell = self
            .term
            .grid_ref(GridPoint::Viewport(viewport(at, self.size)))?;
        let selection = self.press.apply(&mut self.selecting, &self.term, cell)?;
        self.term.set_selection(selection.as_ref())?;
        Ok(())
    }

    /// The press dragged to `at` on the surface; a block between it and where it began if
    /// `rectangle`.
    pub(crate) fn select_drag(
        &mut self,
        at: mouse::Position,
        rectangle: bool,
    ) -> libghostty_vt::error::Result<()> {
        (self.drag)
            .set_position(f64::from(at.x), f64::from(at.y))?
            .set_rectangle(rectangle)?;
        let cell = self
            .term
            .grid_ref(GridPoint::Viewport(viewport(at, self.size)))?;
        let selection =
            self.drag
                .apply(&mut self.selecting, &self.term, cell, geometry(self.size))?;
        self.term.set_selection(selection.as_ref())?;
        Ok(())
    }

    pub(crate) fn select_release(
        &mut self,
        at: mouse::Position,
    ) -> libghostty_vt::error::Result<()> {
        let cell = self
            .term
            .grid_ref(GridPoint::Viewport(viewport(at, self.size)))?;
        (self.release).apply(&mut self.selecting, &self.term, Some(cell))
    }

    /// What's selected, as Ghostty copies it: lines the terminal wrapped joined again, and
    /// without the blanks trailing each.
    pub(crate) fn selected(&self) -> Option<String> {
        let selection = self.term.selection().ok()??;
        let options = FormatterOptions::new()
            .with_format(Format::Plain)
            .with_unwrap(true)
            .with_trim(true)
            .with_selection(&selection);
        let text = Formatter::new(&self.term, options)
            .ok()?
            .format_alloc(None)
            .ok()?;
        Some(String::from_utf8_lossy(&text).into_owned())
    }

    pub(crate) fn select_all(&mut self) {
        if let Ok(Some(all)) = self.term.select_all() {
            self.term.set_selection(Some(&all)).ok();
        }
    }

    /// Whether the selection's drag went past the top or bottom, for the viewport to follow.
    pub(crate) fn autoscrolling(&self) -> bool {
        (self.selecting.autoscroll(&self.term)).is_ok_and(|to| to != gesture::Autoscroll::None)
    }

    /// Scrolls a row the way the drag went past, and takes the selection on to `at` on the
    /// surface. A press the scrolling lost leaves the selection as it was.
    pub(crate) fn select_autoscroll(
        &mut self,
        at: mouse::Position,
        rectangle: bool,
    ) -> libghostty_vt::error::Result<()> {
        (self.tick)
            .set_position(f64::from(at.x), f64::from(at.y))?
            .set_rectangle(rectangle)?;
        let cell = viewport(at, self.size);
        let selection =
            (self.tick).apply(&mut self.selecting, &self.term, cell, geometry(self.size))?;
        if let Some(selection) = selection {
            self.term.set_selection(Some(&selection))?;
        }
        Ok(())
    }

    pub(crate) fn paste(&self, text: &str) -> Vec<u8> {
        let bracketed = self.term.mode(Mode::BRACKETED_PASTE).unwrap_or(false);
        let mut data = text.as_bytes().to_vec();
        let mut bytes = vec![0; data.len() + 12];
        let written = paste::encode(&mut data, bracketed, &mut bytes).unwrap_or(0);
        bytes.truncate(written);
        bytes
    }

    /// The screen with its scrollback, modes and title, as what builds the same in a fresh one.
    /// An alternate screen comes without the one behind it.
    #[cfg(unix)]
    pub(crate) fn screen(&self) -> libghostty_vt::error::Result<Vec<u8>> {
        let options = FormatterOptions::new()
            .with_format(Format::Vt)
            .with_modes(true)
            .with_scrolling_region(true)
            .with_pwd(true)
            .with_keyboard(true)
            .with_cursor(true)
            .with_style(true)
            .with_hyperlink(true)
            .with_protection(true)
            .with_kitty_keyboard(true)
            .with_charsets(true);
        let title = self.term.title().unwrap_or_default();
        let mut screen = b"\x1b]2;".to_vec();
        screen.extend_from_slice(title);
        screen.extend_from_slice(b"\x1b\\");
        screen.extend_from_slice(&Formatter::new(&self.term, options)?.format_alloc(None)?);
        Ok(screen)
    }

    /// What to draw, with what's selected in `chosen`'s selection colours, or the default
    /// colours swapped where it sets none, as Ghostty does.
    pub(crate) fn frame(&mut self, chosen: &Colors) -> libghostty_vt::error::Result<Frame> {
        let snapshot = self.state.update(&self.term)?;
        let colors = snapshot.colors()?;
        let cursor = match snapshot.cursor_visible()? {
            true => snapshot.cursor_viewport()?,
            false => None,
        };
        let mut frame = Frame {
            background: colors.background,
            foreground: colors.foreground,
            cursor_color: colors.cursor.unwrap_or(colors.foreground),
            ..Frame::default()
        };
        let selected_bg = chosen.selection_background.unwrap_or(colors.foreground);
        let selected_fg = chosen.selection_foreground.unwrap_or(colors.background);
        let mut rows = self.rows.update(&snapshot)?;
        let mut cell_text = String::new();
        let mut y = 0;
        while let Some(row) = rows.next() {
            let mut drawn = Row::default();
            let selected = row.selection()?.map(|range| range.start_x..=range.end_x);
            let mut cells = self.cells.update(row)?;
            let mut x = 0;
            while let Some(cell) = cells.next() {
                let at = x;
                x += 1;
                if matches!(
                    cell.raw_cell()?.wide()?,
                    CellWide::SpacerTail | CellWide::SpacerHead
                ) {
                    continue;
                }
                cell_text.clear();
                if cell.graphemes_len()? > 0 {
                    cell.graphemes_utf8(&mut cell_text)?;
                } else {
                    cell_text.push(' ');
                }
                let attributes = cell.style()?;
                let mut fg = cell.fg_color()?.unwrap_or(colors.foreground);
                let mut bg = cell.bg_color()?.unwrap_or(colors.background);
                if attributes.inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if selected.as_ref().is_some_and(|range| range.contains(&at)) {
                    (fg, bg) = (selected_fg, selected_bg);
                }
                if attributes.invisible {
                    fg = bg;
                }
                let style = Style {
                    fg,
                    bg: (bg != colors.background).then_some(bg),
                    bold: attributes.bold,
                    underline: attributes.underline,
                    strikethrough: attributes.strikethrough,
                    faint: attributes.faint,
                };
                if cursor.is_some_and(|c| c.x == at && c.y == y) {
                    frame.cursor = Some(Cursor {
                        x: at,
                        y,
                        text: cell_text.clone(),
                    });
                }
                let start = drawn.text.len();
                drawn.text.push_str(&cell_text);
                match drawn.runs.last_mut() {
                    Some((range, last)) if *last == style => range.end = drawn.text.len(),
                    _ => drawn.runs.push((start..drawn.text.len(), style)),
                }
            }
            row.set_dirty(false)?;
            frame.rows.push(drawn);
            y += 1;
        }
        snapshot.set_dirty(Dirty::Clean)?;
        Ok(frame)
    }
}

/// The viewport cell under `at` on the surface, or the nearest one when it's outside.
fn viewport(at: mouse::Position, size: Size) -> PointCoordinate {
    let index = |at: f32, cell: u16, count: u16| {
        ((at / f32::from(cell.max(1))).max(0.) as u16).min(count.saturating_sub(1))
    };
    PointCoordinate {
        x: index(at.x, size.cell_width, size.cols),
        y: u32::from(index(at.y, size.cell_height, size.rows)),
    }
}

/// The surface the selection gesture measures drags and edges on.
fn geometry(size: Size) -> gesture::Geometry {
    gesture::Geometry {
        columns: u32::from(size.cols),
        cell_width: u32::from(size.cell_width.max(1)),
        padding_left: 0,
        screen_height: u32::from(size.rows) * u32::from(size.cell_height),
    }
}

fn encoder_size(size: Size) -> mouse::EncoderSize {
    let (cell_width, cell_height) = (u32::from(size.cell_width), u32::from(size.cell_height));
    mouse::EncoderSize {
        screen_width: u32::from(size.cols) * cell_width,
        screen_height: u32::from(size.rows) * cell_height,
        cell_width: cell_width.max(1),
        cell_height: cell_height.max(1),
        padding_top: 0,
        padding_bottom: 0,
        padding_right: 0,
        padding_left: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };

    #[test]
    fn a_program_asking_for_status_is_answered_before_what_it_asked_after() {
        let mut vt = Vt::new(SIZE).unwrap();
        vt.write(b"\x1b]7501;?\x07\x1b[c");
        let replies = String::from_utf8(vt.take_replies()).unwrap();
        assert!(
            replies.starts_with("\x1b]7501;?:states=") && replies.contains("\x1b[?"),
            "Claude Code takes the attributes as the end of the replies, so these come first: {replies:?}"
        );
    }

    #[test]
    fn reports_on_the_root_record_are_the_status() {
        let mut vt = Vt::new(SIZE).unwrap();
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
    fn a_key_without_an_unshifted_codepoint_does_not_take_the_last_keys() {
        let mut vt = Vt::new(SIZE).unwrap();
        vt.write(b"\x1b[>1u");
        vt.key(Key::A, Mods::empty(), Some("a".into()), Some('a'));
        let sent = vt.key(Key::Space, Mods::SHIFT, Some(" ".into()), None);
        assert_eq!(
            String::from_utf8(sent).unwrap(),
            " ",
            "shift+space after an a types a space, not the a's kitty report with shift"
        );
    }

    #[test]
    fn wheel_reports_to_a_program_tracking_the_mouse() {
        let mut vt = Vt::new(SIZE).unwrap();
        vt.write(b"\x1b[?1003h\x1b[?1006h");
        let at = mouse::Position { x: 20., y: 40. };
        let up = vt.wheel(2., at, Mods::empty());
        let down = vt.wheel(-1., at, Mods::empty());
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
        let mut vt = Vt::new(SIZE).unwrap();
        vt.write(b"\x1b[?1002h\x1b[?1006h");
        let cell = |col: f32, row: f32| mouse::Position {
            x: col * 8. + 4.,
            y: row * 16. + 8.,
        };
        let left = Some(mouse::Button::Left);
        let mut send = |action, button, at| {
            String::from_utf8(vt.mouse(action, button, at, Mods::empty())).unwrap()
        };
        let sent = [
            send(mouse::Action::Motion, None, cell(1., 1.)),
            send(mouse::Action::Press, left, cell(2., 1.)),
            send(mouse::Action::Motion, left, cell(5., 1.)),
            send(mouse::Action::Motion, left, cell(5.2, 1.)),
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
    fn selected_cells_draw_in_the_default_colours_swapped() {
        let mut vt = Vt::new(SIZE).unwrap();
        vt.write(b"hello world");
        let cell = |x| {
            let point = libghostty_vt::terminal::PointCoordinate { x, y: 0 };
            vt.term
                .grid_ref(libghostty_vt::terminal::Point::Viewport(point))
                .unwrap()
        };
        let hello = libghostty_vt::selection::Selection::new(cell(0), cell(4), false);
        vt.term.set_selection(Some(&hello)).unwrap();
        let frame = vt.frame(&Colors::default()).unwrap();
        let row = &frame.rows[0];
        let backgrounds: Vec<_> = (row.runs.iter())
            .map(|(range, style)| (row.text[range.clone()].trim_end(), style.bg))
            .take(2)
            .collect();
        assert_eq!(
            backgrounds,
            [("hello", Some(frame.foreground)), (" world", None)],
            "only the selection is drawn, on the foreground colour"
        );
    }

    #[test]
    fn the_wheel_sends_arrows_to_an_alternate_screen() {
        let mut vt = Vt::new(SIZE).unwrap();
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
        let frame = vt.frame(&Colors::default()).unwrap();
        let row = &frame.rows[0];
        (row.runs.iter())
            .filter(|(_, style)| style.bg.is_some())
            .map(|(range, _)| row.text[range.clone()].trim_end().to_owned())
            .collect()
    }

    #[test]
    fn a_double_click_dragged_selects_by_word() {
        let mut vt = Vt::new(SIZE).unwrap();
        vt.write(b"hello brave new world");
        let over = |col: f32| mouse::Position {
            x: col * 8. + 4.,
            y: 8.,
        };
        vt.select_press(over(7.), false).unwrap();
        vt.select_release(over(7.)).unwrap();
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
        vt.select_release(over(18.)).unwrap();
        vt.select_press(over(7.), false).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            Vec::<String>::new(),
            "a click a while later is a first click again, and clears it"
        );
    }

    #[test]
    fn a_selection_dragged_above_the_top_scrolls_up_a_row_a_tick() {
        let mut vt = Vt::new(SIZE).unwrap();
        for n in 0..60 {
            vt.write(format!("line {n}\r\n").as_bytes());
        }
        let row = |row: f32| mouse::Position {
            x: 4.,
            y: row * 16. + 8.,
        };
        let first_row = |vt: &mut Vt| {
            vt.frame(&Colors::default()).unwrap().rows[0]
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
            vt.select_autoscroll(row(-1.), false).unwrap();
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
        vt.select_release(row(-1.)).unwrap();
        assert!(!vt.autoscrolling(), "letting go stops it");
    }

    #[test]
    fn copying_all_joins_what_the_terminal_wrapped() {
        let size = Size {
            cols: 10,
            rows: 4,
            ..SIZE
        };
        let mut vt = Vt::new(size).unwrap();
        assert_eq!(vt.selected(), None, "nothing is selected to copy");
        vt.write(b"0123456789abc  \r\nend");
        vt.select_all();
        assert_eq!(
            vt.selected().as_deref(),
            Some("0123456789abc\nend"),
            "a line wrapped at the edge copies as one, with no trailing blanks or rows"
        );
    }

    #[cfg(unix)]
    fn plain(vt: &Vt) -> String {
        let options = FormatterOptions::new().with_format(Format::Plain);
        let text = Formatter::new(&vt.term, options)
            .unwrap()
            .format_alloc(None)
            .unwrap();
        String::from_utf8(text.to_vec()).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn a_screen_written_into_a_fresh_terminal_builds_the_same() {
        let size = Size {
            cols: 20,
            rows: 4,
            ..SIZE
        };
        let mut vt = Vt::new(size).unwrap();
        vt.write(b"\x1b[31mred\x1b[0m\r\n");
        for n in 0..10 {
            vt.write(format!("line {n}\r\n").as_bytes());
        }
        vt.write(b"\x1b]2;working\x07\x1b[?2004h\x1b[2;5Hhere");

        let mut fresh = Vt::new(size).unwrap();
        fresh.write(&vt.screen().unwrap());

        assert_eq!(
            plain(&fresh),
            plain(&vt),
            "the same text, scrollback included"
        );
        assert!(
            plain(&fresh).starts_with("red\nline 0"),
            "the scrollback is there"
        );
        let cursor = |vt: &Vt| (vt.term.cursor_x().unwrap(), vt.term.cursor_y().unwrap());
        assert_eq!(cursor(&fresh), cursor(&vt), "the cursor is where it was");
        assert_eq!(
            fresh.term.title().unwrap(),
            b"working",
            "the title carries over"
        );
        assert!(
            fresh.term.mode(Mode::BRACKETED_PASTE).unwrap(),
            "the modes the shell set carry over"
        );
    }
}
