//! Terminals opened on a worktree: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt and drawn where the detail goes. They end with the app.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

#[cfg(unix)]
use but_tty::Detached;
use but_tty::{
    Colors, Cursor, Ghostty, Key, Mods, RgbColor, Row, Run, Shell, Size, Status, Terminal as Tty,
    Underline, Updates, mouse,
};
use futures::StreamExt;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, Entity, EventEmitter,
    FocusHandle, Focusable, Font, FontWeight, HitboxBehavior, Hsla, KeyDownEvent, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Rgba,
    ScrollWheelEvent, SharedString, StrikethroughStyle, Task, TextAlign, TextRun, UnderlineStyle,
    WeakEntity, Window, actions, canvas, div, prelude::*,
};

use crate::state::Store;
use crate::theme::Theme;
use crate::ui::{dp, fixed};

actions!(terminal, [NewTerminal]);

/// A terminal, numbered in the order the session opened them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub(crate) u64);

pub enum TerminalEvent {
    /// The shell named the terminal this.
    Retitled(SharedString),
    /// The program running is at this now.
    Restatused(Status),
    /// The shell exited.
    Exited,
}

/// The size text shows in when neither the setting nor Ghostty's config says.
const TEXT_SIZE: f32 = 13.;

/// How often the shell is resized at most while the grid's size keeps changing, as Ghostty
/// coalesces resizes, so dragging the window's edge or sliding the navigator has a full-screen
/// program redraw everything a few dozen times a second rather than every frame.
const RESIZE_EVERY: Duration = Duration::from_millis(25);

pub struct Terminal {
    tty: Tty,
    ghostty: Ghostty,
    /// The colours set, once set, with the ground and ink for what they leave unset.
    colors: Option<(Colors, RgbColor, RgbColor)>,
    /// Pixels scrolled that don't make a whole line yet.
    scrolled: Pixels,
    grid: Rc<Cell<Grid>>,
    held: Option<Held>,
    /// Scrolling a selection dragged past the top or bottom, a row a tick.
    autoscrolling: Option<Task<()>>,
    /// Drawing again once the program's hold on the screen times out, as nothing else may.
    hold_timeout: Option<Task<()>>,
    /// The grid's size as last measured, which the shell follows at most every `RESIZE_EVERY`.
    wanted: Size,
    /// Waiting out `RESIZE_EVERY` since the shell was last resized.
    resizing: Option<Task<()>>,
    focus: FocusHandle,
    store: WeakEntity<Store>,
    _updates: Task<()>,
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

    /// A terminal on the shell just started, coloured as `ghostty` says.
    pub fn new(
        shell: Shell,
        ghostty: Ghostty,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let colors = colors_of(Theme::get(cx));
        let (tty, updates) = Tty::new(shell, &colors.0, colors.1, colors.2)?;
        Ok(Self::showing(
            tty,
            updates,
            Some(colors),
            ghostty,
            store,
            cx,
        ))
    }

    /// A terminal on a shell the process this one was before exec handed over, showing what it
    /// showed then. Adopted before the window opens, which brings the theme, it's coloured as
    /// it's first drawn.
    #[cfg(unix)]
    pub fn adopt(
        detached: Detached,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let (tty, updates) = Tty::adopt(detached)?;
        Ok(Self::showing(
            tty,
            updates,
            None,
            Ghostty::load(),
            store,
            cx,
        ))
    }

