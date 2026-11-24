//! Buttons, as but-ui-react's `Button` and `DropdownButton`.

use gpui::{
    Action, AnyElement, App, ClickEvent, Div, ElementId, Hsla, Rems, Role, SharedString, Window,
    div, prelude::*,
};

use crate::theme::{Hue, Theme, tokens};
use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Ghost,
    Outline,
    Gray,
    /// The one primary action of a surface.
    Pop,
    Danger,
    /// An outline in blue, for work to send: a branch's Push, with its unpushed count.
    Info,
    /// An outline in amber, for work waiting on you: Update for what's behind, Resolve.
    Warning,
    /// An outline in green, for what an agent or a check finished: Merge once it's green.
    Success,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    /// An icon after the label, as a count's glyph follows its number.
    icon_end: Option<Icon>,
    picture: Option<AnyElement>,
    style: ButtonStyle,
    /// Held down, as a toggle that's on: React's `data-pressed`.
    pressed: bool,
    /// Why it can't be pressed, shown in place of its tooltip.
    disabled: Option<SharedString>,
    small: bool,
    /// 24px, as React's `medium`: a row of buttons in a dense header.
    medium: bool,
    min_width: Option<Rems>,
    inverted: bool,
    /// The hue of the ground it sits on, as a banner's action takes its banner's.
    tint: Option<Hue>,
    on_click: Option<ClickHandler>,
    tooltip: Option<SharedString>,
    tooltip_key: Option<SharedString>,
    /// The action whose keys the tooltip shows when it names none.
    tooltip_action: Option<Box<dyn Action>>,
    /// Which half of a [`dropdown_button`] it is.
    half: Option<Half>,
    shrinks: bool,
    /// Colours a host restates for its own ground, text and hover, as React's `--button-fg`
    /// and `--button-hover-bg`.
    ink: Option<(Hsla, Hsla)>,
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
        icon_end: None,
        picture: None,
        style: ButtonStyle::Outline,
        pressed: false,
        disabled: None,
        small: false,
        medium: false,
        min_width: None,
        inverted: false,
        tint: None,
        on_click: None,
        tooltip: None,
        tooltip_key: None,
        tooltip_action: None,
        half: None,
        shrinks: false,
        ink: None,
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

/// A picture, sized by the caller, as the round control itself. As with an icon, its tooltip
/// names it.
pub fn picture_button(id: impl Into<ElementId>, picture: impl IntoElement) -> Button {
    Button {
        label: None,
        picture: Some(picture.into_any_element()),
        style: ButtonStyle::Ghost,
        ..button(id, "")
    }
}

impl Button {
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// `icon` after the label, for where the click leads: a chevron opens a menu, an arrow goes on
    /// or out of the app. An icon that names the action is `icon`, before the label.
    pub fn icon_end(mut self, icon: Icon) -> Self {
        self.icon_end = Some(icon);
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// A ghost toggle, on while `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.pressed = selected;
        self.style(ButtonStyle::Ghost)
    }

