//! Shared building blocks so every surface draws rows, buttons and counts the same way.

#[cfg(target_os = "linux")]
pub mod alert;
pub mod change_marks;
pub mod conflict;
pub mod diff;
pub mod emoji;
pub mod file_icons;
pub mod fold;
mod format;
pub mod markdown;
pub mod message_editor;
pub mod person;
pub mod review;
pub mod scrollbar;
pub mod text_input;
pub mod window_controls;
pub mod window_frame;

use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, CursorStyle, DispatchPhase, Div, DragMoveEvent,
    ElementId, EmptyView, Entity, FocusHandle, HighlightStyle, Hsla, MouseButton, MouseDownEvent,
    Pixels, Point, SharedString, Stateful, StyledText, TextLayout, UnderlineStyle, Window, div,
    prelude::*, relative,
};

use crate::state::{Area, Pane, Store};
use crate::theme::{Theme, tokens};
pub use but_ui_gpui::*;
pub use format::*;

/// `el`, a header the window is dragged by, as a title bar is: the app draws its own over
/// macOS's and has none elsewhere. Windows drags it itself, and on macOS and Linux the app starts
/// the move once the pointer moves with the button held, so a click there is still a click. A
/// double click does what the platform's title bars do: on macOS what System Settings says (zoom,
/// minimize or nothing), elsewhere maximize or restore. On Linux a right click opens the window's
/// menu, as GNOME's title bars do.
pub fn drags_window(el: Div) -> Div {
    let el = el.window_control_area(gpui::WindowControlArea::Drag);
    // `holds_back_first_click` lets this one through, so the window drags from here straight
    // away, as from a title bar.
    #[cfg(target_os = "macos")]
    let el = el.capture_any_mouse_down(|event, _, _| {
        if event.first_mouse {
            FIRST_CLICK_ON_HEADER.set(true);
        }
    });
    #[cfg(not(windows))]
    let el = {
        use std::cell::Cell;
        thread_local! {
            /// The left button went down on a header and hasn't moved or come up since.
            static PRESSED: Cell<bool> = const { Cell::new(false) };
        }
        el.on_mouse_down(MouseButton::Left, |event, window, _| {
            match event.click_count {
                2 if cfg!(target_os = "macos") => window.titlebar_double_click(),
                2 => window.zoom_window(),
                _ => PRESSED.set(true),
            }
        })
        .on_mouse_down_out(|_, _, _| PRESSED.set(false))
        .on_mouse_up(MouseButton::Left, |_, _, _| PRESSED.set(false))
        .on_mouse_move(|_, window, _| {
            if PRESSED.replace(false) {
                window.start_window_move();
            }
        })
    };
    #[cfg(target_os = "linux")]
    let el = el.on_mouse_down(MouseButton::Right, |event, window, _| {
        window.show_window_menu(event.position)
    });
    el
}

thread_local! {
    /// The click that brought the window in front landed on a header (`drags_window`).
    static FIRST_CLICK_ON_HEADER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The window that was active when last drawn (`holds_back_first_click`).
    static DRAWN_ACTIVE: std::cell::Cell<Option<gpui::WindowId>> =
        const { std::cell::Cell::new(None) };
}

/// Whether `window` is active. A dev script's never are, so it says which counts as active.
fn is_active(window: &Window) -> bool {
    #[cfg(feature = "dev-script")]
    if crate::dev::scripted() {
        return crate::dev::is_active(window);
    }
    window.is_window_active()
}

/// Bring `window` in front, but for a dev script's, which AppKit would show.
pub fn activate(window: &mut Window) {
    #[cfg(feature = "dev-script")]
    if crate::dev::scripted() {
        return crate::dev::activate(window);
    }
    window.activate_window();
}

/// Whether `event` is the press that brings the window in front, on macOS, which
/// `holds_back_first_click` holds back unless it's on a header. GPUI marks it `first_mouse`, but
/// leaves that on a later press too when a ctrl-click brought the window in front, so it counts
/// only while the window was last drawn inactive: GPUI hears that the window is active only after
/// handling the press that made it so.
pub fn brings_window_in_front(event: &MouseDownEvent, window: &Window) -> bool {
    cfg!(target_os = "macos")
        && event.first_mouse
        && DRAWN_ACTIVE.get() != Some(window.window_handle().window_id())
}

