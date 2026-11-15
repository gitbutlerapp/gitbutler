//! Buttons, as but-ui-react's `Button` and `DropdownButton`.

use gpui::{
    Action, AnyElement, App, ClickEvent, Div, ElementId, Hsla, Rems, Role, SharedString, Window,
    div, prelude::*,
};

use crate::theme::{Hue, Theme, tokens};
use crate::*;

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
    /// Sized by the caller, as a person's picture, in place of an icon: then it is the control
    /// itself, round, and dims under the pointer.
    picture: Option<AnyElement>,
    style: ButtonStyle,
    /// Why it can't be pressed now, shown in place of its tooltip.
    disabled: Option<SharedString>,
    small: bool,
    /// At least this wide, its label centred.
    min_width: Option<Rems>,
    /// On an inverted ground, like a selected row.
    inverted: bool,
    /// The colour of the ground it sits on, as a banner's action takes its banner's.
    tint: Option<Hue>,
    on_click: Option<ClickHandler>,
    tooltip: Option<SharedString>,
    tooltip_key: Option<SharedString>,
    /// The action whose keys the tooltip shows when it names none.
    tooltip_action: Option<Box<dyn Action>>,
    /// Which half of a [`dropdown_button`] it is, squared where the halves meet.
    half: Option<Half>,
    /// Narrower than its label when its row has no room to spare: see [`Button::shrinks`].
    shrinks: bool,
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
        picture: None,
        style: ButtonStyle::Outline,
        disabled: None,
        small: false,
        min_width: None,
        inverted: false,
        tint: None,
        on_click: None,
        tooltip: None,
        tooltip_key: None,
        tooltip_action: None,
        half: None,
        shrinks: false,
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

/// A picture as the control itself, as the account's avatar is: round, with no button around
/// it, dimming under the pointer, its focus ring circling it. As with an icon, its tooltip
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

    /// Narrower than its label when its row has no room to spare, which it then cuts short with
    /// an ellipsis, as a long branch name in it would otherwise push its row past its edge.
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
            ButtonStyle::Ghost => (theme.text_1, None, None, theme.hover),
            ButtonStyle::Outline => (theme.text_1, None, Some(theme.control_border), theme.hover),
            ButtonStyle::Gray => (theme.text_1, Some(theme.muted), None, theme.muted_hover),
            ButtonStyle::Pop => (theme.on_pop, Some(theme.pop_button), None, theme.pop_hover),
            ButtonStyle::Danger => (
                theme.danger,
                Some(theme.danger_soft),
                None,
                theme.danger_soft_hover,
            ),
        };
        // A filled button keeps its fill under the pointer, a step stronger; one without
        // takes the fill there.
        let (fg, bg, hover) = match self.tint {
            Some(tint) => (
                theme.hue(tint),
                bg.map(|_| theme.soft(tint)),
                match bg {
                    Some(_) => theme.soft_hover(tint),
                    None => theme.soft(tint),
                },
            ),
            None if self.inverted && self.style != ButtonStyle::Pop => (
                theme.text_inverted,
                bg.map(|_| theme.inverse_soft),
                match bg {
                    Some(_) => theme.inverse_soft_hover,
                    None => theme.hover_on_solid,
                },
            ),
            None => (fg, bg, hover),
        };
        // but-ui's button tokens (`packages/but-ui/components/button.tokens.json`), as
        // but-ui-react reads them. The heights are `size-control(-sm)`, which `theme.sizes` moves
        // with density, as it does the padding beside a label. An icon alone is 2px wider than
        // tall when small, and square otherwise.
        let sizes = theme.sizes;
        let (height, width, gap, padding, icon_size) = match self.small {
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
        let pictured = self.picture.is_some();
        let shrinks = self.shrinks;
        // What a screen reader calls it: its text, or for an icon its tooltip.
        let name = self.label.clone().or_else(|| self.tooltip.clone());
        h_flex()
            .id(self.id)
            .role(Role::Button)
            .when_some(name, |el, name| el.aria_label(name))
            .map(|el| match shrinks {
                true => el.min_w_0(),
                false => el.flex_none(),
            })
            // A picture is the control itself, with room around it for the focus ring.
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
            .text_size(dp(if self.small { 12. } else { 13. }))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .text_color(fg)
            .when_some(bg, |el, bg| el.bg(bg))
            .when_some(border, |el, border| el.border_1().border_color(border))
            // The halves meet square, split by a seam: an outline's border, or a faint rule
            // across a fill.
            .map(|el| match self.half {
                None if pictured => el.rounded_full(),
                None => el.rounded(dp(theme.shape.control)),
                Some(Half::Action) => el.rounded_l(dp(theme.shape.control)).border_r_0(),
                Some(Half::Menu) => (el.rounded_r(dp(theme.shape.control)).border_l_1())
                    .border_color(border.unwrap_or(theme.bg_1)),
            })
            .when(self.disabled.is_none(), |el| {
                // A picture dims under the pointer rather than sitting in a ground.
                el.map(|el| match pictured {
                    true => el.hover(|el| el.opacity(0.85)),
                    false => el.hover(move |el| el.bg(hover)),
                })
                .active(|el| el.opacity(0.8))
            })
            .when(self.disabled.is_some(), |el| el.opacity(0.5))
            .when_some(self.icon, |el, kind| {
                let opacity = if has_label { 0.7 } else { 0.8 };
                el.child((icon_at(kind, icon_size).text_color(fg)).opacity(opacity))
            })
            .children(self.picture)
            .when_some(self.label, |el, label| match shrinks {
                true => el.child(div().min_w_0().truncate().child(label)),
                false => el.child(label),
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

/// `action` joined to a chevron that opens its menu, as but-ui-react's `DropdownButton`: one
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