    /// Unpressable, with `why` as its tooltip; `None` keeps an earlier reason.
    pub fn disabled_because(mut self, why: Option<impl Into<SharedString>>) -> Self {
        self.disabled = why.map(Into::into).or(self.disabled);
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// 24px, as React's `medium`: a row of buttons in a dense header.
    pub fn medium(mut self) -> Self {
        self.medium = true;
        self
    }

    /// Narrower than its label, cut short with an ellipsis, when its row has no room to spare.
    /// Buttons that shrink share what the row lacks; the rest keep their width.
    pub fn shrinks(mut self) -> Self {
        self.shrinks = true;
        self
    }

    /// At least `width` wide, so buttons that stand in for each other cover the same place.
    pub fn min_width(mut self, width: Rems) -> Self {
        self.min_width = Some(width);
        self
    }

    /// A ghost's text and hover restated for a ground it can't know, as the snackbar's.
    pub(crate) fn ink(mut self, text: Hsla, hover: Hsla) -> Self {
        self.ink = Some((text, hover));
        self
    }

    pub fn tint(mut self, tint: Hue) -> Self {
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

    /// A tooltip with the keys bound to `action`, looked up when it shows.
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

    /// Dispatches `action` on click; the tooltip shows its keys unless it names others.
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
            ButtonStyle::Ghost if self.pressed => (
                theme.foreground,
                Some(theme.muted),
                None,
                theme.derived.muted_hover,
            ),
            ButtonStyle::Ghost => (theme.foreground, None, None, theme.hover),
            ButtonStyle::Outline => (theme.foreground, None, Some(theme.input), theme.hover),
            ButtonStyle::Gray => (
                theme.inverse_foreground,
                Some(theme.inverse),
                None,
                theme.inverse_hover,
            ),
            ButtonStyle::Pop => (
                theme.primary_foreground,
                Some(theme.primary),
                None,
                theme.primary_hover,
            ),
            ButtonStyle::Danger => (
                theme.danger_soft_foreground,
                Some(theme.danger_soft),
                None,
                theme.danger_soft_hover,
            ),
            // The colour's text and an edge of it at 40%, and under the pointer a faint
            // see-through tint of it, as React's coloured outlines draw them.
            ButtonStyle::Info => outlined(theme, Hue::Blue),
            ButtonStyle::Warning => outlined(theme, Hue::Warn),
            ButtonStyle::Success => outlined(theme, Hue::Safe),
        };
        let (fg, bg, hover) = match self.tint {
            Some(tint) => (
                match bg {
                    Some(_) => theme.on_soft(tint),
                    None => theme.hue(tint),
                },
                bg.map(|_| theme.soft(tint)),
                match bg {
                    Some(_) => theme.soft_hover(tint),
                    None => theme.soft(tint),
                },
            ),
            // A coloured outline keeps its colour on a solid selection, in the other scheme's
            // value, as but-ui-react's Button flips its colour scheme there, as Badge does.
            None if self.inverted && coloured_hue(self.style).is_some() => {
                let (fg, _, hover) = theme.outline_on_solid(coloured_hue(self.style));
                (fg, bg, hover)
            }
            None if self.inverted && self.style != ButtonStyle::Pop => (
                theme.inverse_foreground,
                bg.map(|_| theme.inverse_soft),
                match bg {
                    Some(_) => theme.inverse_soft_hover,
                    None => theme.inverse_ghost_hover,
                },
            ),
            None => (fg, bg, hover),
        };
        let (fg, hover) = self.ink.unwrap_or((fg, hover));
        let border = match self.inverted && self.tint.is_none() {
            true if coloured_hue(self.style).is_some() => {
                Some(theme.outline_on_solid(coloured_hue(self.style)).1)
            }
            true => border.map(|_| theme.derived.inverse_input),
            false => border,
        };
        // Disabled, it fades as one picture, as React's `opacity: 0.5` does, in solid colours
        // over what it sits on: GPUI's opacity fades each layer on its own, which would let the
        // fill show through the label. A picture can't be recoloured, so it keeps the opacity.
        let pictured = self.picture.is_some();
        let faded = self.disabled.is_some() && !pictured;
        let ground = match self.inverted {
            true => theme.inverse,
            false => theme.background,
        };
        let fade = |color: Hsla| match faded {
            true => Theme::disabled_on(color, ground),
            false => color,
        };
        let (fg, bg, border) = (fade(fg), bg.map(fade), border.map(fade));
        // but-ui's button tokens (`packages/but-ui/components/button.tokens.json`); the heights
        // are `size-control(-md|-sm)`, which `theme.sizes` moves with density.
        let sizes = theme.sizes;
        let (height, width, gap, padding, icon_size) = match self.small {
            false if self.medium => (
                sizes.control_md,
                sizes.control_md + 2.,
                tokens::BUTTON_MEDIUM_GAP,
                tokens::BUTTON_MEDIUM_PADDING_INLINE,
                tokens::BUTTON_MEDIUM_ICON_SIZE,
            ),
            true => (
                sizes.control_sm,
                sizes.control_sm + 2.,
                tokens::BUTTON_SMALL_GAP,
                tokens::BUTTON_SMALL_PADDING_INLINE,
                tokens::BUTTON_SMALL_ICON_SIZE,
            ),
            false => (
                sizes.control,
                sizes.control,
                tokens::BUTTON_REGULAR_GAP,
                tokens::BUTTON_REGULAR_PADDING_INLINE,
                tokens::BUTTON_REGULAR_ICON_SIZE,
            ),
        };
        let has_label = self.label.is_some();
        // Colour loses an icon at 0.7, so a coloured outline's glyph keeps a step up.
        let coloured = matches!(
            self.style,
            ButtonStyle::Info | ButtonStyle::Warning | ButtonStyle::Success
        );
        let icon_opacity = match (has_label, coloured) {
            (true, false) => 0.7,
            _ => 0.8,
        };
        let shrinks = self.shrinks;
        let name = self.label.clone().or_else(|| self.tooltip.clone());
        h_flex()
            .id(self.id)
            .role(Role::Button)
            .when_some(name, |el, name| el.aria_label(name))
            .map(|el| match shrinks {
                true => el.min_w_0(),
                false => el.flex_none(),
            })
            // Room around a picture for the focus ring.
            .map(|el| match pictured {
                true => el.p(dp(2.)),
                false => el.h(dp(height)),
            })
            .gap(dp(gap))
            .when(has_label, |el| el.px(dp(padding + sizes.padding)))
            .when(!has_label && !pictured, |el| {
                el.w(dp(width)).justify_center()
            })
            .when_some(self.min_width, |el, width| el.min_w(width).justify_center())
            .map(|el| {
                if self.small || self.medium {
                    el.text_12()
                } else {
                    el.text_13()
                }
            })
            .semibold()
            .whitespace_nowrap()
            .text_color(fg)
            .when_some(bg, |el, bg| el.bg(bg))
            .when_some(border, |el, border| el.border_1().border_color(border))
            .map(|el| match self.half {
                None if pictured => el.rounded_full(),
                None => el.rounded(dp(theme.shape.control)),
                Some(Half::Action) => el.rounded_l(dp(theme.shape.control)).border_r_0(),
                Some(Half::Menu) => el.rounded_r(dp(theme.shape.control)).border_l_0(),
            })
            // The seam, a rule inset from top and bottom: the button's own ink over its fill,
            // or a lighter border on an outline. A hovered half's fill marks the seam, so it
            // steps aside then.
            .when(matches!(self.half, Some(Half::Menu)), |el| {
                let (rule, strength) = match (bg, border) {
                    (Some(_), _) => (fg, 0.2),
                    (None, Some(border)) => (border, 0.6),
                    (None, None) => (theme.input, 0.6),
                };
                let enabled = self.disabled.is_none();
                el.relative().child(
                    div()
                        .absolute()
                        .left_0()
                        .top(dp(5.))
                        .bottom(dp(5.))
                        .w(fixed(1.))
                        .bg(rule)
                        .opacity(strength)
                        .when(enabled, |el| {
                            el.group_hover(DROPDOWN_GROUP, |el| el.opacity(0.))
                        }),
                )
            })
            .when(self.disabled.is_none(), |el| {
                el.map(|el| match pictured {
                    true => el.hover(|el| el.opacity(0.85)),
                    false => el.hover(move |el| el.bg(hover)),
                })
                .active(|el| el.opacity(0.8))
            })
            .when(self.disabled.is_some() && pictured, |el| el.opacity(0.5))
            .when_some(self.icon, |el, kind| {
                el.child((icon_at(kind, icon_size).text_color(fg)).opacity(icon_opacity))
            })
            .children(self.picture)
            .when_some(self.label, |el, label| match shrinks {
                true => el.child(div().min_w_0().truncate().child(label)),
                false => el.child(label),
            })
            .when_some(self.icon_end, |el, kind| {
                el.child((icon_at(kind, icon_size).text_color(fg)).opacity(icon_opacity))
            })
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

/// An outline in `hue`'s text, with an edge of it, tinted under the pointer.
/// The hue of a coloured outline: info's, warning's or success's.
fn coloured_hue(style: ButtonStyle) -> Option<Hue> {
    match style {
        ButtonStyle::Info => Some(Hue::Blue),
        ButtonStyle::Warning => Some(Hue::Warn),
        ButtonStyle::Success => Some(Hue::Safe),
        _ => None,
    }
}

fn outlined(theme: &Theme, hue: Hue) -> (Hsla, Option<Hsla>, Option<Hsla>, Hsla) {
    let edge = Some(theme.outline_edge(hue));
    (theme.hue(hue), None, edge, theme.outline_hover(hue))
}

const DROPDOWN_GROUP: &str = "dropdown-button";

/// `action` joined to a chevron that opens its menu, at the regular size, the only one it has.
/// The chevron, named `menu_label`, is disabled with the action.
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
        tooltip: Some(menu_label.into()),
        half: Some(Half::Menu),
        ..button(
            ElementId::NamedChild(std::sync::Arc::new(action.id.clone()), "menu".into()),
            "",
        )
    };
    let action = Button {
        half: Some(Half::Action),
        small: false,
        medium: false,
        ..action
    };
    h_flex()
        .group(DROPDOWN_GROUP)
        .flex_none()
        .child(action)
        .child(menu.on_click(on_menu))
}
