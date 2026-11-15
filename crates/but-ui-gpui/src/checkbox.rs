//! A checkbox, as but-ui-react's `Checkbox`.

use gpui::{Div, ElementId, Role, Stateful, Toggled, div, prelude::*};

use crate::theme::Theme;
use crate::*;

pub fn checkbox(id: impl Into<ElementId>, check: Toggled, theme: &Theme) -> Stateful<Div> {
    let on = check != Toggled::False;
    div()
        .id(id)
        .role(Role::CheckBox)
        .aria_toggled(check)
        .flex_none()
        .size(dp(14.))
        .rounded(dp(theme.shape.xs))
        .border_1()
        .border_color(if on { theme.pop_bg } else { theme.border_2 })
        .when(on, |el| el.bg(theme.pop_bg))
        .flex()
        .items_center()
        .justify_center()
        .map(|el| match check {
            Toggled::True => el.child(icon_at(Icon::Tick, 10.).text_color(theme.on_pop)),
            Toggled::Mixed => el.child(div().w(dp(8.)).h(dp(2.)).rounded(dp(1.)).bg(theme.on_pop)),
            Toggled::False => el,
        })
}
