//! Shared building blocks so every surface draws rows, buttons and counts the same way.

#[cfg(target_os = "linux")]
pub mod alert;
pub mod conflict;
pub mod diff;
pub mod emoji;
pub mod file_icons;
mod format;
pub mod illustration;
pub mod markdown;
pub mod modal;
pub mod person;
pub mod review;
pub mod scrollbar;
pub mod text_input;
pub mod window_controls;
pub mod window_frame;

use std::borrow::Cow;
use std::ops::Range;

use gpui::{
    Action, AnyElement, App, AssetSource, ClickEvent, Context, CursorStyle, Div, DragMoveEvent,
    ElementId, EmptyView, Entity, HighlightStyle, Hsla, MouseButton, MouseDownEvent, Pixels, Point,
    Rems, Role, SharedString, Stateful, StyledText, Svg, Toggled, UnderlineStyle, Window, div,
    prelude::*, rems, svg,
};

use crate::state::{Pane, Store};
use crate::theme::Theme;
pub use format::*;
use illustration::{Illustration, illustration};

/// The rem at 100%, so a [`dp`] is a pixel then.
pub const BASE_REM: f32 = 16.;

/// `n` pixels as designed, at 100%, which the interface scale grows or shrinks: sizes from
/// ui-react's pixels go in as they are. Only what must stay put, as a hairline or the room
/// for macOS's traffic lights, is in plain pixels.
pub fn dp(n: f32) -> Rems {
    rems(n / BASE_REM)
}

/// `n` pixels at every interface scale, for what must stay put: a hairline, the room for
/// macOS's traffic lights, the scrollbar.
#[expect(clippy::disallowed_methods)]
pub const fn fixed(n: f32) -> Pixels {
    gpui::px(n)
}

/// `el`, a header the window is dragged by, as a title bar is: the app draws its own over
/// macOS's and has none elsewhere. Windows drags it itself, and on macOS and Linux the app starts
/// the move once the pointer moves with the button held, so a click there is still a click. A
/// double click does what the platform's title bars do: on macOS what System Settings says (zoom,
/// minimize or nothing), elsewhere maximize or restore. On Linux a right click opens the window's
/// menu, as GNOME's title bars do.
pub fn drags_window(el: Div) -> Div {
    let el = el.window_control_area(gpui::WindowControlArea::Drag);
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

/// `el`, in a header the window is dragged by, still taking its own clicks: the drag area under
/// it would take them, and a double click there would zoom the window.
pub fn above_drag<E: InteractiveElement>(el: E) -> E {
    el.occlude()
}

/// Whether the app draws the window's minimize, maximize and close buttons: on Windows, and on
/// Linux where the desktop leaves the window's frame to the app, as GNOME does on Wayland.
pub fn draws_window_controls(window: &Window) -> bool {
    cfg!(target_os = "windows")
        || matches!(
            window.window_decorations(),
            gpui::Decorations::Client { .. }
        )
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
pub const ROW_HEIGHT: f32 = 28.;
pub const HEADER_HEIGHT: f32 = 44.;
pub const STATUS_BAR_HEIGHT: f32 = 28.;

/// Every icon: the design system's, compiled in from `packages/ui-react` so they're the ones
/// Lite and but.dev draw, and a build fails if one goes; then this app's own, under
/// `assets/icons`, for what ui-react draws in code rather than from a file, or doesn't have.
macro_rules! icons {
    (
        $($variant:ident => $name:literal),* $(,)?;
        own: $($own:ident => $own_name:literal),* $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon {
            $($variant,)*
            $($own,)*
        }

        impl Icon {
            pub fn path(self) -> &'static str {
                match self {
                    $(Icon::$variant => concat!("icons/", $name, ".svg"),)*
                    $(Icon::$own => concat!("icons/", $own_name, ".svg"),)*
                }
            }
        }

        fn icon_bytes(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!(
                    "../../../../packages/ui-react/src/icons/", $name, ".svg"
                ))),)*
                $(concat!("icons/", $own_name, ".svg") => Some(include_bytes!(concat!(
                    "../../assets/icons/", $own_name, ".svg"
                ))),)*
                _ => None,
            }
        }
    };
}

