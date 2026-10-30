//! Terminals opened on a worktree: the user's shell on a pseudo-terminal, emulated by
//! libghostty-vt and drawn where the detail goes. They end with the app.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

#[cfg(unix)]
use but_tty::Detached;
use but_tty::{
    Cursor, Ghostty, Key, Mods, Output, RgbColor, Shell, Size, Style, Terminal as Tty, Underline,
    mouse,
};
use futures::StreamExt;
use gpui::{
    App, ClipboardItem, Context, CursorStyle, DispatchPhase, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, HighlightStyle, HitboxBehavior, Hsla, KeyDownEvent, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Rgba,
    ScrollWheelEvent, SharedString, StrikethroughStyle, StyledText, Task, UnderlineStyle,
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
    /// The shell exited.
    Exited,
}

/// The size text shows in when neither the setting nor Ghostty's config says.
const TEXT_SIZE: f32 = 13.;

pub struct Terminal {
    tty: Tty,
    ghostty: Ghostty,
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
    output: Rc<RefCell<Output>>,
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

    /// A terminal on the shell just started, coloured as `ghostty` says.
    pub fn new(
        shell: Shell,
        ghostty: Ghostty,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let (tty, output) = Tty::new(shell)?;
        Ok(Self::showing(tty, output, ghostty, store, cx))
    }

    /// A terminal on a shell the process this one was before exec handed over, showing what it
    /// showed then.
    #[cfg(unix)]
    pub fn adopt(
        detached: Detached,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let (tty, output) = Tty::adopt(detached)?;
        Ok(Self::showing(tty, output, Ghostty::load(), store, cx))
    }

    fn showing(
        tty: Tty,
        output: Output,
        ghostty: Ghostty,
        store: WeakEntity<Store>,
        cx: &mut App,
    ) -> Entity<Self> {
        let size = tty.size();
        let output = Rc::new(RefCell::new(output));
        let read = output.clone();
        cx.new(|cx: &mut Context<Self>| Self {
            tty,
            ghostty,
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
                while let Some(chunk) = next().await {
                    if this.update(cx, |this, cx| this.feed(&chunk, cx)).is_err() {
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

    /// Stops reading the shell, for another to adopt it, with what it showed until then.
    #[cfg(unix)]
    pub fn detach(&mut self) -> anyhow::Result<Detached> {
        self.tty.detach(&mut self.output.borrow_mut())
    }

    fn feed(&mut self, output: &[u8], cx: &mut Context<Self>) {
        if let Some(title) = self.tty.feed(output) {
            cx.emit(TerminalEvent::Retitled(shown_title(&title)));
        }
        cx.notify();
    }

    fn resize(&mut self, size: Size, cx: &mut Context<Self>) {
        self.tty.resize(size);
        cx.notify();
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

    /// Ghostty's colours for `theme`'s appearance, or the app's own where Ghostty sets none.
    fn set_colors(&mut self, theme: &Theme) {
        if self.dark == Some(theme.dark) {
            return;
        }
        self.dark = Some(theme.dark);
        let colors = self.ghostty.colors(theme.dark);
        (self.tty).set_colors(colors, rgb_color(theme.bg_1), rgb_color(theme.text_1));
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
                let at = self.surface(event.position);
                self.tty.select_release(at);
                self.autoscrolling = None;
            }
            None => {}
        }
    }

    /// Ticks while the selection's drag asks to scroll, about as often as Ghostty's do.
    fn autoscroll(&mut self, cx: &mut Context<Self>) {
        if !self.tty.autoscrolling() {
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
                    let at = this.surface(at);
                    cx.notify();
                    this.tty.select_autoscroll(at, rectangle(modifiers))
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
        let pointer_style = match self.tty.tracks_mouse() {
            true => CursorStyle::Arrow,
            false => CursorStyle::IBeam,
        };
        let frame = self.tty.frame(self.ghostty.colors(theme.dark));
        let this = cx.entity().downgrade();
        let measured = Rc::new(Cell::new(self.tty.size()));
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
        let (background, foreground) = (hsla(frame.background), hsla(frame.foreground));
        let cursor_color = hsla(frame.cursor_color);
        let cursor = frame.cursor.map(|Cursor { x, y, text }| {
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
            .font_family(theme.mono_font.clone())
            .text_size(dp(text_size))
            .line_height(dp(line))
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .child(grid)
                    .children(frame.rows.into_iter().map(|row| {
                        let runs =
                            (row.runs.into_iter()).map(|(range, style)| (range, highlight(style)));
                        div()
                            .h(dp(line))
                            .whitespace_nowrap()
                            .child(StyledText::new(row.text).with_highlights(runs))
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

fn highlight(style: Style) -> HighlightStyle {
    let color = hsla(style.fg);
    HighlightStyle {
        color: Some(color),
        background_color: style.bg.map(hsla),
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

#[cfg(test)]
mod tests {
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
}