/// Holds back the click that brings the window in front, as macOS apps do, so it doesn't also
/// press what's under it: GPUI passes it on (`acceptsFirstMouse`). A header still takes it, to
/// drag the window. Drawn after everything else, as listeners run in the order they're drawn
/// while an event is captured, so the headers' run first, as does a menu's that closes it on a
/// press outside. Other capture listeners see it too, so one that acts checks
/// `brings_window_in_front`.
pub fn holds_back_first_click() -> impl IntoElement {
    gpui::deferred(
        gpui::canvas(
            |_, _, _| {},
            |_, (), window, _| {
                let id = window.window_handle().window_id();
                if is_active(window) {
                    DRAWN_ACTIVE.set(Some(id));
                } else if DRAWN_ACTIVE.get() == Some(id) {
                    DRAWN_ACTIVE.set(None);
                }
                window.on_mouse_event(|event: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && event.first_mouse {
                        let on_header = FIRST_CLICK_ON_HEADER.take();
                        if brings_window_in_front(event, window) && !on_header {
                            cx.stop_propagation();
                        }
                    }
                });
            },
        )
        .absolute()
        .inset_0(),
    )
    .with_priority(usize::MAX)
}

/// `el`, in a header the window is dragged by, still taking its own clicks: the drag area under
/// it would take them, and a double click there would zoom the window.
pub fn above_drag<E: InteractiveElement>(el: E) -> E {
    el.occlude()
}

/// Whether the app draws the window's minimize, maximize and close buttons: on Windows, and on
/// Linux where the desktop leaves the window's frame to the app, as GNOME does on Wayland.
pub fn draws_window_controls(window: &Window) -> bool {
    #[cfg(feature = "dev-script")]
    if crate::dev::linux_controls() {
        return true;
    }
    cfg!(target_os = "windows")
        || matches!(
            window.window_decorations(),
            gpui::Decorations::Client { .. }
        )
}

/// Whether macOS's traffic lights are at the window's top left, where the headers leave room for
/// them: not in full screen, where they come down with the menu bar, over the app.
pub fn has_traffic_lights(window: &Window) -> bool {
    // A dev script trying Linux's window buttons has the header laid out as Linux's.
    #[cfg(feature = "dev-script")]
    if crate::dev::linux_controls() {
        return false;
    }
    cfg!(target_os = "macos") && !window.is_fullscreen()
}

/// What "Reveal in Finder" is called where the file manager isn't Finder.
pub const REVEAL: &str = if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else if cfg!(windows) {
    "Reveal in File Explorer"
} else {
    "Reveal in File Manager"
};

pub use window_controls::WINDOW_CONTROLS_WIDTH;
/// How tall a list row is at Balanced density. Rows are drawn at `theme.sizes.control`, which
/// the density moves; this stays for minimums and for tests, which count at Balanced.
pub const ROW_HEIGHT: f32 = 28.;
pub const HEADER_HEIGHT: f32 = 40.;
pub const STATUS_BAR_HEIGHT: f32 = 30.;

/// Which of `len` tabs comes after the one at `at`, going round, if there's another.
pub fn tab_after(len: usize, at: usize, forward: bool) -> Option<usize> {
    (len > 1).then(|| match forward {
        true => (at + 1) % len,
        false => (at + len - 1) % len,
    })
}

/// How tall a tile's bar is, which says what it shows, while its row has more than one.
pub const TILE_BAR: f32 = 30.;

/// How far a tile's card sits in from its area: half the gap between two, as the navigator's
/// cards are apart, but-ui's `gap/stack`.
pub const TILE_MARGIN: f32 = tokens::GAP_STACK / 2.;

