use gpui::{Context, Window, div, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{dp, select};

/// What a select shows closed, as but-ui-react's `Select` trigger; the list it opens is the app's,
/// which these don't.
pub struct SelectStory;

impl Render for SelectStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page([section(
            "Closed",
            theme,
            [
                variant("Short", theme, select("short", "Unified", theme)),
                variant(
                    "Long, cut off in a narrow place",
                    theme,
                    div().w(dp(160.)).child(select(
                        "long",
                        "origin/feature/a-branch-with-a-long-name",
                        theme,
                    )),
                ),
                // Where its ground shows, as on a settings panel: a field, as a text field is.
                variant(
                    "On a recessed ground",
                    theme,
                    div()
                        .p(dp(12.))
                        .rounded(dp(theme.shape.card))
                        .bg(theme.bg_2)
                        .child(select("recessed", "Unified", theme)),
                ),
            ],
        )])
    }
}
