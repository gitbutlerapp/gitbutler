//! A closed choice from a short list, as but-ui-react's `Select`.

use gpui::{Div, ElementId, Role, SharedString, Stateful, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// The trigger only: the list it opens is the caller's. Unlike `Select`, it's as wide as its
/// choice rather than its field.
pub fn select(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    tab_stop(h_flex().id(id), theme)
        .role(Role::ComboBox)
        .flex_none()
        .h(dp(theme.sizes.control))
        .px(dp(8. + theme.sizes.padding))
        .gap(dp(8.))
        .rounded(dp(theme.shape.control))
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .text_13()
        .text_color(theme.foreground)
        .hover(|el| el.border_color(theme.border_hover))
        .child(div().min_w_0().truncate().child(label.into()))
        .child(
            icon(Icon::ChevronDown)
                .text_color(theme.foreground)
                .opacity(0.7),
        )
}