/// How far a tile's content sits in from its card's edge, its border included: as the
/// navigator's cards hold their rows, and clear of the rounded corners, as GPUI cuts what's
/// inside to a rectangle.
pub const TILE_PAD: f32 = 4.;

/// `el` absolutely over the room it's in, `by` in from every side.
pub fn inset_by(el: Div, by: f32) -> Div {
    el.absolute()
        .top(dp(by))
        .bottom(dp(by))
        .left(dp(by))
        .right(dp(by))
}

/// A tile's card over its area, as the navigator's cards are drawn.
pub fn tile_card(theme: &Theme) -> Div {
    inset_by(div(), TILE_MARGIN)
        .bg(theme.bg_1)
        .border_1()
        .border_color(theme.border_3)
        .rounded(dp(theme.shape.card))
}

/// Where a tile's content goes in its area: inside its card, and cut to it, as a header wider
/// than the tile is.
pub fn tile_content() -> Div {
    inset_by(div(), TILE_MARGIN + TILE_PAD).overflow_hidden()
}

/// A box over `area` of the room it's in, which is `relative`: where a tile goes.
pub fn placed(area: Area) -> Div {
    div()
        .absolute()
        .left(relative(area.left))
        .top(relative(area.top))
        .w(relative(area.width))
        .h(relative(area.height))
}

/// `text` with the letters a search found in it underlined, so a list shows why each row is
/// there. `found` are byte ranges of `text`, in order and apart.
pub fn matched(
    text: impl Into<SharedString>,
    found: impl IntoIterator<Item = Range<usize>>,
) -> StyledText {
    let underline = HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: fixed(1.),
            ..Default::default()
        }),
        ..Default::default()
    };
    StyledText::new(text).with_highlights(found.into_iter().map(|range| (range, underline)))
}

/// A place in text drawn as rows, as the diff's lines or a description's blocks: the row, and
/// a byte of its text.
pub type TextPoint = (usize, usize);

/// Where in `text`, laid out as `layout`, the pointer at `position` is: over a character, the
/// edge of it nearer the pointer, as text selection goes everywhere; off the text, where the
/// text stops, as its start above it and a line's end beside or below it.
pub fn text_index(layout: &TextLayout, text: &str, position: Point<Pixels>) -> usize {
    let ix = match layout.index_for_position(position) {
        Ok(ix) => ix,
        Err(ix) => return ix,
    };
    let next = (text.get(ix..))
        .and_then(|rest| rest.chars().next())
        .map(|c| ix + c.len_utf8());
    let edges = next.and_then(|next| {
        Some((
            layout.position_for_index(ix)?,
            layout.position_for_index(next)?,
            next,
        ))
    });
    match edges {
        Some((start, end, next)) if position.x > (start.x + end.x) / 2. => next,
        _ => ix,
    }
}

/// What a selection from `start` to `end`, in order, takes of `text`, row `row`'s. Nothing
/// when the text changed under it so its ends no longer fall in the text, as after an edit.
pub fn selected_part(
    text: &str,
    row: usize,
    (start, end): (TextPoint, TextPoint),
) -> Option<Range<usize>> {
    if row < start.0 || row > end.0 {
        return None;
    }
    let from = if row == start.0 { start.1 } else { 0 };
    let to = if row == end.0 { end.1 } else { text.len() };
    let fits = text.is_char_boundary(from) && text.is_char_boundary(to);
    (from < to && fits).then_some(from..to)
}

/// A tree's line down from an open row's chevron, `at` its middle, on each row under it,
/// where they join into one, as but.dev's sidebar draws them.
pub fn guide(at: f32, theme: &Theme) -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(dp(at))
        .w(fixed(1.))
        .bg(theme.border_3)
}

/// A fold's glyph in a step as wide as a branch's chevron: a chevron, down while `open` and
/// turning as that changes (see [`fold::chevron`]), or with nothing to fold (`None`) a dot at
/// half strength, as but-ui-react's.
pub fn fold_glyph(id: impl Into<ElementId>, open: Option<bool>, color: Hsla) -> Div {
    let step = div()
        .flex_none()
        .w(dp(14.))
        .h(dp(16.))
        .flex()
        .items_center()
        .justify_center();
    match open {
        None => step.child(small_icon(Icon::BulletDisc).text_color(color).opacity(0.5)),
        Some(open) => step.child(fold::chevron(id, open, color)),
    }
}