icons! {
    Absorb => "absorb",
    Ai => "ai",
    ArrowDoubleDown => "arrow-double-down",
    ArrowDown => "arrow-down",
    ArrowInBox => "arrow-in-box",
    ArrowLineDown => "arrow-line-down",
    ArrowUp => "arrow-up",
    ArrowUpRight => "arrow-up-right",
    Bin => "bin",
    Branch => "branch",
    BranchMerge => "branch-merge",
    BulletDisc => "bullet-disc",
    Camera => "camera",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronUp => "chevron-up",
    ChevronRight => "chevron-right",
    Clock => "clock",
    Contrast => "contrast",
    Copy => "copy",
    Commit => "commit",
    CommitFill => "commit-fill",
    Diff => "diff",
    Discord => "discord",
    Docs => "docs",
    DragVertical => "drag-vertical",
    Edit => "edit",
    Eye => "eye",
    CollapseVertical => "collapse-vertical",
    Cross => "cross",
    ExpandVertical => "expand-vertical",
    File => "file",
    Filter => "filter",
    FileDiff => "file-diff",
    FilesSidebar => "files-sidebar",
    Folder => "folder",
    FolderCopy => "folder-copy",
    Globe => "globe",
    Hash => "hash",
    Kebab => "kebab",
    Lab => "lab",
    Link => "link",
    Lock => "lock",
    Mixer => "mixer",
    Plug => "plug",
    Plus => "plus",
    Pr => "pr",
    PrClose => "pr-close",
    PrDraft => "pr-draft",
    Refresh => "refresh",
    Repo => "repo",
    Search => "search",
    Settings => "settings",
    Sidebar => "sidebar",
    SidebarNarrow => "sidebar-narrow",
    Stack => "stack",
    Tag => "tag",
    TextBlock => "text-block",
    TextCode => "text-code",
    TextWrap => "text-wrap",
    Tick => "tick",
    Undo => "undo",
    Warning => "warning";
    // ui-react's FolderIcon, a component with its gradient in CSS.
    own: FolderFill => "folder-fill",
    // ui-svelte's keyboard, which ui-react doesn't have.
    Keyboard => "keyboard",
    // ui-react's eye, struck through, for a project not watched.
    EyeOff => "eye-off",
    // ui-react's folders with their top right corner cut away, for a dot there while a
    // worktree has uncommitted changes.
    FolderDot => "folder-dot",
    FolderCopyDot => "folder-copy-dot",
    // Another machine, which ui-react has no glyph for, in the style of its folder.
    Computer => "computer",
    // The repo glyph filled as ui-react's `EntityGlyph` fills it, its gradient baked in, for a
    // project's icon: one for each colour but the outline, which is `Repo`.
    RepoPop => "repo-pop",
    RepoSafe => "repo-safe",
    RepoPurple => "repo-purple",
    RepoDanger => "repo-danger",
    RepoWarn => "repo-warn",
    RepoGray => "repo-gray",
}

/// GitButler's fonts, compiled into the binary so the client looks the same everywhere.
/// Embedded fonts are as fast as system ones; a user-installed variable font isn't, as
/// CoreText asks the font daemon about it for every line it lays out.
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    macro_rules! fonts {
        ($($name:literal),* $(,)?) => {
            vec![$(Cow::Borrowed(include_bytes!(concat!("../../assets/fonts/", $name, ".ttf")).as_slice()),)*]
        };
    }
    fonts!(
        "Inter-Regular",
        "Inter-Medium",
        "Inter-SemiBold",
        "GeistMono-Regular",
        "GeistMono-SemiBold",
    )
}

/// Match each of `fonts`' faces against the installed fonts in the background, as Zed does.
/// cosmic-text otherwise does it on the main thread the first time it lays out text in a face,
/// and it walks every installed font to do so.
#[cfg(target_os = "linux")]
pub fn prewarm_fonts(cx: &gpui::App) {
    use gpui::FontWeight;

    let fonts = [
        ("Inter", FontWeight::NORMAL),
        ("Inter", FontWeight::MEDIUM),
        ("Inter", FontWeight::SEMIBOLD),
        ("Geist Mono", FontWeight::NORMAL),
        ("Geist Mono", FontWeight::SEMIBOLD),
    ]
    .map(|(family, weight)| gpui::Font {
        weight,
        ..gpui::font(family)
    });
    let text_system = cx.text_system().clone();
    cx.background_spawn(async move { text_system.prewarm_fonts(&fonts) })
        .detach();
}

/// Serves the icons compiled into the binary.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok((icon_bytes(path).map(Cow::Borrowed)).or_else(|| person::load(path).map(Cow::Owned)))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

pub fn icon(icon: Icon) -> Svg {
    svg().path(icon.path()).size(dp(16.)).flex_none()
}

pub fn small_icon(icon: Icon) -> Svg {
    svg().path(icon.path()).size(dp(12.)).flex_none()
}

/// Icon toggles joined in a row, as ui-react's `ToggleGroupStyles` of outline toggles: one
/// border round them and a rule between.
pub fn toggle_group(theme: &Theme, toggles: impl IntoIterator<Item = Stateful<Div>>) -> Div {
    let line = theme.control_border();
    let toggles: Vec<_> = toggles.into_iter().collect();
    let last = toggles.len().saturating_sub(1);
    h_flex()
        .flex_none()
        .h(dp(28.))
        .rounded(dp(4.))
        .border_1()
        .border_color(line)
        .children(toggles.into_iter().enumerate().map(|(ix, toggle)| {
            // Rounded inside the border, so a pressed one's ground keeps to it.
            toggle
                .when(ix == 0, |el| el.rounded_l(dp(3.)))
                .when(ix == last, |el| el.rounded_r(dp(3.)))
                .when(ix > 0, |el| el.border_l_1().border_color(line))
        }))
}

