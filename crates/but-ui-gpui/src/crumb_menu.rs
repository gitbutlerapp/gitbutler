//! The ⌃⌄ that opens a menu beside a level, as but-ui-react's `CrumbMenuButton`
//! (`Breadcrumbs.tsx`).

use gpui::{App, ClickEvent, ElementId, Role, SharedString, Window, prelude::*};

use crate::theme::Theme;
use crate::*;

const GROUP: &str = "crumb-menu";

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// 18×22, its glyph quiet until the pointer is on it or its menu is open, laid out at the 18px
/// of the line it ends. The trigger for a menu beside words or a glyph that read like a level,
/// as a location bar's layout control. `label` names it to screen readers.
#[derive(IntoElement)]
pub struct CrumbMenuButton {
    id: ElementId,
    label: SharedString,
    open: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

pub fn crumb_menu_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
) -> CrumbMenuButton {
    CrumbMenuButton {
        id: id.into(),
        label: label.into(),
        open: false,
        tooltip: None,
        on_click: None,
    }
}

impl CrumbMenuButton {
    /// While its menu is open: lit as under the pointer, as React's `[data-popup-open]`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
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

impl RenderOnce for CrumbMenuButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx);
        let open = self.open;
        let glyph = icon_at(Icon::ChevronUpDown, 10.)
            .text_color(theme.muted_foreground)
            .opacity(if open { 1. } else { 0.7 })
            .group_hover(GROUP, |el| el.opacity(1.));
        h_flex()
            .id(self.id)
            .group(GROUP)
            .role(Role::Button)
            .aria_label(self.label)
            .flex_none()
            .w(dp(18.))
            .h(dp(22.))
            .my(dp(-2.))
            .justify_center()
            .rounded(dp(theme.shape.control))
            .when(open, |el| el.bg(theme.hover))
            .hover(|el| el.bg(theme.hover))
            .child(glyph)
            .when_some(self.tooltip, |el, tooltip| {
                el.tooltip(move |window, cx| Tooltip::keyed(tooltip.clone(), None)(window, cx))
            })
            .when_some(self.on_click, |el, handler| {
                tab_stop(el, theme).on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
    }
}