/// A row that unfolds a list, "N more", or folds it again, "Show fewer", as but-ui-react's
/// MoreItem: `glyph` at `icon_at` under the list's glyphs, the words at `text_at` under its
/// titles.
pub fn fold_row(
    id: impl Into<ElementId>,
    glyph: Icon,
    icon_at: f32,
    text_at: f32,
    theme: &Theme,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(dp(24.))
        .pl(dp(icon_at))
        .gap(dp(text_at - icon_at - 12.))
        .text_12()
        .text_color(theme.text_2)
        .hover(|el| el.text_color(theme.text_1))
        .child(small_icon(glyph).text_color(theme.text_3))
}

/// For a press on something whose click places the focus itself: left be, the press would
/// hand the focus to the pane around it until the click moved it on, and the selection
/// beside would blink.
pub fn keep_focus_on_press(_: &MouseDownEvent, window: &mut Window, _: &mut App) {
    window.prevent_default()
}

/// For a press on part of a row that acts on its own, without selecting or dragging the row:
/// stopped there, it never reaches `pane`, the focusable parent that would take the keys.
pub fn press_in_pane(
    pane: &FocusHandle,
) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let pane = pane.clone();
    move |_, window, cx| {
        window.focus(&pane, cx);
        cx.stop_propagation();
    }
}

/// A row's fold: [`fold_glyph`] that runs `toggle` on a click without selecting the row,
/// or just the dot when there's nothing to fold (`None`). A press gives `pane` the keys.
pub fn fold_button<V: 'static>(
    id: impl Into<ElementId>,
    open: Option<bool>,
    color: Hsla,
    pane: &FocusHandle,
    cx: &Context<V>,
    toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Stateful<Div> {
    let id = id.into();
    let theme = Theme::get(cx);
    let el = div().id(id.clone()).child(fold_glyph(id, open, color));
    match open {
        None => el,
        Some(_) => el
            .rounded(dp(theme.shape.xs))
            .hover(|el| el.bg(Theme::get(cx).hover))
            .on_mouse_down(MouseButton::Left, press_in_pane(pane))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                toggle(this, window, cx)
            })),
    }
}

/// A row of a side list: its selection solid while the list holds the focus, as the
/// navigator's and the file list's are.
pub fn side_row(
    id: impl Into<ElementId>,
    selected: bool,
    solid: bool,
    theme: &Theme,
) -> Stateful<Div> {
    list_row(id, selected, theme).when(selected && solid, |el| {
        el.bg(theme.fill_gray).text_color(theme.text_inverted)
    })
}

/// `el`, a row's secondary text, softened against a solid selection `on` it.
pub fn quiet(el: Div, on: bool, theme: &Theme) -> Div {
    el.flex_none()
        .when(on, |el| el.text_color(theme.muted_on_solid))
}

/// Where row `ix` of `list` was last drawn, while some of it shows: where ⇧F10 opens the menu
/// of the row selected.
pub fn shown_row(list: &gpui::ListState, ix: usize) -> Option<gpui::Bounds<Pixels>> {
    let (row, view) = (list.bounds_for_item(ix)?, list.viewport_bounds());
    (row.bottom() > view.top() && row.top() < view.bottom()).then_some(row)
}

/// A selected `row` whose neighbours are selected too, `above` and `below`: a run of them
/// reads as one block, as Finder draws one, rounded only where it starts and ends, with a
/// faint line between its rows. `solid` as `list_row`'s selection is while its list has
/// the focus.
pub fn joined(
    row: Stateful<Div>,
    (above, below): (bool, bool),
    solid: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let line = match solid {
        true => theme.inverse_soft,
        false => theme.bg_1,
    };
    row.when(above, |el| {
        el.relative().rounded_t_none().child(
            div()
                .absolute()
                .top_0()
                .left(dp(8.))
                .right(dp(8.))
                .h(fixed(1.))
                .bg(line),
        )
    })
    .when(below, |el| el.rounded_b_none())
}

