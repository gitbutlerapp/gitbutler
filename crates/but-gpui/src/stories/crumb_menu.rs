use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{Icon, crumb_menu_button, h_flex, icon_button};

pub struct CrumbMenuStory;

impl Render for CrumbMenuStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        // The window's layout control: the layout's glyph, then the ⌃⌄ that picks another.
        let layout = |glyph: Icon, id: &'static str| {
            h_flex()
                .child(icon_button(id, glyph).tooltip("Hide sidebar"))
                .child(crumb_menu_button(("layout-menu", glyph as usize), "Layout"))
        };
        page([
            section(
                "States",
                theme,
                [
                    variant("Resting", theme, crumb_menu_button("resting", "Menu")),
                    variant(
                        "Its menu open",
                        theme,
                        crumb_menu_button("open", "Menu").open(true),
                    ),
                ],
            ),
            section(
                "Layout control",
                theme,
                [
                    variant("Both", theme, layout(Icon::Sidebar, "both")),
                    variant("Sidebar only", theme, layout(Icon::SidebarOnly, "sidebar")),
                    variant(
                        "Preview only",
                        theme,
                        layout(Icon::SidebarNarrow, "preview"),
                    ),
                    variant("Sidebar on top", theme, layout(Icon::SidebarTop, "top")),
                ],
            ),
        ])
    }
}
