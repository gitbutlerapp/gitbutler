use gpui::{Context, Div, Window, prelude::*};

use super::{framed, page, section};
use crate::theme::Theme;
use crate::ui::{Icon, dp, fold_button, fold_row, guide, icon, joined, list_row, side_row, v_flex};

const FILES: [&str; 3] = ["app.rs", "lib.rs", "theme.rs"];

/// The rows of a side list, on its recessed ground, with a folder to fold.
pub struct RowsStory {
    open: bool,
}

impl Default for RowsStory {
    fn default() -> Self {
        Self { open: true }
    }
}

impl Render for RowsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let row = |id: &'static str, glyph: Icon, name: &'static str, selected, solid| {
            side_row(id, selected, solid, theme)
                .child(icon(glyph).text_color(if selected && solid {
                    theme.text_inverted
                } else {
                    theme.text_2
                }))
                .child(name)
        };
        // Three selected in a run, as one block.
        let run = |solid: bool| {
            list(theme).children(FILES.iter().enumerate().map(|(ix, name)| {
                let row = side_row(("run", ix + usize::from(solid) * 10), true, solid, theme)
                    .child(*name);
                joined(row, (ix > 0, ix + 1 < FILES.len()), solid, theme)
            }))
        };
        let open = self.open;
        let tree = list(theme)
            .child(
                list_row("src", false, theme)
                    .pl(dp(4.))
                    .child(fold_button(
                        "src-fold",
                        Some(open),
                        theme.text_2,
                        cx,
                        |this, _, cx| {
                            this.open = !this.open;
                            cx.notify();
                        },
                    ))
                    .child("src"),
            )
            .when(open, |el| {
                el.children(FILES.iter().enumerate().map(|(ix, name)| {
                    // The guide runs down from the chevron's middle, the folder row's padding
                    // and half the chevron's step in; the names line up with the folder's.
                    list_row(("src-file", ix), false, theme)
                        .relative()
                        .pl(dp(26.))
                        .child(guide(4. + 7., theme))
                        .child(*name)
                }))
            });
        let more = |id: &'static str, glyph: Icon, words: &'static str| {
            fold_row(id, glyph, 14., 36., theme).child(words)
        };
        page([
            section(
                "Rows",
                theme,
                [framed(
                    "Plain, selected, and selected while the list has the keys",
                    theme,
                    list(theme)
                        .child(row("plain", Icon::Branch, "main", false, false))
                        .child(row("selected", Icon::Branch, "feature", true, false))
                        .child(row("solid", Icon::Branch, "fix-typo", true, true)),
                )],
            ),
            section(
                "A run of selected rows",
                theme,
                [
                    framed("Without the keys", theme, run(false)),
                    framed("With the keys", theme, run(true)),
                ],
            ),
            section(
                "Folds",
                theme,
                [
                    framed("A folder to fold, with its guide", theme, tree),
                    framed(
                        "More, or fewer",
                        theme,
                        list(theme)
                            .child(more("more", Icon::Kebab, "3 more"))
                            .child(more("fewer", Icon::ChevronUp, "Show fewer")),
                    ),
                ],
            ),
        ])
    }
}

/// A side list's ground, as wide as the navigator's narrowest.
fn list(theme: &Theme) -> Div {
    v_flex()
        .w(dp(240.))
        .py(dp(6.))
        .rounded(dp(6.))
        .bg(theme.bg_2)
}
