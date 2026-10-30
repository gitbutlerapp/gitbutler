//! Terminals opened on a worktree: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt and drawn where the detail goes. They end with the app.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    App, Context, CursorStyle, DispatchPhase, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, HighlightStyle, HitboxBehavior, Hsla, KeyDownEvent, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Rgba, ScrollWheelEvent,
    SharedString, StrikethroughStyle, StyledText, Task, UnderlineStyle, WeakEntity, Window,
    actions, canvas, div, prelude::*,
};
use libghostty_vt::fmt::{Format, Formatter, FormatterOptions};
use libghostty_vt::key::{self, Key, Mods};
use libghostty_vt::mouse;
use libghostty_vt::render::{CellIterator, Dirty, RowIterator};
use libghostty_vt::screen::{CellWide, Screen};
use libghostty_vt::selection::gesture::{self, Gesture};
use libghostty_vt::style::{RgbColor, Underline};
use libghostty_vt::terminal::{Mode, Point as GridPoint, PointCoordinate, ScrollViewport};
use libghostty_vt::{RenderState, TerminalOptions, paste};

#[cfg(unix)]
use crate::backend::terminal::Kept;
use crate::backend::terminal::{Colors, Ghostty, Opened, Pty, Size};
use crate::state::Store;
use crate::theme::Theme;
use crate::ui::{dp, fixed};

actions!(terminal, [NewTerminal]);

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
    grid: Rc<Cell<Grid>>,
    held: Option<Held>,
    /// Scrolling a selection dragged past the top or bottom, a row a tick.
    autoscrolling: Option<Task<()>>,
    focus: FocusHandle,
    /// It takes the keys once it's first drawn, as it opens.
    focus_when_drawn: bool,
    store: WeakEntity<Store>,
    output: Rc<RefCell<mpsc::UnboundedReceiver<Vec<u8>>>>,
    _output: Task<()>,
}

