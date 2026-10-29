//! Terminals opened on a worktree: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt and drawn where the detail goes. They end with the app.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use futures::StreamExt;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, HighlightStyle, Hsla,
    KeyDownEvent, Pixels, Rgba, ScrollWheelEvent, SharedString, StrikethroughStyle, StyledText,
    Task, UnderlineStyle, WeakEntity, Window, canvas, div, prelude::*,
};
use libghostty_vt::key::{self, Key, Mods};
use libghostty_vt::render::{CellIterator, Dirty, RowIterator};
use libghostty_vt::screen::CellWide;
use libghostty_vt::style::{RgbColor, Underline};
use libghostty_vt::terminal::{Mode, ScrollViewport};
use libghostty_vt::{RenderState, TerminalOptions, paste};

use crate::backend::terminal::{Ghostty, Opened, Pty, Size};
use crate::state::Store;
use crate::theme::Theme;
use crate::ui::{dp, fixed};

/// A terminal, numbered in the order the session opened them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub(crate) u64);

pub enum TerminalEvent {
    /// The shell named the terminal something else.
    Retitled,
    /// The shell exited.
    Exited,
}

/// The size text shows in when neither the setting nor Ghostty's config says.
const TEXT_SIZE: f32 = 13.;

pub struct Terminal {
    pub title: SharedString,
    vt: Vt,
    pty: Pty,
    ghostty: Ghostty,
    size: Size,
    /// Whether the colours set are the dark theme's, once set.
    dark: Option<bool>,
    /// Pixels scrolled that don't make a whole line yet.
    scrolled: Pixels,
    focus: FocusHandle,
    /// It takes the keys once it's first drawn, as it opens.
    focus_when_drawn: bool,
    store: WeakEntity<Store>,
    _output: Task<()>,
}

impl EventEmitter<TerminalEvent> for Terminal {}

impl Focusable for Terminal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Terminal {
    /// The size a shell starts at, which the terminal's first frame corrects.
    pub const OPENING: Size = Size {
        cols: 80,
        rows: 24,
        cell_width: 8,
        cell_height: 16,
    };

    /// A terminal on the shell `opened` started at [`Terminal::OPENING`].
    pub fn new(
        opened: Opened,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let Opened {
            pty,
            mut output,
            ghostty,
        } = opened;
        let vt = Vt::new(Self::OPENING)?;
        Ok(cx.new(|cx: &mut Context<Self>| Self {
            title: "Terminal".into(),
            vt,
            pty,
            ghostty,
            size: Self::OPENING,
            dark: None,
            scrolled: fixed(0.),
            focus: cx.focus_handle(),
            focus_when_drawn: true,
            store,
            _output: cx.spawn(async move |this, cx| {
                while let Some(mut chunk) = output.next().await {
                    while let Ok(more) = output.try_recv() {
                        chunk.extend(more);
                    }
                    if this.update(cx, |this, cx| this.feed(&chunk, cx)).is_err() {
                        return;
                    }
                }
                this.update(cx, |_, cx| cx.emit(TerminalEvent::Exited)).ok();
            }),
        }))
    }

    fn feed(&mut self, output: &[u8], cx: &mut Context<Self>) {
        self.vt.term.vt_write(output);
        let replies = self.vt.replies.take();
        if !replies.is_empty() {
            self.pty.write(&replies).ok();
        }
        if self.vt.retitled.take() {
            let title = self.vt.term.title().unwrap_or_default().trim();
            self.title = if title.is_empty() { "Terminal" } else { title }
                .to_owned()
                .into();
            cx.emit(TerminalEvent::Retitled);
        }
        cx.notify();
    }

    fn resize(&mut self, size: Size, cx: &mut Context<Self>) {
        if size == self.size {
            return;
        }
        self.size = size;
        let cell = (u32::from(size.cell_width), u32::from(size.cell_height));
        self.vt
            .term
            .resize(size.cols, size.rows, cell.0, cell.1)
            .ok();
        self.pty.resize(size).ok();
        cx.notify();
    }

