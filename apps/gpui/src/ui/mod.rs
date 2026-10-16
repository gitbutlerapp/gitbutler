//! Shared building blocks so every surface draws rows, buttons and counts the same way.

pub mod diff;
pub mod text_input;

use std::borrow::Cow;

use gpui::{
    AnyElement, App, AssetSource, ClickEvent, Div, ElementId, Hsla, SharedString, Svg, Window, div,
    prelude::*, px, svg,
};

use crate::theme::Theme;

pub const ROW_HEIGHT: f32 = 28.;
pub const HEADER_HEIGHT: f32 = 44.;
pub const STATUS_BAR_HEIGHT: f32 = 28.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    ArrowDown,
    ArrowUp,
    Branch,
    ChevronDown,
    ChevronRight,
    Clock,
    Commit,
    Eye,
    CommitFill,
    File,
    FileDiff,
    Plus,
    Pr,
    Refresh,
    Search,
    Sidebar,
    Stack,
    Tick,
    Warning,
}

impl Icon {
    fn file(self) -> &'static str {
        match self {
            Icon::ArrowDown => "arrow-down",
            Icon::ArrowUp => "arrow-up",
            Icon::Branch => "branch",
            Icon::ChevronDown => "chevron-down",
            Icon::ChevronRight => "chevron-right",
            Icon::Clock => "clock",
            Icon::Commit => "commit",
            Icon::Eye => "eye",
            Icon::CommitFill => "commit-fill",
            Icon::File => "file",
            Icon::FileDiff => "file-diff",
            Icon::Plus => "plus",
            Icon::Pr => "pr",
            Icon::Refresh => "refresh",
            Icon::Search => "search",
            Icon::Sidebar => "sidebar",
            Icon::Stack => "stack",
            Icon::Tick => "tick",
            Icon::Warning => "warning",
        }
    }

    fn path(self) -> SharedString {
        format!("icons/{}.svg", self.file()).into()
    }
}

/// Serves the icons compiled into the binary.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        macro_rules! icons {
            ($($name:literal),* $(,)?) => {
                match path {
                    $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!("../../assets/icons/", $name, ".svg")).as_slice()),)*
                    _ => None,
                }
            };
        }
        let bytes = icons!(
            "arrow-down",
            "arrow-up",
            "branch",
            "chevron-down",
            "chevron-right",
            "clock",
            "commit",
            "commit-fill",
            "file",
            "file-diff",
            "plus",
            "pr",
            "refresh",
            "search",
            "sidebar",
            "eye",
            "stack",
            "tick",
            "warning",
        );
        Ok(bytes.map(Cow::Borrowed))
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

/// A horizontal flex row with centred items.
pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

pub fn v_flex() -> Div {
    div().flex().flex_col()
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
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    style: ButtonStyle,
    disabled: bool,
    small: bool,
    /// On an inverted ground, like a selected row.
    inverted: bool,
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
        disabled: false,
        small: false,
        inverted: false,
        on_click: None,
        tooltip: None,
        tooltip_key: None,
    }
}

pub fn icon_button(id: impl Into<ElementId>, icon: Icon) -> Button {
    Button {
        id: id.into(),
        label: None,
        icon: Some(icon),
        style: ButtonStyle::Ghost,
        disabled: false,
        small: false,
        inverted: false,
        on_click: None,
        tooltip: None,
        tooltip_key: None,
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
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
        };
        let (fg, bg, hover) = if self.inverted && self.style != ButtonStyle::Pop {
            (
                theme.text_inverted,
                bg.map(|_| theme.text_inverted.opacity(0.14)),
                theme.text_inverted.opacity(0.22),
            )
        } else {
            (fg, bg, hover)
        };
        let height = if self.small { px(22.) } else { px(28.) };
        let has_label = self.label.is_some();
        h_flex()
            .id(self.id)
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
            .when(!self.disabled, |el| el.hover(move |el| el.bg(hover)))
            .when(self.disabled, |el| el.opacity(0.45))
            .when_some(self.icon, |el, kind| {
                el.child(icon(kind).text_color(fg.opacity(if has_label { 0.7 } else { 0.8 })))
            })
            .when_some(self.label, |el, label| el.child(label))
            .when_some(self.tooltip, |el, tooltip| {
                let key = self.tooltip_key;
                el.tooltip(move |_, cx| Tooltip::with_key(tooltip.clone(), key.clone(), cx))
            })
            .when_some(self.on_click.filter(|_| !self.disabled), |el, handler| {
                el.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
    }
}

/// A plain text tooltip.
/// A plain text tooltip, with the keys that do the same thing at its end.
pub struct Tooltip {
    text: SharedString,
    key: Option<SharedString>,
}

impl Tooltip {
    pub fn text(text: SharedString, cx: &mut App) -> gpui::AnyView {
        cx.new(|_| Tooltip { text, key: None }).into()
    }

    pub fn with_key(text: SharedString, key: Option<SharedString>, cx: &mut App) -> gpui::AnyView {
        cx.new(|_| Tooltip { text, key }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        div()
            .mt(px(4.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(theme.bg_1)
            .border_1()
            .border_color(theme.border_2)
            .shadow_md()
            .text_size(px(12.))
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
            .flex()
            .items_center()
            .gap(px(7.))
            .child(self.text.clone())
            .when_some(self.key.clone(), |el, key| el.child(keycap(&key, theme)))
    }
}

/// A small count, coloured by meaning: amber waits on you, blue is unpushed.
pub fn count(icon_kind: Option<Icon>, value: usize, color: Hsla) -> Div {
    h_flex()
        .flex_none()
        .gap(px(3.))
        .text_size(px(11.))
        .text_color(color)
        .when_some(icon_kind, |el, kind| {
            el.child(small_icon(kind).text_color(color))
        })
        .child(value.to_string())
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

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Whether `seconds` (since the epoch) is within the last two minutes.
pub fn just_now(seconds: i64) -> bool {
    now() - seconds < 120
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

pub fn empty_state(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    theme: &Theme,
) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(14.))
                .text_color(theme.text_1)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(detail.into()),
        )
        .into_any_element()
}
