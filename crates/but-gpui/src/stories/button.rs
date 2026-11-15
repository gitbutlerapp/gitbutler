use gpui::{AnyElement, Context, Window, div, prelude::*};

use super::{Example, example, page, section, variant};
use crate::theme::Theme;
use crate::ui::{Button, ButtonStyle, Icon, button, dp, h_flex, icon_button};

const STYLES: [(&str, ButtonStyle); 5] = [
    ("Ghost", ButtonStyle::Ghost),
    ("Outline", ButtonStyle::Outline),
    ("Gray", ButtonStyle::Gray),
    ("Pop", ButtonStyle::Pop),
    ("Danger", ButtonStyle::Danger),
];

/// The stories of `Button.stories.tsx` this client can draw. It has no coloured outlines, no
/// solid danger, no medium size, no icon after the label and no keys in a button, so `Colours`,
/// `Danger`, `WithIconStartAndEnd`, `WithKbd` and `DenseHeader` aren't here.
pub(super) const EXAMPLES: [Example; 3] = [
    Example {
        id: "components-button--default",
        name: "Default",
        draw: |_| pressable(button("default", "Button").style(ButtonStyle::Pop)).into_any_element(),
    },
    Example {
        id: "components-button--variants",
        name: "Variants",
        draw: |theme| {
            in_order(theme, |ix, style, inverted| {
                pressable(
                    button(("variant", ix), "Button")
                        .style(style)
                        .inverted(inverted),
                )
            })
        },
    },
    Example {
        id: "components-button--icon-only",
        name: "Icon Only",
        draw: |theme| {
            in_order(theme, |ix, style, inverted| {
                let tooltip = format!("{style:?} action");
                pressable(
                    icon_button(("icon-only", ix), Icon::Plus)
                        .style(style)
                        .inverted(inverted),
                )
                .tooltip(tooltip)
            })
        },
    },
];

/// Pressable, so it shows its hover and press as the app's do.
fn pressable(button: Button) -> Button {
    button.on_click(|_, _, _| {})
}

/// A button of each style in the docs site's order, the inverted ones on the inverse ground.
fn in_order(theme: &Theme, build: impl Fn(usize, ButtonStyle, bool) -> Button) -> AnyElement {
    let order = [
        (ButtonStyle::Pop, false),
        (ButtonStyle::Gray, false),
        (ButtonStyle::Outline, false),
        (ButtonStyle::Outline, true),
        (ButtonStyle::Ghost, false),
        (ButtonStyle::Ghost, true),
        (ButtonStyle::Danger, false),
    ];
    h_flex()
        .gap(dp(12.))
        .children(
            order
                .into_iter()
                .enumerate()
                .map(|(ix, (style, inverted))| {
                    let button = build(ix, style, inverted);
                    match inverted {
                        true => (div().p(dp(8.)).rounded(dp(8.)).bg(theme.fill_gray))
                            .child(button)
                            .into_any_element(),
                        false => button.into_any_element(),
                    }
                }),
        )
        .into_any_element()
}

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
        // The docs site's stories first, then what only this client's buttons do.
        page(
            (EXAMPLES.into_iter().map(|e| example(e, theme))).chain([
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
                            (button("icon-pop", "Commit").icon(Icon::Commit))
                                .style(ButtonStyle::Pop),
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
                    "Shrinking, in a row too narrow for them",
                    theme,
                    [variant(
                        "Two that shrink beside one that doesn't",
                        theme,
                        h_flex().w(dp(360.)).gap(dp(6.)).children([
                            (button("shrinks-0", "Leave them out"))
                                .style(ButtonStyle::Gray)
                                .small(),
                            (button(
                                "shrinks-1",
                                "New branch above dp-gpui-hunk-resolution-dialog",
                            ))
                            .style(ButtonStyle::Gray)
                            .small()
                            .shrinks(),
                            (button("shrinks-2", "Commit to dp-gpui-hunk-resolution-dialog"))
                                .style(ButtonStyle::Pop)
                                .small()
                                .shrinks(),
                        ]),
                    )],
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
            ]),
        )
    }
}
