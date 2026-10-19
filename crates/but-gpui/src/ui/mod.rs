//! Shared building blocks so every surface draws rows, buttons and counts the same way.

pub mod conflict;
pub mod diff;
pub mod emoji;
pub mod file_icons;
pub mod illustration;
pub mod markdown;
pub mod modal;
pub mod person;
pub mod review;
pub mod scrollbar;
pub mod text_input;

use std::borrow::Cow;

use gpui::{
    Action, AnyElement, App, AssetSource, ClickEvent, CursorStyle, Div, DragMoveEvent, ElementId,
    EmptyView, Entity, Hsla, Pixels, Point, Role, SharedString, Stateful, Svg, Toggled, Window,
    div, prelude::*, px, svg,
};

use crate::state::{Pane, Store};
use crate::theme::Theme;
use illustration::{Illustration, illustration};

pub const ROW_HEIGHT: f32 = 28.;
pub const HEADER_HEIGHT: f32 = 44.;
pub const STATUS_BAR_HEIGHT: f32 = 28.;

/// Every icon: the design system's, compiled in from `packages/ui-react` so they're the ones
/// Lite and but.dev draw, and a build fails if one goes; then this app's own, under
/// `assets/icons`, for what ui-react draws in code rather than from a file.
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
    Ai => "ai",
    ArrowDown => "arrow-down",
    ArrowUp => "arrow-up",
    Branch => "branch",
    BranchMerge => "branch-merge",
    BulletDisc => "bullet-disc",
    ChevronDown => "chevron-down",
    ChevronUp => "chevron-up",
    ChevronRight => "chevron-right",
    Clock => "clock",
    Contrast => "contrast",
    Commit => "commit",
    CommitFill => "commit-fill",
    Camera => "camera",
    Diff => "diff",
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
    Search => "search",
    Settings => "settings",
    Sidebar => "sidebar",
    SidebarNarrow => "sidebar-narrow",
    Stack => "stack",
    TextBlock => "text-block",
    TextCode => "text-code",
    TextWrap => "text-wrap",
    Tick => "tick",
    Undo => "undo",
    Warning => "warning";
    // ui-react's FolderIcon, a component with its gradient in CSS.
    own: FolderFill => "folder-fill",
    // ui-react's eye, struck through, for a project not watched.
    EyeOff => "eye-off",
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
    svg().path(icon.path()).size(px(16.)).flex_none()
}

pub fn small_icon(icon: Icon) -> Svg {
    svg().path(icon.path()).size(px(12.)).flex_none()
}

/// A row of toggles in one bordered group, as ui-react's `ToggleGroupStyles`.
pub fn toggle_group(theme: &Theme) -> Div {
    h_flex()
        .flex_none()
        .p(px(2.))
        .gap(px(2.))
        .rounded(px(6.))
        .border_1()
        .border_color(theme.border_3)
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
        .text_size(px(12.))
        .text_color(theme.text_2)
        .child(text)
}

/// Faint small text, for details that can be skipped.
pub fn faint(text: impl IntoElement, theme: &Theme) -> Div {
    div()
        .text_size(px(11.))
        .text_color(theme.text_3)
        .child(text)
}

/// The heading over a group of rows.
pub fn section_label(text: impl IntoElement, theme: &Theme) -> Div {
    faint(text, theme).font_weight(gpui::FontWeight::SEMIBOLD)
}

/// A row of a list or menu, shaded when `selected` and otherwise under the pointer.
pub fn list_row(id: impl Into<ElementId>, selected: bool, theme: &Theme) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::ListItem)
        .aria_selected(selected)
        .flex_none()
        .h(px(ROW_HEIGHT))
        .mx(px(6.))
        .px(px(8.))
        .gap(px(8.))
        .rounded(px(4.))
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
        el.relative().rounded_t(px(0.)).child(
            div()
                .absolute()
                .top_0()
                .left(px(8.))
                .right(px(8.))
                .h(px(1.))
                .bg(line),
        )
    })
    .when(below, |el| el.rounded_b(px(0.)))
}

/// An on/off switch, for a setting that applies at once.
pub fn switch(id: impl Into<ElementId>, on: bool, theme: &Theme) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::Switch)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .flex_none()
        .w(px(30.))
        .h(px(18.))
        .p(px(2.))
        .rounded_full()
        .bg(if on {
            theme.pop_bg
        } else {
            theme.fill_gray.opacity(0.22)
        })
        .when(on, |el| el.justify_end())
        .hover(|el| el.opacity(0.9))
        .child(div().size(px(14.)).rounded_full().bg(gpui::white()))
}

