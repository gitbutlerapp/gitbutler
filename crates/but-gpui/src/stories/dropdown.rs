use gpui::{Context, Window, div, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{dp, dropdown};

/// What a dropdown shows closed; the menu it opens is the app's, which these don't.
pub struct DropdownStory;

impl Render for DropdownStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page([section(
            "Closed",
            theme,
            [
                variant("Short", theme, dropdown("short", "Unified", theme)),
                variant(
                    "Long, cut off in a narrow place",
                    theme,
                    div().w(dp(160.)).child(dropdown(
                        "long",
                        "origin/feature/a-branch-with-a-long-name",
                        theme,
                    )),
                ),
            ],
        )])
    }
}