/// The box around a text field, ringed while it has focus.
pub fn field(input: &Entity<text_input::TextInput>, focused: bool, theme: &Theme) -> Div {
    let clicked = input.clone();
    div()
        // Its padding is part of the field to click, and keeps the keys from a focusable
        // parent, which would take them as the press goes on to it.
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            window.focus(&gpui::Focusable::focus_handle(&clicked, cx), cx);
            window.prevent_default();
        })
        .px(dp(8.))
        .py(dp(5.))
        .rounded(dp(theme.shape.control))
        .border_1()
        .border_color(if focused {
            theme.focus_border
        } else {
            theme.border_2
        })
        .bg(theme.bg_1)
        .text_13()
        .child(input.clone())
}

/// A small round marker: new work, or something running.
pub fn dot(color: Hsla) -> Div {
    div().flex_none().size(dp(6.)).rounded_full().bg(color)
}

/// An unread row's mark: a hairline in `color` in the row's left padding, out of its flow so
/// nothing moves when it's read. The row must be `relative`.
pub fn unread_line(color: Hsla) -> Div {
    div()
        .absolute()
        .left(dp(2.))
        .top_0()
        .bottom_0()
        .flex()
        .items_center()
        .child(div().w(fixed(1.)).h(dp(14.)).bg(color))
}

/// How long ago `seconds` (since the epoch) was, faint, in a slot as wide as the longest
/// ("11mo"), so what sits before it lines up from row to row; empty without a time. It starts
/// at the slot's left, beside what comes before.
pub fn age_column(seconds: Option<i64>, theme: &Theme) -> Div {
    faint(seconds.map(relative_time).unwrap_or_default(), theme)
        .flex_none()
        .w(dp(30.))
        .whitespace_nowrap()
}

/// What a page shows while it's first read: nothing at first, then `title` fading in, so a
/// read done in a blink doesn't flash it.
pub fn loading_state(title: &'static str, theme: &Theme, cx: &App) -> AnyElement {
    use gpui::{Animation, AnimationExt as _};
    div()
        .size_full()
        .child(empty_state(None, title, "", theme, cx))
        .with_animation(
            "page-loading",
            Animation::new(std::time::Duration::from_millis(600)),
            |el, delta| el.opacity(((delta - 0.5) * 2.).clamp(0., 1.)),
        )
        .into_any_element()
}

/// `el` pulsing, fading in and out, while what it stands for is under way, where it is, rather
/// than a word beside it that would move what's around it. The status bar names what's under
/// way.
pub fn pulsing<E: IntoElement + Styled + 'static>(el: E) -> impl IntoElement {
    use gpui::{Animation, AnimationExt as _};
    el.with_animation(
        "pulsing",
        Animation::new(std::time::Duration::from_millis(1600))
            .repeat()
            .with_easing(gpui::pulsating_between(0.35, 1.)),
        |el, delta| el.opacity(delta),
    )
}

const SPIN_STEP: std::time::Duration = std::time::Duration::from_millis(100);
/// The dots each step of [`spinning`] shows, as braille's: the low three bits are the left
/// column's, top down, the next three the right's. The terminal's dots spinner, ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏.
const SPIN_FRAMES: [u8; 10] = [0x0b, 0x19, 0x39, 0x38, 0x3c, 0x34, 0x26, 0x27, 0x07, 0x0f];
/// A dot of [`spinning`], and how far apart their centres are, in a `small_icon`'s box.
const SPIN_DOT: f32 = 2.;
const SPIN_PITCH: f32 = 3.5;

