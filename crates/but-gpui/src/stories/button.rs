use gpui::{Context, Window, div, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{Button, ButtonStyle, Icon, button, dp, dropdown_button, h_flex, icon_button};

const STYLES: [(&str, ButtonStyle); 5] = [
    ("Ghost", ButtonStyle::Ghost),
    ("Outline", ButtonStyle::Outline),
    ("Gray", ButtonStyle::Gray),
    ("Pop", ButtonStyle::Pop),
    ("Danger", ButtonStyle::Danger),
];

pub struct ButtonStory;

impl Render for ButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        // Pressable, so they show their hover and press as the app's do.
        let styled = |id: &'static str, ix: usize, (name, style): (&'static str, ButtonStyle)| {
            button((id, ix), name).style(style).on_click(|_, _, _| {})
        };
        // Each style, as `variant` shows the button `build` makes of it.
        let each = |build: &dyn Fn(usize, (&'static str, ButtonStyle)) -> Button| {
            (STYLES.into_iter().enumerate())
                .map(|(ix, style)| variant(style.0, theme, build(ix, style)))
                .collect::<Vec<_>>()
        };
        page([
            section(
                "Styles",
                theme,
                each(&|ix, style| styled("style", ix, style)),
            ),
            section(
                "Small",
                theme,
                each(&|ix, style| styled("small", ix, style).small()),
            ),
            section(
                "Disabled, saying why in the tooltip",
                theme,
                each(&|ix, style| {
                    styled("disabled", ix, style).disabled_because(Some("Nothing to do"))
                }),
            ),
            section(
                "With an icon",
                theme,
                [
                    variant(
                        "Outline",
                        theme,
                        button("icon-outline", "New branch").icon(Icon::Branch),
                    ),
                    variant(
                        "Pop",
                        theme,
                        (button("icon-pop", "Commit").icon(Icon::Commit)).style(ButtonStyle::Pop),
                    ),
                    variant(
                        "Danger",
                        theme,
                        (button("icon-danger", "Discard").icon(Icon::Bin))
                            .style(ButtonStyle::Danger),
                    ),
                ],
            ),
            section(
                "Icon only, named by its tooltip",
                theme,
                [
                    variant(
                        "Ghost",
                        theme,
                        icon_button("only-ghost", Icon::Kebab).tooltip("More"),
                    ),
                    variant(
                        "Outline",
                        theme,
                        (icon_button("only-outline", Icon::Plus).tooltip("Add"))
                            .style(ButtonStyle::Outline),
                    ),
                    variant(
                        "Small",
                        theme,
                        (icon_button("only-small", Icon::Settings).tooltip("Settings")).small(),
                    ),
                ],
            ),
            section(
                "Split",
                theme,
                [
                    variant(
                        "Outline",
                        theme,
                        dropdown_button(
                            button("split-outline", "Fetch"),
                            "Fetch options",
                            |_, _, _| {},
                        ),
                    ),
                    variant(
                        "Pop",
                        theme,
                        dropdown_button(
                            button("split-pop", "Push").style(ButtonStyle::Pop),
                            "Push options",
                            |_, _, _| {},
                        ),
                    ),
                ],
            ),
            section(
                "Inverted, on a selected row",
                theme,
                [variant(
                    "Ghost, Outline and Gray",
                    theme,
                    div().p(dp(6.)).rounded(dp(4.)).bg(theme.fill_gray).child(
                        h_flex().gap(dp(6.)).children(
                            (STYLES.into_iter().enumerate().take(3)).map(|(ix, style)| {
                                styled("inverted", ix, style).small().inverted(true)
                            }),
                        ),
                    ),
                )],
            ),
        ])
    }
}