/// One of a [`toggle_group`]'s toggles, on the recessed ground while `pressed`.
pub fn icon_toggle(
    id: impl Into<ElementId>,
    glyph: Icon,
    tip: &'static str,
    key: Option<SharedString>,
    pressed: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.hover();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(tip)
        .w(dp(28.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .when(pressed, |el| el.bg(theme.bg_2))
        .when(!pressed, |el| el.hover(move |el| el.bg(hover)))
        .child(icon(glyph).text_color(theme.text_1.opacity(0.7)))
        .tooltip(Tooltip::keyed(tip, key))
}

/// A choice of one, as ui-react's `SegmentedControl`: `segments` with whether each is the
/// picked one, on a recessed track. The picked one is raised on the content ground, and a short
/// rule parts two that aren't.
pub fn segmented(
    small: bool,
    theme: &Theme,
    segments: impl IntoIterator<Item = (Stateful<Div>, bool)>,
) -> Div {
    let (text, hover, line) = (theme.text_1, theme.hover(), theme.control_border());
    let segments: Vec<_> = segments.into_iter().collect();
    let picked: Vec<bool> = segments.iter().map(|(_, picked)| *picked).collect();
    h_flex()
        .flex_none()
        .h(dp(if small { 24. } else { 28. }))
        .rounded(dp(4.))
        .bg(theme.bg_2)
        .text_size(dp(if small { 12. } else { 13. }))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .children(
            segments
                .into_iter()
                .enumerate()
                .flat_map(|(ix, (segment, on))| {
                    // Always there, so picking another moves nothing.
                    let before = ix.checked_sub(1).and_then(|before| picked.get(before));
                    let rule = before.map(|before| {
                        let drawn = !before && !on;
                        (div().flex_none().w(fixed(1.)).h(dp(12.)))
                            .when(drawn, |el| el.bg(theme.border_2))
                            .into_any_element()
                    });
                    let segment = tab_stop(segment, theme).role(Role::Button);
                    let segment = (segment.h_full().flex().items_center())
                        .px(dp(if small { 7. } else { 9. }))
                        .rounded(dp(4.))
                        .border_1()
                        .border_color(if on { line } else { gpui::transparent_black() })
                        .text_color(text.opacity(if on { 0.9 } else { 0.7 }))
                        .when(on, |el| el.bg(theme.bg_1))
                        .when(!on, |el| {
                            el.hover(move |el| el.bg(hover).text_color(text.opacity(0.9)))
                        });
                    rule.into_iter().chain([segment.into_any_element()])
                }),
        )
}

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

pub fn v_flex() -> Div {
    div().flex().flex_col()
}

/// Secondary text, like a setting's description.
pub fn muted(text: impl IntoElement, theme: &Theme) -> Div {
    div()
        .text_size(dp(12.))
        .text_color(theme.text_2)
        .child(text)
}

/// Faint small text, for details that can be skipped.
pub fn faint(text: impl IntoElement, theme: &Theme) -> Div {
    div()
        .text_size(dp(11.))
        .text_color(theme.text_3)
        .child(text)
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

/// The heading over a group of rows.
pub fn section_label(text: impl IntoElement, theme: &Theme) -> Div {
    faint(text, theme).font_weight(gpui::FontWeight::SEMIBOLD)
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
        .bg(theme.text_2.opacity(0.2))
}

/// A fold's glyph in a step as wide as a branch's chevron: a chevron, down while `open`, or
/// with nothing to fold (`None`) a dot at half strength, as ui-react's.
pub fn fold_glyph(open: Option<bool>, color: Hsla) -> Div {
    let glyph = match open {
        None => Icon::BulletDisc,
        Some(true) => Icon::ChevronDown,
        Some(false) => Icon::ChevronRight,
    };
    div()
        .flex_none()
        .w(dp(14.))
        .h(dp(16.))
        .flex()
        .items_center()
        .justify_center()
        .child(small_icon(glyph).text_color(match open {
            None => color.opacity(0.5),
            Some(_) => color,
        }))
}

/// A row that unfolds a list, "N more", or folds it again, "Show fewer", as ui-react's
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
        .text_size(dp(12.))
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

/// A row's fold: [`fold_glyph`] that runs `toggle` on a click without selecting the row,
/// or just the dot when there's nothing to fold (`None`).
pub fn fold_button<V: 'static>(
    id: impl Into<ElementId>,
    open: Option<bool>,
    color: Hsla,
    cx: &Context<V>,
    toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Stateful<Div> {
    let el = div().id(id).child(fold_glyph(open, color));
    match open {
        None => el,
        Some(_) => el
            .rounded(dp(3.))
            .hover(|el| el.bg(Theme::get(cx).hover()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
        .when(on, |el| el.text_color(theme.muted_on_solid()))
}

/// A row of a list or menu, shaded when `selected` and otherwise under the pointer.
pub fn list_row(id: impl Into<ElementId>, selected: bool, theme: &Theme) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::ListItem)
        .aria_selected(selected)
        .flex_none()
        .h(dp(ROW_HEIGHT))
        .mx(dp(6.))
        .px(dp(8.))
        .gap(dp(8.))
        .rounded(dp(4.))
        .when(selected, |el| el.bg(theme.selected_unfocused()))
        .when(!selected, |el| el.hover(|el| el.bg(theme.hover())))
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
        true => theme.text_inverted.opacity(0.1),
        false => theme.bg_1.opacity(0.4),
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

/// `el` in the window's tab order, ringed while Tab gave it the keys, as a dialog's controls
/// are on macOS. A click leaves the keys where they were, as clicking a button does there,
/// so a list keeps its arrows and Return doesn't press it again.
pub fn tab_stop(el: Stateful<Div>, theme: &Theme) -> Stateful<Div> {
    // Inside its edge: a shadow spread outside it fills a control without a ground of its own.
    let ring = gpui::BoxShadow::new(fixed(0.), fixed(0.), theme.focus_border())
        .spread_radius(fixed(2.))
        .inset();
    shown_when_focused(el.tab_index(0))
        .focus_visible(move |el| el.shadow(vec![ring]))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
}

gpui::actions!(
    ui,
    [
        /// Scroll what Tab just focused into view, sent where the keys are.
        ShowFocused
    ]
);

/// Where the control Tab just focused was drawn, from it to the [`scrollbar::scrolled`] list
/// it's in, as [`ShowFocused`] goes up from one to the other.
pub struct FocusedAt(pub gpui::Bounds<Pixels>);

impl gpui::Global for FocusedAt {}

/// `el`, which tells the scrolled list around it where it was drawn when Tab focuses it, so
/// the list can scroll it into view.
pub fn shown_when_focused(el: Stateful<Div>) -> Stateful<Div> {
    let drawn = std::rc::Rc::new(std::cell::Cell::new(None));
    let record = drawn.clone();
    el.on_action(move |_: &ShowFocused, _, cx| {
        if let Some(bounds) = drawn.get() {
            cx.set_global(FocusedAt(bounds));
        }
        cx.propagate();
    })
    .child(
        gpui::canvas(
            move |bounds, _, _| record.set(Some(bounds)),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

/// How a switch's knob slides and its ground fades when it's turned: critically damped, most of
/// the way in about 0.1s, as ui-react's Switch moves in `--transition-medium`. A spring, unlike
/// `with_animation`, starts where it's going when first drawn, so opening a page moves nothing,
/// and a second click mid-slide turns it back from where it is.
const SWITCH_SPRING: gpui::SpringConfig = gpui::SpringConfig::new(1600., 80., 1.);

/// A switch's spring toward `on`, settled once the knob is a tenth of a pixel from its end.
pub fn switch_spring(on: bool) -> gpui::SpringAnimation<bool> {
    gpui::SpringAnimation::new(SWITCH_SPRING)
        .to(on)
        .with_epsilon(0.01)
}

/// A switch's ground off and on, to mix in sRGB as CSS does: mixing `Hsla` would sweep the
/// gray's hue round to pop's and tint the middle.
pub fn switch_grounds(theme: &Theme) -> [gpui::Rgba; 2] {
    [theme.fill_gray.opacity(0.22).into(), theme.pop_bg.into()]
}

/// A [`switch`], which takes clicks as the `Stateful<Div>` it wraps does.
pub struct Switch {
    id: ElementId,
    on: bool,
    el: Stateful<Div>,
    grounds: [gpui::Rgba; 2],
}

/// An on/off switch, for a setting that applies at once.
pub fn switch(id: impl Into<ElementId>, on: bool, theme: &Theme) -> Switch {
    let id = id.into();
    let el = tab_stop(h_flex().id(id.clone()), theme)
        .role(Role::Switch)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .flex_none()
        .w(dp(30.))
        .h(dp(18.))
        .p(dp(2.))
        .rounded_full()
        .hover(|el| el.opacity(0.9));
    Switch {
        id,
        on,
        el,
        grounds: switch_grounds(theme),
    }
}

impl Switch {
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.el = self.el.on_click(handler);
        self
    }
}

impl IntoElement for Switch {
    type Element = gpui::SpringAnimationElement<Stateful<Div>>;

    fn into_element(self) -> Self::Element {
        use gpui::AnimationExt as _;
        let [off, on] = self.grounds;
        // The spring wraps the whole switch, not a child: the track is the tab stop, whose
        // focus ring is drawn over its ground and under its children.
        self.el
            .with_spring(self.id, switch_spring(self.on), move |el, phase| {
                el.bg(phase.interpolate_clamped(off, on)).child(
                    div()
                        // The track's width, less its padding and the knob.
                        .ml(phase.interpolate_clamped(dp(0.), dp(12.)))
                        .size(dp(14.))
                        .rounded_full()
                        .bg(gpui::white()),
                )
            })
    }
}

/// A checkbox, ticked, empty or, for a group partly checked, a dash.
pub fn checkbox(id: impl Into<ElementId>, check: Toggled, theme: &Theme) -> Stateful<Div> {
    let on = check != Toggled::False;
    div()
        .id(id)
        .role(Role::CheckBox)
        .aria_toggled(check)
        .flex_none()
        .size(dp(14.))
        .rounded(dp(3.))
        .border_1()
        .border_color(if on { theme.pop_bg } else { theme.border_2 })
        .when(on, |el| el.bg(theme.pop_bg))
        .flex()
        .items_center()
        .justify_center()
        .map(|el| match check {
            Toggled::True => el.child(
                small_icon(Icon::Tick)
                    .size(dp(10.))
                    .text_color(gpui::white()),
            ),
            Toggled::Mixed => el.child(div().w(dp(8.)).h(dp(2.)).rounded(dp(1.)).bg(gpui::white())),
            Toggled::False => el,
        })
}

/// A button showing a choice, which opens a menu of the others when clicked.
pub fn dropdown(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    tab_stop(h_flex().id(id), theme)
        .role(Role::ComboBox)
        .flex_none()
        .h(dp(28.))
        .pl(dp(10.))
        .pr(dp(6.))
        .gap(dp(6.))
        .rounded(dp(4.))
        .border_1()
        .border_color(theme.border_2)
        .text_size(dp(13.))
        .hover(|el| el.bg(theme.hover()))
        .child(div().min_w_0().truncate().child(label.into()))
        .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
}

/// The box around a text field, ringed while it has focus.
pub fn field(input: &Entity<text_input::TextInput>, focused: bool, theme: &Theme) -> Div {
    let clicked = input.clone();
    div()
        // Its padding is part of the field to click.
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            window.focus(&gpui::Focusable::focus_handle(&clicked, cx), cx)
        })
        .px(dp(8.))
        .py(dp(5.))
        .rounded(dp(4.))
        .border_1()
        .border_color(if focused {
            theme.focus_border()
        } else {
            theme.border_2
        })
        .bg(theme.bg_1)
        .text_size(dp(13.))
        .child(input.clone())
}

/// How round a popover's corners are.
pub const POPOVER_RADIUS: f32 = 8.;

/// The card that menus, dialogs, toasts and tooltips float on.
pub fn popover(theme: &Theme) -> Div {
    v_flex()
        .rounded(dp(POPOVER_RADIUS))
        .bg(theme.bg_1)
        .border_1()
        .border_color(theme.border_2)
        .shadow_lg()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// No ground; for actions inside a row, header or card.
    Ghost,
    /// The quiet default on open ground.
    Outline,
    /// Lifts one button above its neighbours without spending colour.
    Gray,
    /// The one primary action of a surface.
    Pop,
    /// For an act the user can't take back: deleting, discarding.
    Danger,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    style: ButtonStyle,
    /// Why it can't be pressed now, shown in place of its tooltip.
    disabled: Option<SharedString>,
    small: bool,
    /// On an inverted ground, like a selected row.
    inverted: bool,
    /// The colour of the ground it sits on, as a banner's action takes its banner's.
    tint: Option<Hsla>,
    on_click: Option<ClickHandler>,
    tooltip: Option<SharedString>,
    tooltip_key: Option<SharedString>,
    /// The action whose keys the tooltip shows when it names none.
    tooltip_action: Option<Box<dyn Action>>,
    /// Which half of a [`dropdown_button`] it is, squared where the halves meet.
    half: Option<Half>,
}

#[derive(Clone, Copy)]
enum Half {
    Action,
    Menu,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button {
        id: id.into(),
        label: Some(label.into()),
        icon: None,
        style: ButtonStyle::Outline,
        disabled: None,
        small: false,
        inverted: false,
        tint: None,
        on_click: None,
        tooltip: None,
        tooltip_key: None,
        tooltip_action: None,
        half: None,
    }
}

pub fn icon_button(id: impl Into<ElementId>, icon: Icon) -> Button {
    Button {
        label: None,
        icon: Some(icon),
        style: ButtonStyle::Ghost,
        ..button(id, "")
    }
}

impl Button {
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Filled in while it's the chosen one of a few.
    pub fn selected(self, selected: bool) -> Self {
        self.style(if selected {
            ButtonStyle::Gray
        } else {
            ButtonStyle::Ghost
        })
    }

    /// Unpressable for the reason given, if any, which its tooltip says instead. `None`
    /// keeps an earlier reason.
    pub fn disabled_because(mut self, why: Option<impl Into<SharedString>>) -> Self {
        self.disabled = why.map(Into::into).or(self.disabled);
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn inverted(mut self, inverted: bool) -> Self {
        self.inverted = inverted;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn tooltip_with_key(
        mut self,
        tooltip: impl Into<SharedString>,
        key: Option<SharedString>,
    ) -> Self {
        self.tooltip = Some(tooltip.into());
        self.tooltip_key = key;
        self
    }

    /// A tooltip of `tooltip` with the keys bound to `action`, looked up when it shows.
    pub fn tooltip_for(mut self, tooltip: impl Into<SharedString>, action: &dyn Action) -> Self {
        self.tooltip = Some(tooltip.into());
        self.tooltip_action = Some(action.boxed_clone());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Dispatch `action` when clicked, as its keys would, which the tooltip shows unless it
    /// names others.
    pub fn dispatch(mut self, action: &dyn Action) -> Self {
        self.tooltip_action = Some(action.boxed_clone());
        let action = action.boxed_clone();
        self.on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx);
        let (fg, bg, border, hover): (Hsla, Option<Hsla>, Option<Hsla>, Hsla) = match self.style {
            ButtonStyle::Ghost => (theme.text_1, None, None, theme.hover()),
            ButtonStyle::Outline => (
                theme.text_1,
                None,
                Some(theme.control_border()),
                theme.hover(),
            ),
            ButtonStyle::Gray => (
                theme.text_1,
                Some(theme.fill_gray.opacity(0.1)),
                None,
                theme.fill_gray.opacity(0.16),
            ),
            ButtonStyle::Pop => (
                gpui::white(),
                Some(theme.pop_button),
                None,
                theme.pop_button.opacity(0.9),
            ),
            ButtonStyle::Danger => (
                theme.danger,
                Some(theme.danger.opacity(0.1)),
                None,
                theme.danger.opacity(0.16),
            ),
        };
        let (fg, bg, hover) = match self.tint {
            Some(tint) => (tint, bg.map(|_| tint.opacity(0.12)), tint.opacity(0.16)),
            None if self.inverted && self.style != ButtonStyle::Pop => (
                theme.text_inverted,
                bg.map(|_| theme.text_inverted.opacity(0.14)),
                theme.text_inverted.opacity(0.22),
            ),
            None => (fg, bg, hover),
        };
        // As ui-react's: small is 20 tall with a 14 icon, an icon alone 22 wide; regular 28.
        let (height, width, icon_size) = match self.small {
            true => (dp(20.), dp(22.), dp(14.)),
            false => (dp(28.), dp(28.), dp(16.)),
        };
        let has_label = self.label.is_some();
        // What a screen reader calls it: its text, or for an icon its tooltip.
        let name = self.label.clone().or_else(|| self.tooltip.clone());
        h_flex()
            .id(self.id)
            .role(Role::Button)
            .when_some(name, |el, name| el.aria_label(name))
            .flex_none()
            .h(height)
            .gap(dp(if self.small { 4. } else { 6. }))
            .when(has_label, |el| el.px(dp(if self.small { 6. } else { 10. })))
            .when(!has_label, |el| el.w(width).justify_center())
            .text_size(dp(if self.small { 12. } else { 13. }))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .text_color(fg)
            .when_some(bg, |el, bg| el.bg(bg))
            .when_some(border, |el, border| el.border_1().border_color(border))
            // The halves meet square, split by a seam: an outline's border, or a faint rule
            // across a fill.
            .map(|el| match self.half {
                None => el.rounded(dp(4.)),
                Some(Half::Action) => el.rounded_l(dp(4.)).border_r_0(),
                Some(Half::Menu) => (el.rounded_r(dp(4.)).border_l_1())
                    .border_color(border.unwrap_or(fg.opacity(0.3))),
            })
            .when(self.disabled.is_none(), |el| {
                el.hover(move |el| el.bg(hover))
                    // Gives under the pointer while pressed.
                    .active(|el| el.opacity(0.8))
            })
            .when(self.disabled.is_some(), |el| el.opacity(0.5))
            .when_some(self.icon, |el, kind| {
                let opacity = if has_label { 0.7 } else { 0.8 };
                el.child(icon(kind).size(icon_size).text_color(fg.opacity(opacity)))
            })
            .when_some(self.label, |el, label| el.child(label))
            .when_some(self.disabled.clone().or(self.tooltip), |el, tooltip| {
                let enabled = self.disabled.is_none();
                let key = self.tooltip_key.filter(|_| enabled);
                let action = self.tooltip_action.filter(|_| enabled);
                el.tooltip(move |window, cx| {
                    let key = key.clone().or_else(|| shortcut(action.as_deref()?, cx));
                    Tooltip::keyed(tooltip.clone(), key)(window, cx)
                })
            })
            .when_some(
                self.on_click.filter(|_| self.disabled.is_none()),
                |el, handler| {
                    tab_stop(el, theme).on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        handler(event, window, cx)
                    })
                },
            )
    }
}

/// `action` joined to a chevron that opens its menu, as ui-react's `DropdownButton`: one
/// control split by a seam. The chevron, called `menu_label`, is disabled with the action.
pub fn dropdown_button(
    action: Button,
    menu_label: impl Into<SharedString>,
    on_menu: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let menu = Button {
        label: None,
        icon: Some(Icon::ChevronDown),
        style: action.style,
        disabled: action.disabled.clone(),
        small: action.small,
        tooltip: Some(menu_label.into()),
        half: Some(Half::Menu),
        ..button(
            ElementId::NamedChild(std::sync::Arc::new(action.id.clone()), "menu".into()),
            "",
        )
    };
    let action = Button {
        half: Some(Half::Action),
        ..action
    };
    h_flex()
        .flex_none()
        .child(action)
        .child(menu.on_click(on_menu))
}

/// A plain text tooltip, with the keys that do the same thing at its end.
pub struct Tooltip {
    text: SharedString,
    key: Option<SharedString>,
    /// When it first showed: what had covered the window by then, and where the pointer was.
    shown: Option<(u64, Point<Pixels>)>,
}

/// What covers the window, as a menu, picker, prompt or modal does: how many times something
/// came to, and where the pointer was when the one up now came. A tooltip gpui showed for an
/// element under it, or was about to, as a click opened it, stays until the pointer leaves the
/// element; this tells the tooltip to hide meanwhile.
#[derive(Default)]
pub struct Covered {
    times: u64,
    at: Option<Point<Pixels>>,
}

impl gpui::Global for Covered {}

impl Covered {
    /// Note whether something covers the window now.
    pub fn note(covered: bool, window: &Window, cx: &mut App) {
        let state = cx.default_global::<Covered>();
        match (covered, state.at) {
            (true, None) => {
                state.times += 1;
                state.at = Some(window.mouse_position());
            }
            (false, Some(_)) => state.at = None,
            _ => {}
        }
    }
}

impl Tooltip {
    /// A tooltip of `text` with the shortcut `key`, for an element's `tooltip`.
    pub fn keyed(
        text: impl Into<SharedString>,
        key: Option<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
        let text = text.into();
        move |_, cx| {
            let (text, key) = (text.clone(), key.clone());
            cx.new(|_| Tooltip {
                text,
                key,
                shown: None,
            })
            .into()
        }
    }

    /// A tooltip of just `text`, for an element's `tooltip`.
    pub fn text(
        text: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
        Self::keyed(text, None)
    }
}

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let (times, at) = cx
            .try_global::<Covered>()
            .map_or((0, None), |c| (c.times, c.at));
        let shown = *self.shown.get_or_insert((times, window.mouse_position()));
        // An element's under what covers the window when its tooltip showed before that came,
        // or since without the pointer moving onto something on top.
        let under = at.is_some_and(|at| shown.0 != times || shown.1 == at);
        let theme = Theme::get(cx);
        popover(theme)
            .when(under, |el| el.invisible())
            .flex_row()
            .items_center()
            .mt(dp(4.))
            .max_w(dp(240.))
            .rounded(dp(4.))
            .px(dp(8.))
            .py(dp(6.))
            .text_size(dp(12.))
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
            .gap(dp(7.))
            // Within the cap, a long one wraps, a path between its characters.
            .child(div().min_w_0().child(self.text.clone()))
            .when_some(self.key.clone(), |el, key| {
                el.child(keycap(&key, theme).flex_none())
            })
    }
}

/// The keys bound to `action`, as macOS writes them, like `⌘⇧P`: [`keycap`] and [`keys_text`]
/// show them as the platform does.
pub fn shortcut(action: &dyn gpui::Action, cx: &App) -> Option<SharedString> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    Some(keys_of(keymap.bindings_for_action(action).next_back()?))
}

/// Every key that runs `action`, as [`shortcut`] writes keys, with the context it works in.
pub fn bindings_of(action: &dyn gpui::Action, cx: &App) -> Vec<(SharedString, Option<String>)> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    (keymap.bindings_for_action(action))
        .map(|binding| (keys_of(binding), binding.predicate().map(|p| p.to_string())))
        .collect()
}

/// `keys`, as a keymap writes them (`secondary-shift-p`), as [`shortcut`] writes keys.
pub fn keys_written(keys: &str, cx: &App) -> Option<SharedString> {
    let mapper = cx.keyboard_mapper();
    let binding = (gpui::KeyBinding::load(
        keys,
        Box::new(gpui::NoAction),
        None,
        false,
        None,
        mapper.as_ref(),
    ))
    .ok()?;
    Some(keys_of(&binding))
}

/// The keys that run `action` where the keys go now, unless the same keys do something else
/// there, as [`shortcut`] writes them.
pub fn shortcut_here(action: &dyn gpui::Action, window: &Window, cx: &App) -> Option<SharedString> {
    let focused = window.focused(cx)?;
    Some(keys_of(&window.highest_precedence_binding_for_action_in(
        action, &focused,
    )?))
}

fn keys_of(binding: &gpui::KeyBinding) -> SharedString {
    let keys: Vec<String> = binding
        .keystrokes()
        .iter()
        .map(|keystroke| {
            let m = keystroke.modifiers();
            let mut text = String::new();
            for (on, symbol) in [
                (m.control, "⌃"),
                (m.alt, "⌥"),
                (m.shift, "⇧"),
                (m.platform, "⌘"),
            ] {
                if on {
                    text.push_str(symbol);
                }
            }
            text.push_str(match keystroke.key() {
                "enter" => "↩",
                "backspace" => "⌫",
                "delete" => "⌦",
                "escape" => "esc",
                "space" => "space",
                "tab" => "⇥",
                "up" => "↑",
                "down" => "↓",
                "left" => "←",
                "right" => "→",
                key if key.len() == 1
                    || key
                        .strip_prefix('f')
                        .is_some_and(|n| n.parse::<u8>().is_ok()) =>
                {
                    return text + &key.to_uppercase();
                }
                key => key,
            });
            text
        })
        .collect();
    keys.join(" ").into()
}

/// What Windows calls the keys macOS writes as symbols. The app's ⌘ keys are ctrl there, and a
/// modifier's name ends in `+`, as it's written before the key.
#[cfg(not(target_os = "macos"))]
const KEY_NAMES: [(char, &str); 8] = [
    ('⌘', "Ctrl+"),
    ('⌃', "Ctrl+"),
    ('⌥', "Alt+"),
    ('⇧', "Shift+"),
    ('↩', "Enter"),
    ('⌫', "Backspace"),
    ('⌦', "Delete"),
    ('⇥', "Tab"),
];

/// `text` with the keys in it, written as macOS writes them (`⌘⇧P`), as the platform writes
/// them: unchanged on macOS, `Ctrl+Shift+P` on Windows.
pub fn keys_text(text: &str) -> String {
    #[cfg(target_os = "macos")]
    return text.to_owned();
    #[cfg(not(target_os = "macos"))]
    text.chars()
        .map(
            |c| match KEY_NAMES.iter().find(|(symbol, _)| *symbol == c) {
                Some((_, name)) => (*name).to_owned(),
                None => c.to_string(),
            },
        )
        .collect()
}

/// Keys as Lite's `Kbd` draws them: a small box per key, like `⌘` `⇧` `P`, or `Ctrl` `Shift`
/// `P` on Windows. `text` writes them as macOS does.
pub fn keycap(text: &str, theme: &Theme) -> Div {
    let mut keys: Vec<String> = Vec::new();
    for token in text.split(' ') {
        let mut rest = token;
        while let Some(symbol) = rest.chars().next().filter(|c| "⌃⌥⇧⌘".contains(*c)) {
            keys.push(
                keys_text(&symbol.to_string())
                    .trim_end_matches('+')
                    .to_owned(),
            );
            rest = rest.strip_prefix(symbol).unwrap_or_default();
        }
        if !rest.is_empty() {
            keys.push(keys_text(rest));
        }
    }
    h_flex()
        .flex_none()
        .gap(dp(3.))
        .children(keys.into_iter().map(|key| {
            div()
                .flex_none()
                .min_w(dp(16.))
                .h(dp(16.))
                .px(dp(3.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(dp(3.))
                .bg(theme.fill_gray.opacity(0.05))
                .border_1()
                .border_color(theme.text_1.opacity(0.16))
                .text_size(dp(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_1.opacity(0.6))
                .child(key)
        }))
}

/// A small round marker: new work, or something running.
pub fn dot(color: Hsla) -> Div {
    div().flex_none().size(dp(6.)).rounded_full().bg(color)
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

/// A small count in a row's meta, coloured by meaning (amber waits on you, blue is
/// unpushed), saying on hover what it counts; nothing at zero.
pub fn meta_count(glyph: Icon, n: usize, color: Hsla, tip: String) -> Option<Stateful<Div>> {
    let tip = SharedString::from(tip);
    (n > 0).then(|| {
        h_flex()
            .id(ElementId::Name(tip.clone()))
            .flex_none()
            .gap(dp(3.))
            .text_size(dp(11.))
            .text_color(color)
            .child(small_icon(glyph).text_color(color))
            .child(n.to_string())
            .tooltip(Tooltip::text(tip))
    })
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

/// A faint pop wash over a `list_row`'s ground that fades out once, as for a commit that just
/// showed up (`Store::glows`), so an agent's commit is noticed where it lands rather than in a
/// card. Make it the row's first child, of a `relative` row, so the row's text draws over it.
/// A child rather than a wrapper, so the row keeps its identity, and with it a press about to
/// become a drag, when the wash goes. Keep it off a selected row, whose ground marks it.
pub fn glow_once(id: impl Into<ElementId>, theme: &Theme) -> impl IntoElement {
    use gpui::{Animation, AnimationExt as _};
    let wash = theme.pop;
    div().absolute().inset_0().rounded(dp(4.)).with_animation(
        id,
        Animation::new(crate::state::GLOW).with_easing(gpui::ease_in_out),
        move |el, delta| el.bg(wash.opacity(0.08 * (1. - delta))),
    )
}

/// A surface with nothing in it, as ui-react's EmptyState: a drawing picked by what the state
/// means, a title naming it, and a line saying what happens next, each left out when there's
/// none. Its foot is padded so the block sits a little above the middle, where it reads as
/// centred, and it caps its width so it stays a block in a wide pane. In a pane too short for
/// it, it's cut off rather than drawn over what's above.
pub fn empty_state(
    drawing: Option<Illustration>,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let (title, detail): (SharedString, SharedString) = (title.into(), detail.into());
    v_flex()
        .size_full()
        .overflow_hidden()
        .items_center()
        .justify_center()
        .px(dp(32.))
        .pb(dp(60.))
        .text_center()
        .child(
            v_flex()
                .max_w(dp(350.))
                // So a short pane shrinks the drawing rather than clip it.
                .min_h_0()
                .items_center()
                .gap(dp(24.))
                .children(drawing.map(|drawing| illustration(drawing, theme, cx)))
                .child(
                    v_flex()
                        .items_center()
                        .gap(dp(8.))
                        .when(!title.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(dp(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.text_1)
                                    .child(title),
                            )
                        })
                        .when(!detail.is_empty(), |el| el.child(muted(detail, theme))),
                ),
        )
        .into_any_element()
}

/// How many, in a small grey pill, as ui-react's light grey `Badge`.
pub fn count_badge(count: usize, theme: &Theme) -> Div {
    div()
        .flex_none()
        .h(dp(16.))
        .min_w(dp(16.))
        .px(dp(5.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme.fill_gray.opacity(0.15))
        .text_size(dp(11.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.text_1)
        .child(count.to_string())
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

/// Resize `pane` while its handle is dragged, measuring from the left of the element this
/// listens on.
pub fn resize_pane(
    pane: Pane,
    store: Entity<Store>,
) -> impl Fn(&DragMoveEvent<Pane>, &mut Window, &mut App) + 'static {
    move |event, _, cx| {
        if *event.drag(cx) == pane {
            let width = f32::from(event.event.position.x - event.bounds.left());
            let width = width / Theme::get(cx).scale;
            store.update(cx, |store, cx| store.set_width(pane, Some(width), cx));
        }
    }
}
