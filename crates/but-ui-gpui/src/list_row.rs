//! A row of a list or menu, as but-ui-react's `NavItem`.

use gpui::{Div, ElementId, Role, Stateful, prelude::*};

use crate::theme::Theme;
use crate::*;

pub fn list_row(id: impl Into<ElementId>, selected: bool, theme: &Theme) -> Stateful<Div> {
    h_flex()
        .id(id)
        .role(Role::ListItem)
        .aria_selected(selected)
        .flex_none()
        .h(dp(theme.sizes.control))
        .mx(dp(6.))
        .px(dp(8.))
        .gap(dp(8.))
        .rounded(dp(theme.shape.control))
        .when(selected, |el| el.bg(theme.selected_unfocused))
        .when(!selected, |el| el.hover(|el| el.bg(theme.hover)))
}
