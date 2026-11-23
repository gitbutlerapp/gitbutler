use gpui::{Context, Window, div, prelude::*};

use super::{framed, page, section, variant};
use crate::theme::Theme;
use crate::ui::{
    MetaCountType, age_column, count_badge, dot, dp, list_row, meta_count, now, unread_line, v_flex,
};

pub struct CountsStory;

impl Render for CountsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let ago = |seconds: i64| Some(now() - seconds);
        page([
            section(
                "Badge",
                theme,
                [1, 12, 128].map(|n| variant(n.to_string(), theme, count_badge(n, theme))),
            ),
            section(
                "In a row's meta, saying on hover what they count",
                theme,
                [
                    variant(
                        "Unpushed",
                        theme,
                        meta_count(MetaCountType::Unpushed, Some("3".into()), theme),
                    ),
                    variant(
                        "Waiting on you",
                        theme,
                        meta_count(MetaCountType::Conflicts, Some("2".into()), theme),
                    ),
                ],
            ),
            section(
                "Dots",
                theme,
                [
                    variant("New work", theme, dot(theme.primary_text)),
                    variant("Running", theme, dot(theme.warning_text)),
                    variant("Failed", theme, dot(theme.danger_text)),
                ],
            ),
            section(
                "Age, in a slot as wide as the longest",
                theme,
                [
                    ("Just now", ago(20)),
                    ("An hour", ago(3600)),
                    ("Days", ago(3 * 86400)),
                    ("Over a year", ago(400 * 86400)),
                    ("No time", None),
                ]
                .map(|(caption, at)| variant(caption, theme, age_column(at, theme))),
            ),
            section(
                "Unread",
                theme,
                [framed(
                    "A hairline in the row's padding, moving nothing",
                    theme,
                    v_flex().w(dp(240.)).py(dp(4.)).bg(theme.canvas).child(
                        list_row("unread", false, theme)
                            .relative()
                            .child(unread_line(theme.primary_text))
                            .child(div().flex_1().child("Agent's new commit")),
                    ),
                )],
            ),
        ])
    }
}
