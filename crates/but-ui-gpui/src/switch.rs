//! An on/off switch, as but-ui-react's `Switch`.

use gpui::{
    App, ClickEvent, Div, ElementId, Hsla, Role, SharedString, Stateful, Toggled, Window, div,
    prelude::*,
};

use crate::theme::Theme;
use crate::*;

/// Critically damped, most of the way in about 0.1s, as but-ui-react's Switch moves in
/// `--but-transition-medium`. Unlike `with_animation`, a spring starts at its target when first
/// drawn, so opening a page moves nothing, and a click mid-slide turns it back from where it is.
const SWITCH_SPRING: gpui::SpringConfig = gpui::SpringConfig::new(1600., 80., 1.);

/// Settles once the knob is a tenth of a pixel from its end.
pub fn switch_spring(on: bool) -> gpui::SpringAnimation<bool> {
    gpui::SpringAnimation::new(SWITCH_SPRING)
        .to(on)
        .with_epsilon(0.01)
}

/// A switch's ground off and on, to mix in sRGB as CSS does: mixing `Hsla` would sweep the
/// gray's hue round to pop's and tint the middle.
pub fn switch_grounds(theme: &Theme) -> [gpui::Rgba; 2] {
    [theme.border_2.into(), theme.pop_bg.into()]
}

pub struct Switch {
    id: ElementId,
    on: bool,
    el: Stateful<Div>,
    grounds: [gpui::Rgba; 2],
    knob: Hsla,
    disabled: bool,
}

/// An on/off switch, for a setting that applies at once.
pub fn switch(id: impl Into<ElementId>, on: bool, theme: &Theme) -> Switch {
    let id = id.into();
    Switch {
        el: tab_stop(switch_track(id.clone(), on), theme).hover(|el| el.opacity(0.9)),
        id,
        on,
        grounds: switch_grounds(theme),
        knob: theme.bg_1,
        disabled: false,
    }
}

fn switch_track(id: ElementId, on: bool) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::Switch)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .flex_none()
        .w(dp(30.))
        .h(dp(18.))
        .p(dp(2.))
        .rounded_full()
}

impl Switch {
    /// Dimmed, out of the tab order and taking no clicks, with `why` in its tooltip.
    pub fn disabled_because(mut self, why: Option<impl Into<SharedString>>) -> Self {
        if let Some(why) = why {
            self.el = switch_track(self.id.clone(), self.on)
                .opacity(0.5)
                .tooltip(Tooltip::text(why));
            self.disabled = true;
        }
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        if !self.disabled {
            self.el = self.el.on_click(handler);
        }
        self
    }
}

impl IntoElement for Switch {
    type Element = gpui::SpringAnimationElement<Stateful<Div>>;

    fn into_element(self) -> Self::Element {
        use gpui::AnimationExt as _;
        let [off, on] = self.grounds;
        let knob = self.knob;
        // The spring wraps the whole switch, not a child: the track is the tab stop, whose
        // focus ring is drawn over its ground and under its children.
        self.el
            .with_spring(self.id, switch_spring(self.on), move |el, phase| {
                el.bg(phase.interpolate_clamped(off, on)).child(
                    div()
                        .ml(phase.interpolate_clamped(dp(0.), dp(12.)))
                        .size(dp(14.))
                        .rounded_full()
                        .bg(knob),
                )
            })
    }
}