/// Dots running round, as a terminal's spinner does, while what it stands for is under way,
/// `turning` only while it shows. It steps 10 times a second, each step drawing the view it's
/// in, and with it the whole window, again: an agent's spinner turns for minutes, and drawing
/// the window any more often kept it from going idle and WindowServer busy all that while.
/// Dots read as stepping on purpose at that rate, where a turning glyph would look choppy.
pub fn spinning(color: Hsla, turning: bool) -> impl IntoElement {
    use gpui::canvas;
    static START: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(crate::clock::now);
    let since = crate::clock::now().saturating_duration_since(*START);
    let steps = since.as_nanos() / SPIN_STEP.as_nanos();
    #[expect(clippy::indexing_slicing, reason = "taken modulo the length")]
    let frame = SPIN_FRAMES[(steps % SPIN_FRAMES.len() as u128) as usize];
    let due = SPIN_STEP
        - std::time::Duration::from_nanos((since.as_nanos() % SPIN_STEP.as_nanos()) as u64);
    let left = (12. - SPIN_PITCH - SPIN_DOT) / 2.;
    let top = (12. - 2. * SPIN_PITCH - SPIN_DOT) / 2.;
    let dots = (0..6)
        .filter(|bit| frame & (1 << bit) != 0)
        .map(move |bit| {
            let (column, row) = ((bit / 3) as f32, (bit % 3) as f32);
            div()
                .absolute()
                .left(dp(left + column * SPIN_PITCH))
                .top(dp(top + row * SPIN_PITCH))
                .size(dp(SPIN_DOT))
                .rounded_full()
                .bg(color)
        });
    div()
        .relative()
        .flex_none()
        .size(dp(12.))
        .children(dots)
        // The next step asked for as this one is drawn, so a spinner that's gone, or one in a
        // view that isn't drawn, asks for no more.
        .when(turning, |el| {
            el.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| {
                        let view = window.current_view();
                        (cx.spawn(async move |cx| {
                            cx.background_executor().timer(due).await;
                            cx.update(|cx| cx.notify(view));
                        }))
                        .detach();
                    },
                )
                .absolute(),
            )
        })
}

/// A faint pop wash over a `list_row`'s ground that fades out once, as for a commit that just
/// showed up (`Store::glows`), so an agent's commit is noticed where it lands rather than in a
/// card. Make it the row's first child, of a `relative` row, so the row's text draws over it.
/// A child rather than a wrapper, so the row keeps its identity, and with it a press about to
/// become a drag, when the wash goes. Keep it off a selected row, whose ground marks it.
pub fn glow_once(id: impl Into<ElementId>, theme: &Theme) -> impl IntoElement {
    use gpui::{Animation, AnimationExt as _};
    let wash = theme.pop;
    div()
        .absolute()
        .inset_0()
        .rounded(dp(theme.shape.control))
        .with_animation(
            id,
            Animation::new(crate::state::GLOW).with_easing(gpui::ease_in_out),
            move |el, delta| el.bg(wash).opacity(0.08 * (1. - delta)),
        )
}

pub const BLINK: std::time::Duration = std::time::Duration::from_millis(300);

/// A `wash` over what y copied that holds a moment and goes, as the TUI highlights it, or over
/// where a mode would put what it takes, as a click it can't use reminds the user of it:
/// `shape`, an absolute div over the row or box, shaped as its ground is. Its first child, as
/// [`glow_once`] is, where it has a ground of its own for the text to draw over. Give each
/// blink an `id` of its own, so one right after another starts again.
pub fn blink_once(id: impl Into<ElementId>, shape: Div, wash: Hsla) -> impl IntoElement {
    use gpui::{Animation, AnimationExt as _};
    shape.with_animation(
        id,
        Animation::new(BLINK).with_easing(gpui::quadratic),
        move |el, delta| el.bg(wash).opacity(0.5 * (1. - delta)),
    )
}