/// A checkbox, ticked, empty or, for a group partly checked, a dash.
pub fn checkbox(id: impl Into<ElementId>, check: Toggled, theme: &Theme) -> Stateful<Div> {
    let on = check != Toggled::False;
    div()
        .id(id)
        .role(Role::CheckBox)
        .aria_toggled(check)
        .flex_none()
        .size(px(14.))
        .rounded(px(3.))
        .border_1()
        .border_color(if on { theme.pop } else { theme.border_2 })
        .when(on, |el| el.bg(theme.pop))
        .flex()
        .items_center()
        .justify_center()
        .map(|el| match check {
            Toggled::True => el.child(
                small_icon(Icon::Tick)
                    .size(px(10.))
                    .text_color(gpui::white()),
            ),
            Toggled::Mixed => el.child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(gpui::white())),
            Toggled::False => el,
        })
}

/// A button showing a choice, which opens a menu of the others when clicked.
pub fn dropdown(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::ComboBox)
        .flex_none()
        .h(px(28.))
        .pl(px(10.))
        .pr(px(6.))
        .gap(px(6.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme.border_2)
        .text_size(px(13.))
        .hover(|el| el.bg(theme.hover()))
        .child(label.into())
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
        .px(px(8.))
        .py(px(5.))
        .rounded(px(4.))
        .border_1()
        .border_color(if focused { theme.pop } else { theme.border_2 })
        .bg(theme.bg_1)
        .text_size(px(13.))
        .child(input.clone())
}

/// The card that menus, dialogs, toasts and tooltips float on.
pub fn popover(theme: &Theme) -> Div {
    v_flex()
        .rounded(px(8.))
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

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Dispatch `action` when clicked, as its keys would.
    pub fn dispatch(self, action: &dyn Action) -> Self {
        let action = action.boxed_clone();
        self.on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx);
        let (fg, bg, border, hover): (Hsla, Option<Hsla>, Option<Hsla>, Hsla) = match self.style {
            ButtonStyle::Ghost => (theme.text_1, None, None, theme.hover()),
            ButtonStyle::Outline => (theme.text_1, None, Some(theme.border_2), theme.hover()),
            ButtonStyle::Gray => (
                theme.text_1,
                Some(theme.fill_gray.opacity(0.1)),
                None,
                theme.fill_gray.opacity(0.16),
            ),
            ButtonStyle::Pop => (
                gpui::white(),
                Some(theme.pop_bg),
                None,
                theme.pop_bg.opacity(0.85),
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
        let height = if self.small { px(22.) } else { px(28.) };
        let has_label = self.label.is_some();
        // What a screen reader calls it: its text, or for an icon its tooltip.
        let name = self.label.clone().or_else(|| self.tooltip.clone());
        h_flex()
            .id(self.id)
            .role(Role::Button)
            .when_some(name, |el, name| el.aria_label(name))
            .flex_none()
            .h(height)
            .gap(px(6.))
            .when(has_label, |el| el.px(px(if self.small { 8. } else { 10. })))
            .when(!has_label, |el| el.w(height).justify_center())
            .rounded(px(4.))
            .text_size(px(if self.small { 12. } else { 13. }))
            .whitespace_nowrap()
            .text_color(fg)
            .when_some(bg, |el, bg| el.bg(bg))
            .when_some(border, |el, border| el.border_1().border_color(border))
            .when(self.disabled.is_none(), |el| {
                el.hover(move |el| el.bg(hover))
                    // Gives under the pointer while pressed.
                    .active(|el| el.opacity(0.8))
            })
            .when(self.disabled.is_some(), |el| el.opacity(0.45))
            .when_some(self.icon, |el, kind| {
                el.child(icon(kind).text_color(fg.opacity(if has_label { 0.7 } else { 0.8 })))
            })
            .when_some(self.label, |el, label| el.child(label))
            .when_some(self.disabled.clone().or(self.tooltip), |el, tooltip| {
                let key = self.tooltip_key.filter(|_| self.disabled.is_none());
                el.tooltip(move |_, cx| Tooltip::with_key(tooltip.clone(), key.clone(), cx))
            })
            .when_some(
                self.on_click.filter(|_| self.disabled.is_none()),
                |el, handler| {
                    el.on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        handler(event, window, cx)
                    })
                },
            )
    }
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
    pub fn with_key(text: SharedString, key: Option<SharedString>, cx: &mut App) -> gpui::AnyView {
        cx.new(|_| Tooltip {
            text,
            key,
            shown: None,
        })
        .into()
    }

    /// A tooltip of just `text`, for an element's `tooltip`.
    pub fn text(
        text: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
        let text = text.into();
        move |_, cx| Self::with_key(text.clone(), None, cx)
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
            .mt(px(4.))
            .px(px(8.))
            .py(px(4.))
            .text_size(px(12.))
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
            .gap(px(7.))
            .child(self.text.clone())
            .when_some(self.key.clone(), |el, key| el.child(keycap(&key, theme)))
    }
}

