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
    [theme.border.into(), theme.primary_mark.into()]
}

pub struct Switch {
    id: ElementId,
    on: bool,
    large: bool,
    el: Stateful<Div>,
    colors: Colors,
    disabled: bool,
}

/// The track's ground off and on, its knob, and the page under them.
#[derive(Clone, Copy)]
struct Colors {
    grounds: [gpui::Rgba; 2],
    knob: Hsla,
    page: Hsla,
}

impl Colors {
    fn of(theme: &Theme) -> Self {
        Colors {
            grounds: switch_grounds(theme),
            knob: theme.background,
            page: theme.background,
        }
    }

    /// Faded as one picture, as React's `opacity: 50%` does: solid colours, so the knob doesn't
    /// let the track show through it as it would under GPUI's opacity, which fades each layer.
    fn disabled(self) -> Self {
        let fade = |color: Hsla| Theme::disabled_on(color, self.page);
        Colors {
            grounds: self.grounds.map(|ground| fade(ground.into()).into()),
            knob: fade(self.knob),
            page: self.page,
        }
    }
}

/// An on/off switch, for a setting that applies at once.
pub fn switch(id: impl Into<ElementId>, on: bool, theme: &Theme) -> Switch {
    let id = id.into();
    let el = h_flex()
        .id(id.clone())
        .role(Role::Switch)
        .aria_toggled(if on { Toggled::True } else { Toggled::False });
    Switch {
        el: tab_stop(el, theme).hover(|el| el.opacity(0.9)),
        id,
        on,
        large: false,
        colors: Colors::of(theme),
        disabled: false,
    }
}

impl Switch {
    /// The size a settings row wears, where the switch is the row's only control.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// Dimmed, out of the tab order and taking no clicks, with `why` in its tooltip.
    pub fn disabled_because(mut self, why: Option<impl Into<SharedString>>) -> Self {
        if let Some(why) = why {
            self.el = h_flex()
                .id(self.id.clone())
                .role(Role::Switch)
                .aria_toggled(if self.on {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .tooltip(Tooltip::text(why));
            self.colors = self.colors.disabled();
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
        let height = if self.large { 16. } else { 14. };
        track(self.el, self.id, self.on, height, self.colors)
    }
}

/// A switch drawn for a control that presses it, as [`switch_button`] is: the default size,
/// with no role, focus or clicks of its own.
pub(crate) fn switch_mark(
    id: ElementId,
    on: bool,
    disabled: bool,
    theme: &Theme,
) -> gpui::SpringAnimationElement<Stateful<Div>> {
    let colors = match disabled {
        true => Colors::of(theme).disabled(),
        false => Colors::of(theme),
    };
    track(h_flex().id(id.clone()), id, on, 14., colors)
}

/// The track and its knob, `height` tall: React's two sizes are 24×14 and 28×16, the knob 4
/// smaller than the track and travelling to 2 from its far end.
fn track(
    el: Stateful<Div>,
    id: ElementId,
    on: bool,
    height: f32,
    colors: Colors,
) -> gpui::SpringAnimationElement<Stateful<Div>> {
    use gpui::AnimationExt as _;
    let width = height + 10.;
    let knob = height - 4.;
    let travel = width - height;
    let Colors {
        grounds: [off, on_ground],
        knob: knob_color,
        ..
    } = colors;
    // The spring wraps the whole switch, not a child: the track is the tab stop, whose
    // focus ring is drawn over its ground and under its children.
    el.flex_none()
        .w(dp(width))
        .h(dp(height))
        .p(dp(2.))
        .rounded_full()
        .with_spring(id, switch_spring(on), move |el, phase| {
            el.bg(phase.interpolate_clamped(off, on_ground)).child(
                div()
                    .ml(phase.interpolate_clamped(dp(0.), dp(travel)))
                    .size(dp(knob))
                    .rounded_full()
                    .bg(knob_color),
            )
        })
}