/// A strip along a pane's right edge: drag it to resize the pane, double-click to put it
/// back. The pane needs `.relative()`, and an ancestor that starts where the pane does
/// listens with [`resize_pane`].
pub fn resize_handle(pane: Pane, store: Entity<Store>, theme: &Theme) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("resize-{pane:?}").into()))
        .absolute()
        .top_0()
        .bottom_0()
        .right(dp(-2.))
        .w(dp(5.))
        .group("resize-handle")
        .cursor(CursorStyle::ResizeLeftRight)
        // Over the pane's scrollbar, which would otherwise take the press and the drag.
        .block_mouse_except_scroll()
        // A line along the pane's last column of pixels, as but.dev marks it.
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(dp(2.))
                .w(fixed(1.))
                .group_hover("resize-handle", |el| el.bg(theme.border_1)),
        )
        .on_drag(pane, |_, _, _, cx| cx.new(|_| EmptyView))
        .on_click(move |event, _, cx| {
            if event.click_count() == 2 {
                store.update(cx, |store, cx| store.set_width(pane, None, cx));
            }
        })
}

/// A strip over the gap between two cards, `axis` saying which way they're beside each other:
/// drag it to move what's between them. A pill in its middle shows it while the pointer's on
/// it or, as the store keeps, it's being dragged, `which` it is. Its drag goes on with the
/// payload the caller gives it and [`mark_dragged`].
pub fn gap_divider(
    id: impl Into<ElementId>,
    axis: crate::state::Axis,
    which: crate::state::DividerDrag,
    store: &Entity<Store>,
    theme: &Theme,
    cx: &App,
) -> Stateful<Div> {
    use crate::state::Axis;
    let dragged = store.read(cx).dragged_divider.as_ref() == Some(&which);
    let (cursor, pill) = match axis {
        Axis::Across => (CursorStyle::ResizeLeftRight, div().w(dp(4.)).h(dp(32.))),
        Axis::Down => (CursorStyle::ResizeUpDown, div().w(dp(32.)).h(dp(4.))),
    };
    let pill = pill
        .rounded_full()
        .when(dragged, |el| el.bg(theme.border_1))
        .group_hover("gap-divider", |el| el.bg(theme.border_1));
    let let_go = {
        let store = store.clone();
        move |_: &gpui::MouseUpEvent, _: &mut Window, cx: &mut App| {
            store.update(cx, |store, cx| {
                if store.dragged_divider.take().is_some() {
                    cx.notify();
                }
            })
        }
    };
    div()
        .id(id)
        .absolute()
        .flex()
        .when(axis == Axis::Down, |el| el.flex_col())
        .items_center()
        .justify_center()
        .group("gap-divider")
        .cursor(cursor)
        // Over the cards' own pointer handling, as a terminal's selecting.
        .block_mouse_except_scroll()
        .child(pill)
        .when(dragged, |el| {
            el.on_mouse_up(MouseButton::Left, let_go.clone())
                .on_mouse_up_out(MouseButton::Left, let_go)
        })
}

/// A handle over where [`gap_divider`]s meet, 16 across, shown by their pills along each line
/// of `junction` while the pointer's on it or it's dragged, as `which`. The caller places it.
/// What lets go of it is the room's, as a junction can join another as it's dragged, and so
/// stop being drawn.
pub fn gap_junction(
    id: impl Into<ElementId>,
    junction: &crate::state::Junction,
    which: &crate::state::DividerDrag,
    store: &Entity<Store>,
    theme: &Theme,
    cx: &App,
) -> Stateful<Div> {
    use crate::state::Side;
    let dragged = store.read(cx).dragged_divider.as_ref() == Some(which);
    // From the handle's middle, as long as half a divider's pill.
    let pill = |side: Side| {
        let pill = div().absolute().rounded_full();
        let pill = match side {
            Side::Top => pill.left(dp(6.)).top(dp(-8.)).w(dp(4.)).h(dp(18.)),
            Side::Bottom => pill.left(dp(6.)).top(dp(6.)).w(dp(4.)).h(dp(18.)),
            Side::Left => pill.top(dp(6.)).left(dp(-8.)).h(dp(4.)).w(dp(18.)),
            Side::Right => pill.top(dp(6.)).left(dp(6.)).h(dp(4.)).w(dp(18.)),
        };
        pill.when(dragged, |el| el.bg(theme.border_1))
            .group_hover("gap-junction", |el| el.bg(theme.border_1))
    };
    let pills = [Side::Top, Side::Right, Side::Bottom, Side::Left]
        .into_iter()
        .filter(|side| junction.reaches(*side))
        .map(pill);
    div()
        .id(id)
        .absolute()
        .group("gap-junction")
        .cursor(CursorStyle::ResizeUpLeftDownRight)
        // Over the cards' own pointer handling, as a terminal's selecting.
        .block_mouse_except_scroll()
        .children(pills)
}

