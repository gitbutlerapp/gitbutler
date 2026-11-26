//! The window's layout control, as 🍑 But UI's location bar draws it: a glyph that shows or hides
//! the navigator, then a ⌃⌄ that picks a layout.

use gpui::{App, ClickEvent, Div, Window, prelude::*};

use crate::app::{ToggleDetail, ToggleNavigator};
use crate::overlays::MenuItem;
use crate::theme::Theme;
use crate::ui::{Icon, above_drag, crumb_menu_button, h_flex, icon_button};

/// Which of the window's panes show: both, the navigator alone, the detail alone, or the
/// navigator across the top. The navigator and the detail can't both go, so it's a choice of
/// four rather than two switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout {
    Both,
    Navigator,
    Detail,
    Top,
}

impl Layout {
    /// The control's glyph: the layout the window is in.
    fn glyph(self) -> Icon {
        match self {
            Layout::Both => Icon::Sidebar,
            Layout::Navigator => Icon::SidebarOnly,
            Layout::Detail => Icon::SidebarNarrow,
            Layout::Top => Icon::SidebarTop,
        }
    }
}

/// The layouts to pick from, `current` ticked. A side's keys are those of hiding the other.
pub(crate) fn layout_items(current: Layout, cx: &App) -> Vec<MenuItem> {
    let pick = |to: Layout| {
        move |app: &mut crate::app::AppView,
              window: &mut Window,
              cx: &mut gpui::Context<crate::app::AppView>| {
            app.set_layout(to, window, cx)
        }
    };
    vec![
        MenuItem::new("Show both", pick(Layout::Both))
            .icon(Icon::SidebarPreviewPane)
            .under("Layout")
            .ticked(current == Layout::Both),
        MenuItem::keyed("Sidebar", &ToggleDetail, cx, pick(Layout::Navigator))
            .icon(Icon::Sidebar)
            .ticked(current == Layout::Navigator),
        MenuItem::keyed("Preview", &ToggleNavigator, cx, pick(Layout::Detail))
            .icon(Icon::PreviewPane)
            .ticked(current == Layout::Detail),
        MenuItem::new("Sidebar on top", pick(Layout::Top))
            .icon(Icon::SidebarTop)
            .separated()
            .ticked(current == Layout::Top),
    ]
}

/// The glyph, which shows or hides the navigator as ⌘B does, then the ⌃⌄, which runs `menu` to
/// open [`layout_items`]. Across the top, the navigator has nothing to show or hide.
pub(crate) fn layout_control(
    current: Layout,
    menu: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let tip = match current {
        Layout::Detail => "Show sidebar",
        _ => "Hide sidebar",
    };
    let toggle = icon_button("layout-toggle", current.glyph())
        .tooltip_for(tip, &ToggleNavigator)
        .disabled_because((current == Layout::Top).then_some("The sidebar is across the top"))
        .dispatch(&ToggleNavigator);
    let menu = crumb_menu_button("layout-menu", "Layout")
        .tooltip("Layout")
        .on_click(menu);
    h_flex()
        .flex_none()
        .child(above_drag(gpui::div().child(toggle)))
        .child(above_drag(gpui::div().child(menu)))
}

/// The hairline between the header's groups, 18px tall, as the navigator's against the bar.
pub(crate) fn bar_rule(theme: &Theme) -> Div {
    gpui::div()
        .flex_none()
        .w(crate::ui::fixed(1.))
        .h(crate::ui::dp(18.))
        .bg(theme.border)
}