    fn showing(
        tty: Tty,
        mut updates: Updates,
        colors: Option<(Colors, RgbColor, RgbColor)>,
        ghostty: Ghostty,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> Entity<Self> {
        let size = tty.size();
        cx.new(|cx: &mut Context<Self>| Self {
            tty,
            ghostty,
            colors,
            scrolled: fixed(0.),
            grid: Rc::new(Cell::new(Grid::opening(size))),
            held: None,
            autoscrolling: None,
            hold_timeout: None,
            wanted: size,
            resizing: None,
            focus: cx.focus_handle(),
            store,
            // The emulator reads the shell on a thread of its own, as Ghostty does, so a program
            // printing without pause never holds up the UI; this only shows what it last drew.
            _updates: cx.spawn(async move |this, cx| {
                while updates.next().await.is_some() {
                    if this.update(cx, |this, cx| this.updated(cx)).is_err() {
                        return;
                    }
                }
                this.update(cx, |_, cx| cx.emit(TerminalEvent::Exited)).ok();
            }),
        })
    }

    pub fn title(&self) -> SharedString {
        shown_title(&self.tty.title())
    }

    pub fn status(&self) -> Status {
        self.tty.status()
    }

    /// The status once the user has seen this.
    pub fn seen(&mut self) -> Status {
        self.tty.seen()
    }

    /// Whether this is on screen, for the emulator to draw only then.
    pub fn set_shown(&mut self, shown: bool) {
        self.tty.set_shown(shown);
    }

    /// Stops reading the shell, for another to adopt it, with what it showed until then.
    #[cfg(unix)]
    pub fn detach(&mut self) -> anyhow::Result<Detached> {
        self.tty.detach()
    }

    /// The emulator has more to show.
    fn updated(&mut self, cx: &mut Context<Self>) {
        if let Some(title) = self.tty.retitled() {
            cx.emit(TerminalEvent::Retitled(shown_title(&title)));
        }
        if let Some(status) = self.tty.restatused() {
            cx.emit(TerminalEvent::Restatused(status));
        }
        // A moment past it, as a redraw before it changes nothing and so draws nothing to wake
        // this again.
        self.hold_timeout = self.tty.hold_left().map(|left| {
            cx.spawn(async move |this, cx| {
                let past = left + Duration::from_millis(1);
                cx.background_executor().timer(past).await;
                this.update(cx, |this, _| this.tty.redraw()).ok();
            })
        });
        cx.notify();
    }

    /// Resizes the shell to `size` now, or once `RESIZE_EVERY` has passed since it last was, to
    /// the size wanted then.
    fn resize(&mut self, size: Size, cx: &mut Context<Self>) {
        self.wanted = size;
        if self.resizing.is_some() {
            return;
        }
        self.tty.resize(size);
        cx.notify();
        self.resizing = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESIZE_EVERY).await;
            (this.update(cx, |this, cx| {
                this.resizing = None;
                if this.wanted != this.tty.size() {
                    this.resize(this.wanted, cx);
                }
            }))
            .ok();
        }));
    }

    fn typed(&mut self, cx: &mut Context<Self>) {
        self.scrolled = fixed(0.);
        cx.notify();
    }

    fn text_size(&self, cx: &App) -> f32 {
        let chosen = (self.store.upgrade()).and_then(|store| store.read(cx).prefs.terminal_text);
        (chosen.map(f32::from))
            .or(self.ghostty.font_size)
            .unwrap_or(TEXT_SIZE)
    }

    /// `theme`'s terminal colours, or the app's own ground and ink where they set none.
    fn set_colors(&mut self, theme: &Theme) {
        let colors = colors_of(theme);
        if self.colors.as_ref() == Some(&colors) {
            return;
        }
        (self.tty).set_colors(&colors.0, colors.1, colors.2);
        self.colors = Some(colors);
    }

    /// `position` in the window on the terminal's surface.
    fn surface(&self, position: Point<Pixels>) -> mouse::Position {
        self.grid.get().surface(position, self.tty.size())
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        // ⌘ chords stay the app's, but for copying, selecting all and pasting and, as Ghostty
        // sends them on macOS, deleting to the line's start and going to either end.
        if modifiers.platform {
            let alone = !(modifiers.shift || modifiers.alt || modifiers.control);
            match keystroke.key.as_str() {
                "c" if alone => {
                    if let Some(text) = self.tty.selected() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
                "a" if alone => self.tty.select_all(),
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.tty.paste(&text);
                    }
                }
                "backspace" if alone => self.tty.input(b"\x15"),
                "left" if alone => self.tty.input(b"\x01"),
                "right" if alone => self.tty.input(b"\x05"),
                _ => return,
            }
            self.typed(cx);
            cx.stop_propagation();
            return;
        }
        let text = (keystroke.key_char.clone()).filter(|text| !text.chars().any(char::is_control));
        let mut chars = keystroke.key.chars();
        let unshifted = match keystroke.key.as_str() {
            "space" => Some(' '),
            _ => chars.next().filter(|_| chars.next().is_none()),
        };
        (self.tty).key(key_for(&keystroke.key), mods(modifiers), text, unshifted);
        self.typed(cx);
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
            let at = self.surface(event.position);
            self.tty.select_press(at, event.click_count > 1);
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
                let at = self.surface(event.position);
                self.tty.select_drag(at, rectangle(event.modifiers));
                self.autoscroll(event.position, cx);
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
                let at = self.surface(event.position);
                self.tty.select_release(at);
                self.autoscrolling = None;
            }
            None => {}
        }
    }

    /// Ticks while the selection is dragged past the top or bottom, about as often as Ghostty's
    /// do. Told by where the pointer is, as the emulator learns of the drag only after.
    fn autoscroll(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.past_edge(position) {
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
                    if !this.past_edge(at) {
                        return false;
                    }
                    let surface = this.surface(at);
                    cx.notify();
                    this.tty.select_autoscroll(surface, rectangle(modifiers));
                    true
                });
                if !ticked.unwrap_or(false) {
                    break;
                }
            }
            (this.update(cx, |this, _| this.autoscrolling = None)).ok();
        }));
    }

    /// Whether `position` in the window is above the grid's top or below its bottom.
    fn past_edge(&self, position: Point<Pixels>) -> bool {
        let grid = self.grid.get();
        let y = position.y - grid.origin.y;
        y < fixed(0.) || y >= grid.cell.height * f32::from(self.tty.size().rows)
    }

    /// Whether the program tracking the mouse is told of it. ⇧ keeps it for selecting, as
    /// Ghostty's `mouse-shift-capture` does by default.
    fn reports(&self, modifiers: Modifiers) -> bool {
        !modifiers.shift && self.tty.tracks_mouse()
    }

    fn report(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        position: Point<Pixels>,
        modifiers: Modifiers,
    ) {
        let at = self.surface(position);
        self.tty.mouse(action, button, at, mods(modifiers));
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let line = fixed(f32::from(self.tty.size().cell_height.max(1)));
        self.scrolled += event.delta.pixel_delta(line).y;
        let lines = (self.scrolled / line).trunc();
        if lines != 0. {
            self.scrolled -= line * lines;
            let at = self.surface(event.position);
            self.tty.scroll(lines, at, mods(event.modifiers));
            cx.notify();
        }
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        self.set_colors(&theme);
        let text_size = self.text_size(cx);
        let font = theme.terminal_font.clone();
        let bold = theme.terminal_bold;
        let text_system = window.text_system();
        let font_size = theme.scaled(text_size);
        let font_id = text_system.resolve_font(&font);
        let cell_width = (text_system.advance(font_id, font_size, 'm'))
            .map_or(font_size * 0.6, |advance| advance.width);
        // A row is as tall as the font, to the device pixel, as in Ghostty, so the font's box
        // drawing joins up from row to row.
        let font_height =
            text_system.ascent(font_id, font_size) + text_system.descent(font_id, font_size).abs();
        let scale = window.scale_factor();
        let cell_height = (font_height * scale).round() / scale;
        let focused = self.focus.is_focused(window);
        let pointer_style = match self.tty.tracks_mouse() {
            true => CursorStyle::Arrow,
            false => CursorStyle::IBeam,
        };
        let frame = self.tty.frame();
        let rows = frame.clone();
        let this = cx.entity().downgrade();
        let measured = Rc::new(Cell::new(self.wanted));
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
                if size != measured.get() {
                    measured.set(size);
                    window.defer(cx, move |_, cx| {
                        this.update(cx, |this, cx| this.resize(size, cx)).ok();
                    });
                }
                window.insert_hitbox(bounds, HitboxBehavior::Normal)
            },
            move |bounds, hitbox, window, cx| {
                let cell = gpui::size(cell_width, cell_height);
                paint_rows(&rows.rows, bounds.origin, cell, bold, window, cx);
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
        let (background, foreground) = (hsla(frame.background), hsla(frame.foreground));
        let cursor_color = hsla(frame.cursor_color);
        let cursor = frame.cursor.clone().map(|Cursor { x, y, text }| {
            let at = div()
                .absolute()
                .left(cell_width * f32::from(x))
                .top(cell_height * f32::from(y))
                .w(cell_width)
                .h(cell_height);
            if focused {
                at.bg(cursor_color).text_color(background).child(text)
            } else {
                at.border_1().border_color(cursor_color)
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
            .bg(background)
            .text_color(foreground)
            .font_family(font.family)
            .when(!font.features.tag_value_list().is_empty(), |el| {
                el.font_features(font.features)
            })
            .text_size(dp(text_size))
            .line_height(cell_height)
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .child(grid)
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

/// `theme`'s terminal colours, with the app's own ground and ink for what they leave unset.
fn colors_of(theme: &Theme) -> (Colors, RgbColor, RgbColor) {
    let terminal = theme.terminal.clone();
    (terminal, rgb_color(theme.bg_1), rgb_color(theme.text_1))
}

/// Whether a drag selects a block, with ⌥ on macOS and ⌃⌥ elsewhere, as in Ghostty.
fn rectangle(modifiers: Modifiers) -> bool {
    modifiers.alt && (cfg!(target_os = "macos") || modifiers.control)
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

/// What the shell titled the terminal, or that it's one.
fn shown_title(title: &str) -> SharedString {
    match title.trim() {
        "" => "Terminal".into(),
        title => title.to_owned().into(),
    }
}

/// Paints `rows` on the grid from `origin`, bold text at `bold`, every background before any
/// text so no cell's covers a glyph reaching into it. Each run's text is shaped on its own and
/// placed at its column, as Ghostty does. GPUI keeps a shaped line while it's drawn frame after
/// frame, so a run is shaped again only when it changes, not whenever anything on its row
/// does, as a relative line number does at every move.
fn paint_rows(
    rows: &[Arc<Row>],
    origin: Point<Pixels>,
    cell: gpui::Size<Pixels>,
    bold: FontWeight,
    window: &mut Window,
    cx: &mut App,
) {
    let at = |column: u16, row: usize| {
        gpui::point(
            origin.x + cell.width * f32::from(column),
            origin.y + cell.height * row as f32,
        )
    };
    for (y, row) in rows.iter().enumerate() {
        for (columns, color) in backgrounds(row) {
            let size = gpui::size(
                cell.width * f32::from(columns.end - columns.start),
                cell.height,
            );
            let bounds = Bounds::new(at(columns.start, y), size);
            window.paint_quad(gpui::fill(bounds, hsla(color)));
        }
    }
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let regular = text_style.font();
    let bold = Font {
        weight: bold,
        ..regular.clone()
    };
    for (y, row) in rows.iter().enumerate() {
        for (column, text, run) in shown_text(row) {
            let style = &run.style;
            let mut color = hsla(style.fg);
            if style.faint {
                color.fade_out(0.4);
            }
            let text_run = TextRun {
                len: text.len(),
                font: if style.bold { &bold } else { &regular }.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            // Held to the cells, as a glyph from a fallback font may be wider or narrower.
            let line = (window.text_system()).shape_line(
                text.to_owned().into(),
                font_size,
                &[text_run],
                Some(cell.width),
            );
            let origin = at(column, y);
            (line.paint(origin, cell.height, TextAlign::Left, None, window, cx)).ok();
            // Across the run's cells, where GPUI's own reach only as far as the glyphs do, which
            // a wide character's from a fallback font may fall short of. At GPUI's heights.
            let width = cell.width * f32::from(run.columns.end - column);
            if style.underline != Underline::None {
                let offset = gpui::underline_y_offset(cell.height, line.ascent, line.descent);
                let underline = UnderlineStyle {
                    thickness: fixed(1.),
                    color: Some(color),
                    wavy: style.underline == Underline::Curly,
                };
                window.paint_underline(origin + gpui::point(fixed(0.), offset), width, &underline);
            }
            if style.strikethrough {
                let baseline = (cell.height - line.ascent - line.descent) / 2. + line.ascent;
                let offset = (line.ascent * 0.5 + baseline) * 0.5;
                let strikethrough = StrikethroughStyle {
                    thickness: fixed(1.),
                    color: Some(color),
                };
                let at = origin + gpui::point(fixed(0.), offset);
                window.paint_strikethrough(at, width, &strikethrough);
            }
        }
    }
}

/// The columns of `row` with a background, those side by side in one colour joined so no
/// seam shows between them.
fn backgrounds(row: &Row) -> Vec<(Range<u16>, RgbColor)> {
    let mut joined: Vec<(Range<u16>, RgbColor)> = Vec::new();
    for run in &row.runs {
        let Some(color) = run.style.bg else {
            continue;
        };
        match joined.last_mut() {
            Some((columns, last)) if *last == color && columns.end == run.columns.start => {
                columns.end = run.columns.end;
            }
            _ => joined.push((run.columns.clone(), color)),
        }
    }
    joined
}

/// The text each run of `row` shows, with the column it starts at. Blanks at either end are
/// left to the background, so a run shows the same text however wide the row is, unless
/// they're underlined or struck through.
fn shown_text(row: &Row) -> impl Iterator<Item = (u16, &str, &Run)> {
    row.runs.iter().filter_map(|run| {
        let text = row.text.get(run.text.clone())?;
        let style = &run.style;
        if style.underline != Underline::None || style.strikethrough {
            return Some((run.columns.start, text, run));
        }
        let trimmed = text.trim_start_matches(' ');
        let column = run.columns.start + (text.len() - trimmed.len()) as u16;
        let trimmed = trimmed.trim_end_matches(' ');
        (!trimmed.is_empty()).then_some((column, trimmed, run))
    })
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
    use but_tty::Style;

    use super::*;

    #[test]
    fn a_point_over_a_fractional_cell_lands_in_that_cell_on_the_surface() {
        let grid = Grid {
            origin: gpui::point(fixed(100.), fixed(50.)),
            cell: gpui::size(fixed(7.8), fixed(16.)),
        };
        let over_column_61 = gpui::point(fixed(100. + 7.8 * 60.5), fixed(58.));
        let at = grid.surface(over_column_61, Terminal::OPENING);
        assert_eq!(
            (at.x / 8.) as u16 + 1,
            61,
            "7.8px cells drawn map onto the 8px cells the encoders count in, not column 59"
        );
    }

    /// A row of `runs` side by side, each its text over that many columns.
    fn row(runs: &[(&str, u16, Style)]) -> Row {
        let mut row = Row::default();
        let mut column = 0;
        for &(text, columns, style) in runs {
            let start = row.text.len();
            row.text.push_str(text);
            (row.runs).push(Run {
                text: start..row.text.len(),
                columns: column..column + columns,
                style,
            });
            column += columns;
        }
        row
    }

    const PLAIN: Style = Style {
        fg: RgbColor { r: 0, g: 0, b: 0 },
        bg: None,
        bold: false,
        underline: Underline::None,
        strikethrough: false,
        faint: false,
    };

    #[test]
    fn a_run_shows_the_same_text_however_wide_its_row_is() {
        let shown = |row: &Row| -> Vec<(u16, String)> {
            (shown_text(row).map(|(column, text, _)| (column, text.to_owned()))).collect()
        };
        let number = Style {
            fg: RgbColor { r: 9, g: 9, b: 9 },
            ..PLAIN
        };
        let narrow = row(&[("  12 ", 5, number), ("fn main()  ", 11, PLAIN)]);
        let wide = row(&[("  12 ", 5, number), ("fn main()        ", 17, PLAIN)]);
        assert_eq!(
            shown(&narrow),
            [(2, "12".to_owned()), (5, "fn main()".to_owned())],
            "blanks are left out, each run starting where its text does"
        );
        assert_eq!(
            shown(&narrow),
            shown(&wide),
            "the blanks a wider row pads with don't change what's shaped"
        );
        let underlined = Style {
            underline: Underline::Single,
            ..PLAIN
        };
        let blank = row(&[("   ", 3, underlined)]);
        assert_eq!(
            shown(&blank),
            [(0, "   ".to_owned())],
            "blanks underlined show their line"
        );
    }

    #[test]
    fn backgrounds_side_by_side_in_one_colour_are_joined() {
        let on = |r| Style {
            bg: Some(RgbColor { r, g: 0, b: 0 }),
            ..PLAIN
        };
        let row = row(&[
            ("use", 3, on(1)),
            (" crate", 6, on(1)),
            ("::", 2, PLAIN),
            ("x", 1, on(1)),
            ("y", 1, on(2)),
        ]);
        let joined: Vec<_> = (backgrounds(&row).into_iter())
            .map(|(columns, color)| (columns, color.r))
            .collect();
        assert_eq!(
            joined,
            [(0..9, 1), (11..12, 1), (12..13, 2)],
            "one quad across runs of one colour, so no seam shows, but not across a gap"
        );
    }
}