/// What starts a [`gap_divider`]'s drag: the store keeps that `which` is dragged, as its pill
/// shows, and GPUI draws nothing under the pointer for it.
pub fn mark_dragged<T>(
    store: &Entity<Store>,
    which: crate::state::DividerDrag,
) -> impl Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<EmptyView> + 'static {
    let store = store.clone();
    move |_, _, _, cx| {
        store.update(cx, |store, cx| {
            store.dragged_divider = Some(which.clone());
            cx.notify();
        });
        cx.new(|_| EmptyView)
    }
}

/// Resize `pane` while its handle is dragged, measuring from the left of the element this
/// listens on, the handle's middle `beyond` short of the pane's edge.
pub fn resize_pane(
    pane: Pane,
    beyond: f32,
    store: Entity<Store>,
) -> impl Fn(&DragMoveEvent<Pane>, &mut Window, &mut App) + 'static {
    move |event, _, cx| {
        if *event.drag(cx) == pane {
            let width = f32::from(event.event.position.x - event.bounds.left());
            let width = width / Theme::get(cx).scale + beyond;
            store.update(cx, |store, cx| store.set_width(pane, Some(width), cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tab_after;

    #[test]
    fn no_icon_is_resized_after_its_path_is_picked() {
        // `icon`, `small_icon` and `icon_at` bake the drawn size into the path, to keep the
        // strokes as wide (see `keep_strokes`). A `.size()` after one of them draws that path
        // at another size, which is the thin-stroke bug again, so the size goes to `icon_at`.
        let mut resized = Vec::new();
        // This crate's call sites and but-ui-gpui's, where the helpers are.
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut files = vec![crate_dir.join("src"), crate_dir.join("../but-ui-gpui/src")];
        while let Some(dir) = files.pop() {
            for entry in std::fs::read_dir(&dir).expect("the crate's sources are readable") {
                let path = entry.expect("the entry is readable").path();
                if path.is_dir() {
                    files.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path).expect("the source is UTF-8");
                // The call, then anything chained that isn't itself a `.size(`, then `.size(`.
                for (ix, _) in source.match_indices(".size(") {
                    let before = source.get(..ix).unwrap_or_default();
                    let call = before.rsplit_once([';', '{']);
                    let call = call.map_or(before, |(_, after)| after);
                    let drawn = ["icon(", "small_icon(", "icon_at(", "snackbar_icon("]
                        .iter()
                        .any(|helper| call.contains(helper));
                    if drawn && !call.contains("svg()") {
                        let line = before.matches('\n').count() + 1;
                        resized.push(format!("{}:{line}", path.display()));
                    }
                }
            }
        }
        assert!(
            resized.is_empty(),
            "these draw an icon and then resize it, so its strokes are made for another size; \
             pass the size to `icon_at` instead: {resized:?}"
        );
    }

    #[test]
    fn the_next_tab_is_the_one_after() {
        assert_eq!(tab_after(3, 0, true), Some(1));
    }

    #[test]
    fn the_next_tab_goes_round_to_the_first() {
        assert_eq!(tab_after(3, 2, true), Some(0));
    }

    #[test]
    fn the_previous_tab_is_the_one_before() {
        assert_eq!(tab_after(3, 2, false), Some(1));
    }

    #[test]
    fn the_previous_tab_goes_round_to_the_last() {
        assert_eq!(tab_after(3, 0, false), Some(2));
    }

    #[test]
    fn a_page_with_one_tab_has_no_other() {
        assert_eq!(tab_after(1, 0, true), None);
    }
}