    fn input(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if bytes.is_empty() {
            return;
        }
        self.vt.term.scroll_viewport(ScrollViewport::Bottom);
        self.scrolled = fixed(0.);
        self.pty.write(bytes).ok();
        cx.notify();
    }

    fn text_size(&self, cx: &App) -> f32 {
        let chosen = (self.store.upgrade()).and_then(|store| store.read(cx).prefs.terminal_text);
        (chosen.map(f32::from))
            .or(self.ghostty.font_size)
            .unwrap_or(TEXT_SIZE)
    }

    /// Ghostty's colours for `theme`'s appearance, or the app's own where Ghostty sets none.
    fn set_colors(&mut self, theme: &Theme) {
        if self.dark == Some(theme.dark) {
            return;
        }
        self.dark = Some(theme.dark);
        let colors = self.ghostty.colors(theme.dark);
        let term = &mut self.vt.term;
        let background = colors.background.unwrap_or(rgb_color(theme.bg_1));
        let foreground = colors.foreground.unwrap_or(rgb_color(theme.text_1));
        term.set_default_bg_color(Some(background)).ok();
        term.set_default_fg_color(Some(foreground)).ok();
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

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        // ⌘ chords stay the app's, but for pasting.
        if modifiers.platform {
            if keystroke.key == "v" {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    let bytes = self.vt.paste(&text);
                    self.input(&bytes, cx);
                }
                cx.stop_propagation();
            }
            return;
        }
        let mut mods = Mods::empty();
        mods.set(Mods::SHIFT, modifiers.shift);
        mods.set(Mods::ALT, modifiers.alt);
        mods.set(Mods::CTRL, modifiers.control);
        let text = (keystroke.key_char.clone()).filter(|text| !text.chars().any(char::is_control));
        let mut chars = keystroke.key.chars();
        let unshifted = chars.next().filter(|_| chars.next().is_none());
        let bytes = self.vt.key(key_for(&keystroke.key), mods, text, unshifted);
        self.input(&bytes, cx);
        cx.stop_propagation();
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let line = fixed(f32::from(self.size.cell_height.max(1)));
        self.scrolled += event.delta.pixel_delta(line).y;
        let lines = (self.scrolled / line).trunc();
        if lines != 0. {
            self.scrolled -= line * lines;
            // ponytail: scrolls the scrollback only; programs that track the mouse get no wheel.
            (self.vt.term).scroll_viewport(ScrollViewport::Delta(-(lines as isize)));
            cx.notify();
        }
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        self.set_colors(&theme);
        if std::mem::take(&mut self.focus_when_drawn) {
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        let text_size = self.text_size(cx);
        let line = (text_size * 1.3).round();
        let font = gpui::font(theme.mono_font.clone());
        let text_system = window.text_system();
        let font_size = theme.scaled(text_size);
        let cell_width = (text_system.advance(text_system.resolve_font(&font), font_size, 'm'))
            .map_or(font_size * 0.6, |advance| advance.width);
        let cell_height = theme.scaled(line);
        let focused = self.focus.is_focused(window);
        let frame = self.vt.frame().unwrap_or_default();
        let this = cx.entity().downgrade();
        let measured = Rc::new(Cell::new(self.size));
        let grid = canvas(
            move |bounds, window, cx| {
                let size = Size {
                    cols: ((bounds.size.width / cell_width).floor() as u16).max(2),
                    rows: ((bounds.size.height / cell_height).floor() as u16).max(1),
                    cell_width: f32::from(cell_width).round() as u16,
                    cell_height: f32::from(cell_height).round() as u16,
                };
                // ponytail: resizes on every changed frame, so a sliding navigator reflows
                // the shell each frame; debounce if that shows.
                if size != measured.get() {
                    measured.set(size);
                    window.defer(cx, move |_, cx| {
                        this.update(cx, |this, cx| this.resize(size, cx)).ok();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let cursor = frame.cursor.map(|(x, y, text)| {
            let at = div()
                .absolute()
                .left(cell_width * f32::from(x))
                .top(cell_height * f32::from(y))
                .w(cell_width)
                .h(cell_height);
            if focused {
                at.bg(frame.cursor_color)
                    .text_color(frame.background)
                    .child(text)
            } else {
                at.border_1().border_color(frame.cursor_color)
            }
        });
        div()
            .id("terminal")
            .key_context("Terminal TextInput")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_scroll_wheel(cx.listener(Self::scroll))
            .size_full()
            .p(dp(8.))
            .bg(frame.background)
            .text_color(frame.foreground)
            .font_family(theme.mono_font.clone())
            .text_size(dp(text_size))
            .line_height(dp(line))
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .child(grid)
                    .children(frame.rows.into_iter().map(|(text, runs)| {
                        div()
                            .h(dp(line))
                            .whitespace_nowrap()
                            .child(StyledText::new(text).with_highlights(runs))
                    }))
                    .children(cursor),
            )
    }
}

/// A row's text's colours and styles, by the bytes they cover.
type Runs = Vec<(Range<usize>, HighlightStyle)>;

/// What a frame draws: each row's text with its colours and styles, and the cursor's cell,
/// by column and row, with the text under it.
#[derive(Default)]
struct Frame {
    rows: Vec<(SharedString, Runs)>,
    cursor: Option<(u16, u16, SharedString)>,
    background: Hsla,
    foreground: Hsla,
    cursor_color: Hsla,
}

/// The emulator and what it's read and written with.
struct Vt {
    term: libghostty_vt::Terminal<'static, 'static>,
    state: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    keys: key::Encoder<'static>,
    key: key::Event<'static>,
    /// What the emulator answers the shell's queries with, to send it.
    replies: Rc<Cell<Vec<u8>>>,
    retitled: Rc<Cell<bool>>,
}

impl Vt {
    fn new(size: Size) -> libghostty_vt::error::Result<Self> {
        use libghostty_vt::terminal::{
            ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
            PrimaryDeviceAttributes, SecondaryDeviceAttributes, TertiaryDeviceAttributes,
        };
        let mut term = libghostty_vt::Terminal::new(TerminalOptions {
            cols: size.cols,
            rows: size.rows,
            max_scrollback: 10_000,
        })?;
        let replies = Rc::new(Cell::new(Vec::new()));
        let retitled = Rc::new(Cell::new(false));
        let (sent, titled) = (replies.clone(), retitled.clone());
        term.on_pty_write(move |_, bytes| {
            let mut all = sent.take();
            all.extend_from_slice(bytes);
            sent.set(all);
        })?
        .on_title_changed(move |_| titled.set(true))?
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
        Ok(Self {
            term,
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            keys: key::Encoder::new()?,
            key: key::Event::new()?,
            replies,
            retitled,
        })
    }

    fn key(
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
            .set_utf8(text);
        if let Some(unshifted) = unshifted {
            event.set_unshifted_codepoint(unshifted);
        }
        let mut bytes = Vec::new();
        self.keys.encode_to_vec(&self.key, &mut bytes).ok();
        bytes
    }

    fn paste(&self, text: &str) -> Vec<u8> {
        let bracketed = self.term.mode(Mode::BRACKETED_PASTE).unwrap_or(false);
        let mut data = text.as_bytes().to_vec();
        let mut bytes = vec![0; data.len() + 12];
        let written = paste::encode(&mut data, bracketed, &mut bytes).unwrap_or(0);
        bytes.truncate(written);
        bytes
    }

    fn frame(&mut self) -> libghostty_vt::error::Result<Frame> {
        let snapshot = self.state.update(&self.term)?;
        let colors = snapshot.colors()?;
        let cursor = match snapshot.cursor_visible()? {
            true => snapshot.cursor_viewport()?,
            false => None,
        };
        let mut frame = Frame {
            background: hsla(colors.background),
            foreground: hsla(colors.foreground),
            cursor_color: hsla(colors.cursor.unwrap_or(colors.foreground)),
            ..Frame::default()
        };
        let mut rows = self.rows.update(&snapshot)?;
        let mut cell_text = String::new();
        let mut y = 0;
        while let Some(row) = rows.next() {
            let mut text = String::new();
            let mut runs = Runs::new();
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
                let style = cell.style()?;
                let mut fg = cell.fg_color()?.unwrap_or(colors.foreground);
                let mut bg = cell.bg_color()?.unwrap_or(colors.background);
                if style.inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if style.invisible {
                    fg = bg;
                }
                let color = hsla(fg);
                let highlight = HighlightStyle {
                    color: Some(color),
                    background_color: (bg != colors.background).then(|| hsla(bg)),
                    font_weight: style.bold.then_some(FontWeight::SEMIBOLD),
                    underline: (style.underline != Underline::None).then(|| UnderlineStyle {
                        thickness: fixed(1.),
                        color: Some(color),
                        wavy: style.underline == Underline::Curly,
                    }),
                    strikethrough: style.strikethrough.then(|| StrikethroughStyle {
                        thickness: fixed(1.),
                        color: Some(color),
                    }),
                    fade_out: style.faint.then_some(0.4),
                    ..HighlightStyle::default()
                };
                if cursor.is_some_and(|c| c.x == at && c.y == y) {
                    frame.cursor = Some((at, y, cell_text.clone().into()));
                }
                let start = text.len();
                text.push_str(&cell_text);
                match runs.last_mut() {
                    Some((range, last)) if *last == highlight => range.end = text.len(),
                    _ => runs.push((start..text.len(), highlight)),
                }
            }
            row.set_dirty(false)?;
            frame.rows.push((text.into(), runs));
            y += 1;
        }
        snapshot.set_dirty(Dirty::Clean)?;
        Ok(frame)
    }
}

fn hsla(color: RgbColor) -> Hsla {
    Rgba {
        r: f32::from(color.r) / 255.,
        g: f32::from(color.g) / 255.,
        b: f32::from(color.b) / 255.,
        a: 1.,
    }
    .into()
}

fn rgb_color(color: Hsla) -> RgbColor {
    let rgba = Rgba::from(color);
    let byte = |channel: f32| (channel * 255.).round() as u8;
    RgbColor {
        r: byte(rgba.r),
        g: byte(rgba.g),
        b: byte(rgba.b),
    }
}

/// The physical key GPUI names `key`, for the encoder to look up what to send.
fn key_for(key: &str) -> Key {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        const LETTERS: [Key; 26] = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        const DIGITS: [Key; 10] = [
            Key::Digit0,
            Key::Digit1,
            Key::Digit2,
            Key::Digit3,
            Key::Digit4,
            Key::Digit5,
            Key::Digit6,
            Key::Digit7,
            Key::Digit8,
            Key::Digit9,
        ];
        return match c.to_ascii_lowercase() {
            c @ 'a'..='z' => LETTERS
                .get(usize::from(c as u8 - b'a'))
                .copied()
                .unwrap_or(Key::Unidentified),
            c @ '0'..='9' => DIGITS
                .get(usize::from(c as u8 - b'0'))
                .copied()
                .unwrap_or(Key::Unidentified),
            '`' => Key::Backquote,
            '\\' => Key::Backslash,
            '[' => Key::BracketLeft,
            ']' => Key::BracketRight,
            ',' => Key::Comma,
            '=' => Key::Equal,
            '-' => Key::Minus,
            '.' => Key::Period,
            '\'' => Key::Quote,
            ';' => Key::Semicolon,
            '/' => Key::Slash,
            ' ' => Key::Space,
            _ => Key::Unidentified,
        };
    }
    const FUNCTION: [Key; 12] = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];
    match key {
        "enter" => Key::Enter,
        "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "escape" => Key::Escape,
        "space" => Key::Space,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        _ => (key.strip_prefix('f'))
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| FUNCTION.get(n.wrapping_sub(1)).copied())
            .unwrap_or(Key::Unidentified),
    }
}
