use gpui::{AnyElement, Context, Div, Window, div, prelude::*};

use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::{
    BadgeSize, BadgeVariant, Icon, Text, badge, dp, h_flex, small_icon, spinning, v_flex,
};

/// The stories of `Badge.stories.tsx` this client can draw. Its badges don't turn to the other
/// scheme's colours on a selected row, so `Inverted` isn't here. An icon takes the badge's text
/// colour, which `svg` doesn't inherit, and the spinner is the client's own running dots.
pub(super) const EXAMPLES: [Example; 5] = [
    Example {
        id: "components-badge--default",
        name: "Default",
        draw: |theme| {
            regular(BadgeVariant::FillGray, theme)
                .child("42")
                .into_any_element()
        },
    },
    Example {
        id: "components-badge--all-variants",
        name: "All Variants",
        draw: |theme| {
            row([
                ("fillGray", BadgeVariant::FillGray),
                ("lightGray", BadgeVariant::LightGray),
                ("safe", BadgeVariant::Safe),
                ("warn", BadgeVariant::Warn),
                ("danger", BadgeVariant::Danger),
                ("purple", BadgeVariant::Purple),
                ("blue", BadgeVariant::Blue),
            ]
            .map(|(name, variant)| regular(variant, theme).child(name)))
        },
    },
    Example {
        id: "components-badge--all-sizes",
        name: "All Sizes",
        draw: |theme| {
            row([
                regular(BadgeVariant::FillGray, theme).child("regular"),
                large(BadgeVariant::FillGray, theme).child("large"),
                regular(BadgeVariant::Safe, theme).child("42"),
                large(BadgeVariant::Safe, theme).child("42"),
            ])
        },
    },
    Example {
        id: "components-badge--with-icon",
        name: "With Icon",
        draw: |theme| {
            row([
                regular(BadgeVariant::Safe, theme)
                    .child(small_icon(Icon::Tick).text_color(theme.success_text)),
                regular(BadgeVariant::Danger, theme)
                    .child(small_icon(Icon::Cross).text_color(theme.danger_text)),
                regular(BadgeVariant::LightGray, theme).child(spinning(theme.foreground, true)),
                regular(BadgeVariant::Warn, theme)
                    .child(small_icon(Icon::Warning).text_color(theme.warning_text)),
                (regular(BadgeVariant::Safe, theme))
                    .child(small_icon(Icon::Tick).text_color(theme.success_text))
                    .child("Passed"),
                (large(BadgeVariant::Safe, theme))
                    .child(small_icon(Icon::Tick).text_color(theme.success_text))
                    .child("Passed"),
            ])
        },
    },
    Example {
        id: "components-badge--ci-states",
        name: "CI States",
        draw: |theme| {
            v_flex()
                .gap(dp(12.))
                .children(
                    [
                        ("success", BadgeVariant::Safe, Some(Icon::Tick)),
                        ("failure", BadgeVariant::Danger, Some(Icon::Cross)),
                        ("in progress", BadgeVariant::LightGray, None),
                        ("in progress (some failed)", BadgeVariant::Danger, None),
                        ("cancelled", BadgeVariant::LightGray, Some(Icon::Cross)),
                        ("action required", BadgeVariant::Warn, Some(Icon::Warning)),
                        ("unknown", BadgeVariant::LightGray, Some(Icon::Question)),
                    ]
                    .map(|(label, variant, icon)| {
                        let color = variant.colors(theme).1;
                        let mark = match icon {
                            Some(icon) => small_icon(icon).text_color(color).into_any_element(),
                            None => spinning(color, true).into_any_element(),
                        };
                        h_flex()
                            .gap(dp(8.))
                            .items_center()
                            .child(regular(variant, theme).child(mark))
                            .child(div().text_12().child(label))
                    }),
                )
                .into_any_element()
        },
    },
];

fn regular(variant: BadgeVariant, theme: &Theme) -> Div {
    badge(variant, BadgeSize::Regular, theme)
}

fn large(variant: BadgeVariant, theme: &Theme) -> Div {
    badge(variant, BadgeSize::Large, theme)
}

/// Badges side by side, 8px apart, as the stories lay them out.
fn row(badges: impl IntoIterator<Item = Div>) -> AnyElement {
    h_flex()
        .gap(dp(8.))
        .items_center()
        .children(badges)
        .into_any_element()
}

pub struct BadgeStory;

impl Render for BadgeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(EXAMPLES.into_iter().map(|e| example(e, theme)))
    }
}
