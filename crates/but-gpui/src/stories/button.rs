use gpui::{AnyElement, Context, Window, div, prelude::*};

use super::{Example, example, page, section, variant};
use crate::theme::Theme;
use crate::ui::{Button, ButtonStyle, Icon, button, dp, h_flex, icon_button, v_flex};

const STYLES: [(&str, ButtonStyle); 8] = [
    ("Ghost", ButtonStyle::Ghost),
    ("Outline", ButtonStyle::Outline),
    ("Gray", ButtonStyle::Gray),
    ("Pop", ButtonStyle::Pop),
    ("Danger", ButtonStyle::Danger),
    ("Info", ButtonStyle::Info),
    ("Warning", ButtonStyle::Warning),
    ("Success", ButtonStyle::Success),
];

/// The stories of `Button.stories.tsx` this client can draw. It has no solid danger, no medium
/// size and no keys in a button, so `Danger`, `WithKbd` and `DenseHeader` aren't here.
pub(super) const EXAMPLES: [Example; 6] = [
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
        id: "components-button--colours",
        name: "Colours",
        draw: |_| {
            h_flex()
                .gap(dp(12.))
                .items_center()
                .children(
                    [
                        button("colour-push", "8")
                            .icon(Icon::ArrowUp)
                            .style(ButtonStyle::Info)
                            .small()
                            .tooltip("Push 8 commits"),
                        button("colour-update", "3")
                            .icon(Icon::ArrowDown)
                            .style(ButtonStyle::Warning)
                            .small()
                            .tooltip("Update 3 commits"),
                        button("colour-merge-small", "Merge")
                            .style(ButtonStyle::Success)
                            .small(),
                        button("colour-push-regular", "Push").style(ButtonStyle::Info),
                        button("colour-update-regular", "Update").style(ButtonStyle::Warning),
                        button("colour-merge-regular", "Merge").style(ButtonStyle::Success),
                    ]
                    .map(pressable),
                )
                .into_any_element()
        },
    },
    Example {
        id: "components-button--colours-on-a-selection",
        name: "Colours On A Selection",
        draw: |theme| {
            h_flex()
                .gap(dp(8.))
                .items_center()
                .p(dp(8.))
                .rounded(dp(8.))
                .bg(theme.inverse)
                .children(
                    [
                        button("selected-push", "8")
                            .icon(Icon::ArrowUp)
                            .style(ButtonStyle::Info)
                            .tooltip("Push 8 commits"),
                        button("selected-update", "3")
                            .icon(Icon::ArrowDown)
                            .style(ButtonStyle::Warning)
                            .tooltip("Update 3 commits"),
                        button("selected-merge", "Merge").style(ButtonStyle::Success),
                        icon_button("selected-hide", Icon::Eye)
                            .style(ButtonStyle::Outline)
                            .tooltip("Hide project"),
                    ]
                    .map(|button| pressable(button.small().inverted(true))),
                )
                .into_any_element()
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
    Example {
        id: "components-button--with-icon-start-and-end",
        name: "With Icon Start And End",
        draw: |_| {
            v_flex()
                .gap(dp(12.))
                .child(
                    h_flex()
                        .gap(dp(12.))
                        .child(pressable(
                            button("new-branch", "New branch").icon(Icon::Plus),
                        ))
                        .child(pressable(
                            button("refresh", "Refresh")
                                .style(ButtonStyle::Ghost)
                                .icon(Icon::Refresh),
                        )),
                )
                .child(
                    h_flex()
                        .gap(dp(12.))
                        .child(pressable(
                            button("sort", "Sort by date").icon_end(Icon::ChevronDown),
                        ))
                        .child(pressable(
                            button("continue", "Continue")
                                .style(ButtonStyle::Pop)
                                .icon_end(Icon::ArrowRight),
                        ))
                        .child(pressable(
                            button("open", "Open on GitHub")
                                .style(ButtonStyle::Ghost)
                                .icon_end(Icon::ArrowUpRight),
                        )),
                )
                .child(
                    h_flex().child(pressable(
                        button("filter", "Author")
                            .icon(Icon::Filter)
                            .icon_end(Icon::ChevronDown),
                    )),
                )
                .into_any_element()
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
                        true => (div().p(dp(8.)).rounded(dp(8.)).bg(theme.inverse))
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
                        div().p(dp(6.)).rounded(dp(4.)).bg(theme.inverse).child(
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
