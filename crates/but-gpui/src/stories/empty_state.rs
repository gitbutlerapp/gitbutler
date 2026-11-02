use gpui::{AnyElement, Context, Window, div, prelude::*};

use super::{framed, page, section};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::{dp, empty_state, loading_state};

pub struct EmptyStateStory;

impl Render for EmptyStateStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let drawings = [
            (
                Illustration::Cactus,
                "No branches yet",
                "Create one to start working",
            ),
            (Illustration::Papers, "Nothing found", "Try other words"),
            (
                Illustration::Terminal,
                "No terminals",
                "Open one to run a command here",
            ),
            (Illustration::Waving, "All caught up", ""),
        ];
        page([
            section(
                "With a drawing, picked by what it means",
                theme,
                drawings.map(|(drawing, title, detail)| {
                    let caption = format!("{drawing:?}");
                    let state = empty_state(Some(drawing), title, detail, theme, cx);
                    framed(caption, theme, pane(theme, state))
                }),
            ),
            section(
                "Without one",
                theme,
                [
                    framed(
                        "A title and a line",
                        theme,
                        pane(
                            theme,
                            empty_state(
                                None,
                                "No changes",
                                "Edit a file to see it here",
                                theme,
                                cx,
                            ),
                        ),
                    ),
                    framed(
                        "Loading: blank, then fading in",
                        theme,
                        pane(theme, loading_state("Loading branches", theme, cx)),
                    ),
                ],
            ),
        ])
    }
}

/// A pane for a state to fill, as tall as the waving drawing needs.
fn pane(theme: &Theme, state: AnyElement) -> impl IntoElement {
    div()
        .w(dp(300.))
        .h(dp(340.))
        .rounded(dp(6.))
        .border_1()
        .border_color(theme.border_3)
        .child(state)
}
