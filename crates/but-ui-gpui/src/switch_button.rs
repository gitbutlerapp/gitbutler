//! A switch and its label in a button's frame, as but-ui-react's `SwitchButton`.

use gpui::{Action, App, ClickEvent, ElementId, Role, SharedString, Toggled, Window, prelude::*};

use crate::theme::Theme;
use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchButtonStyle {
    /// Inside something that already frames it.
    Ghost,
    /// On open ground.
    Outline,
}

#[derive(IntoElement)]
pub struct SwitchButton {
    id: ElementId,
    label: SharedString,
    on: bool,
    style: SwitchButtonStyle,
    disabled: Option<SharedString>,
    tooltip: Option<SharedString>,
    tooltip_action: Option<Box<dyn Action>>,
    on_click: Option<ClickHandler>,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The pair reads and presses as one control: a click anywhere on it, label included, toggles.
pub fn switch_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> SwitchButton {
    SwitchButton {
        id: id.into(),
        label: label.into(),
        on,
        style: SwitchButtonStyle::Ghost,
        disabled: None,
        tooltip: None,
        tooltip_action: None,
        on_click: None,
    }
}

impl SwitchButton {
    pub fn style(mut self, style: SwitchButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Unpressable, with `why` as its tooltip; `None` keeps an earlier reason.
    pub fn disabled_because(mut self, why: Option<impl Into<SharedString>>) -> Self {
        self.disabled = why.map(Into::into).or(self.disabled);
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
}

impl RenderOnce for SwitchButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx);
        let enabled = self.disabled.is_none();
        let switch = switch_mark(
            ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "switch".into()),
            self.on,
            !enabled,
            theme,
        );
        // Disabled, it fades as one picture, as React's does, in solid colours: GPUI's opacity
        // fades each layer on its own and would show the track through the knob.
        let fade = |color| match enabled {
            true => color,
            false => Theme::disabled_on(color, theme.background),
        };
        // A regular button's height and type, with the roomier padding and gap React gives it.
        h_flex()
            .id(self.id)
            .role(Role::Switch)
            .aria_label(self.label.clone())
            .aria_toggled(if self.on {
                Toggled::True
            } else {
                Toggled::False
            })
            .flex_none()
            .h(dp(theme.sizes.control))
            // An outline's border is part of its 10, as in React.
            .px(dp(match self.style {
                SwitchButtonStyle::Ghost => 10.,
                SwitchButtonStyle::Outline => 9.,
            }))
            .gap(dp(8.))
            .rounded(dp(theme.shape.control))
            .text_13()
            .semibold()
            .whitespace_nowrap()
            .text_color(fade(theme.foreground))
            .when(self.style == SwitchButtonStyle::Outline, |el| {
                el.border_1().border_color(fade(theme.input))
            })
            .when(enabled, |el| {
                el.hover(|el| el.bg(theme.hover))
                    .active(|el| el.opacity(0.8))
            })
            .child(switch)
            .child(self.label)
            .when_some(self.disabled.clone().or(self.tooltip), |el, tooltip| {
                let action = self.tooltip_action.filter(|_| enabled);
                el.tooltip(move |window, cx| {
                    let key = action.as_deref().and_then(|action| shortcut(action, cx));
                    Tooltip::keyed(tooltip.clone(), key)(window, cx)
                })
            })
            .when_some(self.on_click.filter(|_| enabled), |el, handler| {
                tab_stop(el, theme).on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
    }
}