/// The keys bound to `action`, as macOS shows them, like `⌘⇧P`.
pub fn shortcut(action: &dyn gpui::Action, cx: &App) -> Option<SharedString> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let binding = keymap.bindings_for_action(action).next_back()?;
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
                key if key.len() == 1 => {
                    return text + &key.to_uppercase();
                }
                key => key,
            });
            text
        })
        .collect();
    Some(keys.join(" ").into())
}

/// Keys as Lite's `Kbd` draws them: a small box per key, like `⌘` `⇧` `P`.
pub fn keycap(text: &str, theme: &Theme) -> Div {
    let mut keys: Vec<String> = Vec::new();
    for token in text.split(' ') {
        let mut rest = token;
        while let Some(symbol) = rest.chars().next().filter(|c| "⌃⌥⇧⌘".contains(*c)) {
            keys.push(symbol.to_string());
            rest = &rest[symbol.len_utf8()..];
        }
        if !rest.is_empty() {
            keys.push(rest.to_owned());
        }
    }
    h_flex()
        .flex_none()
        .gap(px(3.))
        .children(keys.into_iter().map(|key| {
            div()
                .flex_none()
                .min_w(px(16.))
                .h(px(16.))
                .px(px(3.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .bg(theme.fill_gray.opacity(0.05))
                .border_1()
                .border_color(theme.text_1.opacity(0.16))
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_1.opacity(0.6))
                .child(key)
        }))
}

/// A small round marker: new work, or something running.
pub fn dot(color: Hsla) -> Div {
    div().flex_none().size(px(6.)).rounded_full().bg(color)
}

/// Seconds since the epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Whether `seconds` (since the epoch) is within the last two minutes.
pub fn just_now(seconds: i64) -> bool {
    now() - seconds < 120
}

/// A commit's title as shown, with a stand-in when its message is empty.
pub fn commit_title(title: &str) -> SharedString {
    match title {
        "" => "No message".into(),
        title => title.to_owned().into(),
    }
}

/// "1 commit", "3 commits".
pub fn plural(n: usize, word: &str) -> String {
    match n {
        1 => format!("1 {word}"),
        n => format!("{n} {word}s"),
    }
}

/// `text` quoted in a menu item, cut to 40 characters.
pub fn quoted(text: &str) -> String {
    match text.chars().count() > 40 {
        true => format!("\"{}…\"", text.chars().take(39).collect::<String>()),
        false => format!("\"{text}\""),
    }
}

/// `path` as shown, with the home folder as `~`, so one cut short keeps more of its end.
pub fn short_path(path: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match home.and_then(|home| {
        path.strip_prefix(home)
            .ok()
            .map(|rest| rest.display().to_string())
    }) {
        Some(rest) => format!("~/{rest}"),
        None => path.display().to_string(),
    }
}

/// What to show of an error: Git's own complaints when it ran Git, like "fatal: Could not
/// read from remote repository.", and what the remote said back, as a server's hook does
/// when it refuses a push, as the rest repeats the command it ran; else its first lines,
/// without the codes libgit2 appends ("; class=Index (10); code=Locked (-14)").
pub fn error_summary(text: &str) -> String {
    let lines: Vec<&str> = (text.lines())
        .map(|line| line.split("; class=").next().unwrap_or(line))
        .collect();
    let mut said: Vec<&str> = Vec::new();
    for line in lines.iter().map(|line| line.trim()) {
        let (remote, line) = match line.strip_prefix("remote:") {
            Some(said) => (true, said.trim()),
            None => (false, line),
        };
        let git = line.starts_with("fatal:") || line.starts_with("error:");
        if line.is_empty() || !(remote || git) {
            continue;
        }
        // Git's line again with a wrapper's ": …" after it keeps its short form.
        let wraps = |long: &str, short: &str| {
            (long.strip_prefix(short)).is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
        };
        match (said.iter_mut()).find(|said| wraps(said, line) || wraps(line, said)) {
            Some(said) if line.len() < said.len() => *said = line,
            Some(_) => {}
            None => said.push(line),
        }
    }
    match said.is_empty() {
        true => lines[..lines.len().min(4)].join("\n"),
        false => said.join("\n"),
    }
}

pub fn relative_time(seconds: i64) -> String {
    let delta = (now() - seconds).max(0);
    match delta {
        0..60 => "now".into(),
        60..3600 => format!("{}m", delta / 60),
        3600..86_400 => format!("{}h", delta / 3600),
        86_400..2_592_000 => format!("{}d", delta / 86_400),
        2_592_000..31_536_000 => format!("{}mo", delta / 2_592_000),
        _ => format!("{}y", delta / 31_536_000),
    }
}

/// How long ago `seconds` (since the epoch) was, faint, in a slot as wide as the longest
/// ("11mo"), so what sits before it lines up from row to row; empty without a time. It starts
/// at the slot's left, beside what comes before.
pub fn age_column(seconds: Option<i64>, theme: &Theme) -> Div {
    faint(seconds.map(relative_time).unwrap_or_default(), theme)
        .flex_none()
        .w(px(30.))
        .whitespace_nowrap()
}

/// `seconds` (since the epoch) as the time here, the way but.dev dates a commit:
/// "16/10/2026, 21:04".
pub fn local_time(seconds: i64) -> String {
    local(seconds, "%d/%m/%Y, %H:%M")
}

/// `seconds` (since the epoch) in `format`, at the time here.
fn local(seconds: i64, format: &'static str) -> String {
    let offset = gix::date::Time::now_local_or_utc().offset;
    gix::date::Time::new(seconds, offset).format_or_unix(gix::date::time::CustomFormat::new(format))
}

/// The day `seconds` (since the epoch) fell on here, to tell one day's from another's.
pub fn local_day(seconds: i64) -> String {
    local(seconds, "%Y-%m-%d")
}

/// The day `seconds` fell on, as Tauri's history heads a day's operations: Today, Yesterday,
/// or the weekday and date, with the year when it isn't this one.
pub fn day_label(seconds: i64) -> String {
    let today = now();
    match local_day(seconds) {
        day if day == local_day(today) => "Today".into(),
        day if day == local_day(today - 86_400) => "Yesterday".into(),
        _ if local(seconds, "%Y") == local(today, "%Y") => local(seconds, "%a, %-d %b"),
        _ => local(seconds, "%a, %-d %b %Y"),
    }
}

/// The time of day `seconds` fell on here, like 14:05.
pub fn clock_time(seconds: i64) -> String {
    local(seconds, "%H:%M")
}

/// A surface with nothing in it, as ui-react's EmptyState: a drawing picked by what the state
/// means, a title naming it, and a line saying what happens next, each left out when there's
/// none. Its foot is padded so the block sits a little above the middle, where it reads as
/// centred, and it caps its width so it stays a block in a wide pane.
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
        .items_center()
        .justify_center()
        .px(px(32.))
        .pb(px(60.))
        .text_center()
        .child(
            v_flex()
                .max_w(px(350.))
                .items_center()
                .gap(px(24.))
                .children(drawing.map(|drawing| illustration(drawing, theme, cx)))
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(8.))
                        .when(!title.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(14.))
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
        .px(px(6.))
        .rounded_full()
        .bg(theme.fill_gray.opacity(0.1))
        .text_size(px(11.))
        .text_color(theme.text_2)
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
        .right(px(-2.))
        .w(px(5.))
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
                .left(px(2.))
                .w(px(1.))
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
            store.update(cx, |store, cx| store.set_width(pane, Some(width), cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::error_summary;

    #[test]
    fn a_git_failure_shows_what_git_said_once() {
        let failed = "fetching remote `origin` failed: git command exited with code 128:\n\n\
            ARGS:\n[\"fetch\"]\n\nSTDERR:\nfatal: '/x.git' does not appear to be a git repository\n\
            fatal: Could not read from remote repository.\n: git command exited with code 128:\n\n\
            STDERR:\nfatal: '/x.git' does not appear to be a git repository\n";
        assert_eq!(
            error_summary(failed),
            "fatal: '/x.git' does not appear to be a git repository\n\
             fatal: Could not read from remote repository.",
            "Git's own lines, each once"
        );
        let rejected = "error: remote unpack failed\n ! [remote rejected] a -> b\n\
            error: failed to push some refs to '/u.git': git command exited with code 1:\n\n\
            STDERR:\nerror: remote unpack failed\nerror: failed to push some refs to '/u.git'\n";
        assert_eq!(
            error_summary(rejected),
            "error: remote unpack failed\nerror: failed to push some refs to '/u.git'",
            "a line repeated with the wrapper's words after it, in its short form"
        );
        let declined = "git command exited with non-zero exit code 1:\nSTDERR:\n\
            remote: pushes are frozen for the release        \n\
            ! [remote rejected] a -> b (pre-receive hook declined)\n\
            error: failed to push some refs to '/u.git'\n";
        assert_eq!(
            error_summary(declined),
            "pushes are frozen for the release\nerror: failed to push some refs to '/u.git'",
            "the remote's reason, as its hook gave it"
        );
        assert_eq!(
            error_summary("the index is locked; class=Index (10); code=Locked (-14)"),
            "the index is locked",
            "without libgit2's codes"
        );
        assert_eq!(
            error_summary("Branch not found\nmore\nand more\nstill\ncut"),
            "Branch not found\nmore\nand more\nstill",
            "any other error by its first lines"
        );
    }
}
