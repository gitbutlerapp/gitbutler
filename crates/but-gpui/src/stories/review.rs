use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::backend::{Checks, ReviewState};
use crate::theme::Theme;
use crate::ui::review::{checks_summary, state_glyph, state_label, title_color};
use crate::ui::{dp, h_flex, icon};

const STATES: [ReviewState; 4] = [
    ReviewState::Open,
    ReviewState::Draft,
    ReviewState::Merged,
    ReviewState::Closed,
];

/// How a pull request and its checks read at a glance, as the navigator shows them.
pub struct ReviewStory;

impl Render for ReviewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let checks = [
            (
                "Passed",
                Checks {
                    passed: 8,
                    ..Default::default()
                },
            ),
            (
                "Failed",
                Checks {
                    passed: 6,
                    failed: 2,
                    ..Default::default()
                },
            ),
            (
                "Running",
                Checks {
                    passed: 5,
                    running: 3,
                    ..Default::default()
                },
            ),
            (
                "Running, some failed",
                Checks {
                    passed: 4,
                    failed: 1,
                    running: 3,
                },
            ),
        ];
        page([
            section(
                "Pull request, with its title's colour",
                theme,
                STATES.map(|state| {
                    let (glyph, color) = state_glyph(state, theme);
                    let title = h_flex()
                        .gap(dp(6.))
                        .text_color(title_color(state, theme))
                        .child(icon(glyph).text_color(color))
                        .child("Add stories");
                    variant(state_label(state), theme, title)
                }),
            ),
            section(
                "Checks",
                theme,
                checks.map(|(caption, checks)| {
                    let (glyph, color, text) = checks_summary(checks, theme);
                    let summary = h_flex()
                        .gap(dp(6.))
                        .text_color(theme.text_2)
                        .child(icon(glyph).text_color(color))
                        .child(text);
                    variant(caption, theme, summary)
                }),
            ),
        ])
    }
}