/// What a button pressed over the terminal does until it's let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    /// The program tracking the mouse is told where it goes.
    Reported,
    /// It selects, as Ghostty does, from the pointer last over `at` with `modifiers` held.
    Selecting {
        at: Point<Pixels>,
        modifiers: Modifiers,
    },
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

    /// A terminal on the shell `opened` started.
    pub fn new(
        opened: Opened,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let Opened {
            pty,
            output,
            ghostty,
            size,
        } = opened;
        let vt = Vt::new(size)?;
        let output = Rc::new(RefCell::new(output));
        let read = output.clone();
        Ok(cx.new(|cx: &mut Context<Self>| Self {
            title: "Terminal".into(),
            vt,
            pty,
            ghostty,
            size,
            dark: None,
            scrolled: fixed(0.),
            grid: Rc::new(Cell::new(Grid::opening(size))),
            held: None,
            autoscrolling: None,
            focus: cx.focus_handle(),
            focus_when_drawn: true,
            store,
            output,
            _output: cx.spawn(async move |this, cx| {
                let next =
                    || futures::future::poll_fn(|task| read.borrow_mut().poll_next_unpin(task));
                while let Some(mut chunk) = next().await {
                    while let Ok(more) = read.borrow_mut().try_recv() {
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

    /// A terminal on a shell the process this one was before exec handed over, showing what it
    /// showed then.
    #[cfg(unix)]
    pub fn adopt(
        kept: Kept,
        size: Size,
        screen: &str,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let terminal = Self::new(crate::backend::terminal::adopt(kept, size)?, store, cx)?;
        terminal.update(cx, |terminal, cx| terminal.feed(screen.as_bytes(), cx));
        Ok(terminal)
    }

    /// Stops reading the shell, for another to adopt it, with what it showed until then.
    #[cfg(unix)]
    pub fn hand_off(&mut self, cx: &mut Context<Self>) -> anyhow::Result<(Kept, Size, String)> {
        let kept = self.pty.keep()?;
        let left: Vec<u8> = {
            let mut output = self.output.borrow_mut();
            std::iter::from_fn(|| output.try_recv().ok())
                .flatten()
                .collect()
        };
        self.feed(&left, cx);
        let screen = String::from_utf8_lossy(&self.vt.screen()?).into_owned();
        Ok((kept, self.size, screen))
    }

    fn feed(&mut self, output: &[u8], cx: &mut Context<Self>) {
        self.vt.write(output);
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
        self.vt.resize(size);
        self.pty.resize(size).ok();
        cx.notify();
    }

    fn input(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if bytes.is_empty() {
            return;
        }
        self.vt.term.scroll_viewport(ScrollViewport::Bottom);
        self.vt.term.set_selection(None).ok();
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
        // ⌘ chords stay the app's, but for pasting and, as Ghostty sends them on macOS, deleting
        // to the line's start and going to either end.
        if modifiers.platform {
            let alone = !(modifiers.shift || modifiers.alt || modifiers.control);
            let bytes = match keystroke.key.as_str() {
                "v" => (cx.read_from_clipboard())
                    .and_then(|item| item.text())
                    .map(|text| self.vt.paste(&text)),
                "backspace" if alone => Some(b"\x15".to_vec()),
                "left" if alone => Some(b"\x01".to_vec()),
                "right" if alone => Some(b"\x05".to_vec()),
                _ => return,
            };
            if let Some(bytes) = bytes {
                self.input(&bytes, cx);
            }
            cx.stop_propagation();
            return;
        }
        let text = (keystroke.key_char.clone()).filter(|text| !text.chars().any(char::is_control));
        let mut chars = keystroke.key.chars();
        let unshifted = chars.next().filter(|_| chars.next().is_none());
        let bytes = self
            .vt
            .key(key_for(&keystroke.key), mods(modifiers), text, unshifted);
        self.input(&bytes, cx);
        cx.stop_propagation();
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if self.reports(event.modifiers) {
            let button = button(event.button);
            self.report(
                mouse::Action::Press,
                button,
                event.position,
                event.modifiers,
            );
            self.held = Some(Held::Reported);
        } else if event.button == MouseButton::Left {
            let at = self.grid.get().surface(event.position, self.size);
            let repeated = event.click_count > 1;
            self.vt.select_press(at, self.size, repeated).ok();
            self.held = Some(Held::Selecting {
                at: event.position,
                modifiers: event.modifiers,
            });
            cx.notify();
        }
    }

    /// While a button pressed over the terminal is held it follows the pointer anywhere, first
    /// in line so nothing under the pointer reacts; otherwise only over the terminal.
    fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        phase: DispatchPhase,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        match (self.held, phase) {
            (Some(Held::Reported), DispatchPhase::Capture) => {
                let button = event.pressed_button.and_then(button);
                self.report(
                    mouse::Action::Motion,
                    button,
                    event.position,
                    event.modifiers,
                );
                cx.stop_propagation();
            }
            (Some(Held::Selecting { .. }), DispatchPhase::Capture) => {
                self.held = Some(Held::Selecting {
                    at: event.position,
                    modifiers: event.modifiers,
                });
                let at = self.grid.get().surface(event.position, self.size);
                let rectangle = rectangle(event.modifiers);
                self.vt.select_drag(at, self.size, rectangle).ok();
                self.autoscroll(cx);
                cx.notify();
                cx.stop_propagation();
            }
            (None, DispatchPhase::Bubble) if hovered && self.reports(event.modifiers) => {
                self.report(mouse::Action::Motion, None, event.position, event.modifiers);
            }
            _ => {}
        }
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, phase: DispatchPhase) {
        if phase != DispatchPhase::Capture {
            return;
        }
        match self.held.take() {
            Some(Held::Reported) => {
                let button = button(event.button);
                self.report(
                    mouse::Action::Release,
                    button,
                    event.position,
                    event.modifiers,
                );
            }
            Some(Held::Selecting { .. }) => {
                let at = self.grid.get().surface(event.position, self.size);
                self.vt.select_release(at, self.size).ok();
                self.autoscrolling = None;
            }
            None => {}
        }
    }

    /// Ticks while the selection's drag asks to scroll, about as often as Ghostty's do.
    fn autoscroll(&mut self, cx: &mut Context<Self>) {
        if !self.vt.autoscrolling() {
            self.autoscrolling = None;
            return;
        }
        if self.autoscrolling.is_some() {
            return;
        }
        self.autoscrolling = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(15))
                    .await;
                let ticked = this.update(cx, |this, cx| {
                    let Some(Held::Selecting { at, modifiers }) = this.held else {
                        return false;
                    };
                    let at = this.grid.get().surface(at, this.size);
                    let rectangle = rectangle(modifiers);
                    cx.notify();
                    (this.vt.select_autoscroll(at, this.size, rectangle)).is_ok()
                        && this.vt.autoscrolling()
                });
                if !ticked.unwrap_or(false) {
                    break;
                }
            }
            (this.update(cx, |this, _| this.autoscrolling = None)).ok();
        }));
    }

    /// Whether the program tracking the mouse is told of it. ⇧ keeps it for selecting, as
    /// Ghostty's `mouse-shift-capture` does by default.
    fn reports(&self, modifiers: Modifiers) -> bool {
        !modifiers.shift && self.vt.term.is_mouse_tracking().unwrap_or(false)
    }

    fn report(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        position: Point<Pixels>,
        modifiers: Modifiers,
    ) {
        let at = self.grid.get().surface(position, self.size);
        let bytes = self.vt.mouse(action, button, at, mods(modifiers));
        if !bytes.is_empty() {
            self.pty.write(&bytes).ok();
        }
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let line = fixed(f32::from(self.size.cell_height.max(1)));
        self.scrolled += event.delta.pixel_delta(line).y;
        let lines = (self.scrolled / line).trunc();
        if lines != 0. {
            self.scrolled -= line * lines;
            if self.vt.term.is_mouse_tracking().unwrap_or(false) {
                let at = self.grid.get().surface(event.position, self.size);
                let bytes = self.vt.wheel(lines, at, mods(event.modifiers));
                self.pty.write(&bytes).ok();
            } else if let Some(arrows) = self.vt.alternate_scroll(lines) {
                self.pty.write(&arrows).ok();
            } else {
                (self.vt.term).scroll_viewport(ScrollViewport::Delta(-(lines as isize)));
            }
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
        let pointer_style = match self.vt.term.is_mouse_tracking().unwrap_or(false) {
            true => CursorStyle::Arrow,
            false => CursorStyle::IBeam,
        };
        let frame = (self.vt.frame(self.ghostty.colors(theme.dark))).unwrap_or_default();
        let this = cx.entity().downgrade();
        let measured = Rc::new(Cell::new(self.size));
        let drawn = self.grid.clone();
        let pointer = this.clone();
        let grid = canvas(
            move |bounds, window, cx| {
                drawn.set(Grid {
                    origin: bounds.origin,
                    cell: gpui::size(cell_width, cell_height),
                });
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
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |_, hitbox, window, _| {
                window.on_mouse_event({
                    let this = pointer.clone();
                    move |event: &MouseMoveEvent, phase, window, cx| {
                        let hovered = hitbox.is_hovered(window);
                        (this.update(cx, |this, cx| this.mouse_move(event, phase, hovered, cx)))
                            .ok();
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    pointer
                        .update(cx, |this, _| this.mouse_up(event, phase))
                        .ok();
                });
            },
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
            .on_any_mouse_down(cx.listener(Self::mouse_down))
            .on_scroll_wheel(cx.listener(Self::scroll))
            .cursor(pointer_style)
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

/// Where the grid was last drawn in the window, and the size its cells were drawn at.
#[derive(Clone, Copy)]
struct Grid {
    origin: Point<Pixels>,
    cell: gpui::Size<Pixels>,
}

impl Grid {
    /// Until it's drawn, cells are the size the shell was told.
    fn opening(size: Size) -> Self {
        Self {
            origin: Point::default(),
            cell: gpui::size(
                fixed(f32::from(size.cell_width)),
                fixed(f32::from(size.cell_height)),
            ),
        }
    }

    /// `position` in the window on the surface the encoders see, whose cells are `size`'s
    /// whole pixels rather than the fractional ones drawn.
    fn surface(self, position: Point<Pixels>, size: Size) -> mouse::Position {
        let at = position - self.origin;
        mouse::Position {
            x: at.x / self.cell.width * f32::from(size.cell_width),
            y: at.y / self.cell.height * f32::from(size.cell_height),
        }
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

    /// Takes what the program wrote, and the mouse tracking it may have asked for with it.
    /// Taking that clears which cell motion was last told of, so it's taken here rather than
    /// with each motion.
    // ponytail: a program that writes as it's told of motion is told of each move within a cell;
    // take the tracking per event once Ghostty's setopt_from_terminal keeps the cell, as setopt
    // does, when nothing changed.
    fn write(&mut self, bytes: &[u8]) {
        self.term.vt_write(bytes);
        self.mouse.set_options_from_terminal(&self.term);
    }

    /// Sizes the emulator and, so motion is told of once a cell, the mouse encoder only when
    /// the size changes.
    fn resize(&mut self, size: Size) {
        let cell = (u32::from(size.cell_width), u32::from(size.cell_height));
        (self.term)
            .resize(size.cols, size.rows, cell.0, cell.1)
            .ok();
        self.mouse.set_size(encoder_size(size));
    }

    /// `button` going `action` over `at` on the grid, for a program tracking the mouse; nothing
    /// for what it doesn't track, or motion within the cell it was last told of.
    fn mouse(
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
    fn select_press(
        &mut self,
        at: mouse::Position,
        size: Size,
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
            .grid_ref(GridPoint::Viewport(viewport(at, size)))?;
        let selection = self.press.apply(&mut self.selecting, &self.term, cell)?;
        self.term.set_selection(selection.as_ref())?;
        Ok(())
    }

    /// The press dragged to `at` on the surface; a block between it and where it began if
    /// `rectangle`.
    fn select_drag(
        &mut self,
        at: mouse::Position,
        size: Size,
        rectangle: bool,
    ) -> libghostty_vt::error::Result<()> {
        (self.drag)
            .set_position(f64::from(at.x), f64::from(at.y))?
            .set_rectangle(rectangle)?;
        let cell = self
            .term
            .grid_ref(GridPoint::Viewport(viewport(at, size)))?;
        let selection = self
            .drag
            .apply(&mut self.selecting, &self.term, cell, geometry(size))?;
        self.term.set_selection(selection.as_ref())?;
        Ok(())
    }

    fn select_release(
        &mut self,
        at: mouse::Position,
        size: Size,
    ) -> libghostty_vt::error::Result<()> {
        let cell = self
            .term
            .grid_ref(GridPoint::Viewport(viewport(at, size)))?;
        (self.release).apply(&mut self.selecting, &self.term, Some(cell))
    }

    /// Whether the selection's drag went past the top or bottom, for the viewport to follow.
    fn autoscrolling(&self) -> bool {
        (self.selecting.autoscroll(&self.term)).is_ok_and(|to| to != gesture::Autoscroll::None)
    }

    /// Scrolls a row the way the drag went past, and takes the selection on to `at` on the
    /// surface. A press the scrolling lost leaves the selection as it was.
    fn select_autoscroll(
        &mut self,
        at: mouse::Position,
        size: Size,
        rectangle: bool,
    ) -> libghostty_vt::error::Result<()> {
        (self.tick)
            .set_position(f64::from(at.x), f64::from(at.y))?
            .set_rectangle(rectangle)?;
        let cell = viewport(at, size);
        let selection = (self.tick).apply(&mut self.selecting, &self.term, cell, geometry(size))?;
        if let Some(selection) = selection {
            self.term.set_selection(Some(&selection))?;
        }
        Ok(())
    }

    fn paste(&self, text: &str) -> Vec<u8> {
        let bracketed = self.term.mode(Mode::BRACKETED_PASTE).unwrap_or(false);
        let mut data = text.as_bytes().to_vec();
        let mut bytes = vec![0; data.len() + 12];
        let written = paste::encode(&mut data, bracketed, &mut bytes).unwrap_or(0);
        bytes.truncate(written);
        bytes
    }

    /// The screen with its scrollback, modes and title, as what builds the same in a fresh one.
    /// An alternate screen comes without the one behind it.
    fn screen(&self) -> libghostty_vt::error::Result<Vec<u8>> {
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
        let mut screen = format!("\x1b]2;{title}\x1b\\").into_bytes();
        screen.extend_from_slice(&Formatter::new(&self.term, options)?.format_alloc(None)?);
        Ok(screen)
    }

    /// What to draw, with what's selected in `chosen`'s selection colours, or the default
    /// colours swapped where it sets none, as Ghostty does.
    fn frame(&mut self, chosen: &Colors) -> libghostty_vt::error::Result<Frame> {
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
        let selected_bg = chosen.selection_background.unwrap_or(colors.foreground);
        let selected_fg = chosen.selection_foreground.unwrap_or(colors.background);
        let mut rows = self.rows.update(&snapshot)?;
        let mut cell_text = String::new();
        let mut y = 0;
        while let Some(row) = rows.next() {
            let mut text = String::new();
            let mut runs = Runs::new();
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
                let style = cell.style()?;
                let mut fg = cell.fg_color()?.unwrap_or(colors.foreground);
                let mut bg = cell.bg_color()?.unwrap_or(colors.background);
                if style.inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if selected.as_ref().is_some_and(|range| range.contains(&at)) {
                    (fg, bg) = (selected_fg, selected_bg);
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

/// Whether a drag selects a block, with ⌥ on macOS and ⌃⌥ elsewhere, as in Ghostty.
fn rectangle(modifiers: Modifiers) -> bool {
    modifiers.alt && (cfg!(target_os = "macos") || modifiers.control)
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

fn button(button: MouseButton) -> Option<mouse::Button> {
    match button {
        MouseButton::Left => Some(mouse::Button::Left),
        MouseButton::Right => Some(mouse::Button::Right),
        MouseButton::Middle => Some(mouse::Button::Middle),
        MouseButton::Navigate(_) => None,
    }
}

fn mods(modifiers: Modifiers) -> Mods {
    let mut mods = Mods::empty();
    mods.set(Mods::SHIFT, modifiers.shift);
    mods.set(Mods::ALT, modifiers.alt);
    mods.set(Mods::CTRL, modifiers.control);
    mods
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_reports_to_a_program_tracking_the_mouse() {
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
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
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
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
    fn the_wheel_reports_the_cell_drawn_under_it_when_cells_are_fractional() {
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
        vt.write(b"\x1b[?1003h\x1b[?1006h");
        let grid = Grid {
            origin: gpui::point(fixed(100.), fixed(50.)),
            cell: gpui::size(fixed(7.8), fixed(16.)),
        };
        let over_column_61 = gpui::point(fixed(100. + 7.8 * 60.5), fixed(58.));
        let at = grid.surface(over_column_61, Terminal::OPENING);
        assert_eq!(
            vt.wheel(1., at, Mods::empty()),
            b"\x1b[<64;61;1M".to_vec(),
            "7.8px cells drawn report as the 8px cells the encoder counts in, not 59"
        );
    }

    #[test]
    fn selected_cells_draw_in_the_default_colours_swapped() {
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
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
        let (text, runs) = &frame.rows[0];
        let backgrounds: Vec<_> = (runs.iter())
            .map(|(range, style)| (text[range.clone()].trim_end(), style.background_color))
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
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
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
        let (text, runs) = &frame.rows[0];
        (runs.iter())
            .filter(|(_, style)| style.background_color.is_some())
            .map(|(range, _)| text[range.clone()].trim_end().to_owned())
            .collect()
    }

    #[test]
    fn a_double_click_dragged_selects_by_word() {
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
        vt.write(b"hello brave new world");
        let size = Terminal::OPENING;
        let over = |col: f32| mouse::Position {
            x: col * 8. + 4.,
            y: 8.,
        };
        vt.select_press(over(7.), size, false).unwrap();
        vt.select_release(over(7.), size).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            Vec::<String>::new(),
            "a click selects nothing"
        );
        vt.select_press(over(7.), size, true).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            ["brave"],
            "a second click in a row takes the word"
        );
        vt.select_drag(over(18.), size, false).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            ["brave new world"],
            "dragging from it takes whole words up to the one under the pointer"
        );
        vt.select_release(over(18.), size).unwrap();
        vt.select_press(over(7.), size, false).unwrap();
        assert_eq!(
            highlighted(&mut vt),
            Vec::<String>::new(),
            "a click a while later is a first click again, and clears it"
        );
    }

    #[test]
    fn a_selection_dragged_above_the_top_scrolls_up_a_row_a_tick() {
        let mut vt = Vt::new(Terminal::OPENING).unwrap();
        for n in 0..60 {
            vt.write(format!("line {n}\r\n").as_bytes());
        }
        let size = Terminal::OPENING;
        let row = |row: f32| mouse::Position {
            x: 4.,
            y: row * 16. + 8.,
        };
        let first_row = |vt: &mut Vt| {
            vt.frame(&Colors::default()).unwrap().rows[0]
                .0
                .trim_end()
                .to_owned()
        };
        assert_eq!(
            first_row(&mut vt),
            "line 37",
            "the screen shows the last lines"
        );
        vt.select_press(row(5.), size, false).unwrap();
        vt.select_drag(row(-1.), size, false).unwrap();
        assert!(vt.autoscrolling(), "above the top it asks to scroll");
        for _ in 0..3 {
            vt.select_autoscroll(row(-1.), size, false).unwrap();
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
        vt.select_release(row(-1.), size).unwrap();
        assert!(!vt.autoscrolling(), "letting go stops it");
    }

    fn plain(vt: &Vt) -> String {
        let options = FormatterOptions::new().with_format(Format::Plain);
        let text = Formatter::new(&vt.term, options)
            .unwrap()
            .format_alloc(None)
            .unwrap();
        String::from_utf8(text.to_vec()).unwrap()
    }

    #[test]
    fn a_screen_written_into_a_fresh_terminal_builds_the_same() {
        let size = Size {
            cols: 20,
            rows: 4,
            ..Terminal::OPENING
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
            "working",
            "the title carries over"
        );
        assert!(
            fresh.term.mode(Mode::BRACKETED_PASTE).unwrap(),
            "the modes the shell set carry over"
        );
    }
}
