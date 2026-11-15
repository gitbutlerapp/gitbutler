//! A closed choice from a short list, as but-ui-react's `Select`.

use gpui::{Div, ElementId, Role, SharedString, Stateful, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// One choice from a short list, closed, as but-ui-react's `Select` trigger: a field, as [`field`]
/// is, holding the choice in regular weight with a chevron after it. It wears the field's ground,
/// edge and padding, and under the pointer its edge darkens rather than its ground filling. The
/// list it opens is the caller's. It is as wide as its choice, where `Select` takes its field's
/// width.
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
        .border_color(theme.border_2)
        .bg(theme.bg_1)
        .text_size(dp(13.))
        .text_color(theme.text_1)
        .hover(|el| el.border_color(theme.border_1))
        .child(div().min_w_0().truncate().child(label.into()))
        .child(
            icon(Icon::ChevronDown)
                .text_color(theme.text_1)
                .opacity(0.7),
        )
}
