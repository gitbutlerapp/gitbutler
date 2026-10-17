//! A branch's pull request on its page: its state, labels, checks and who's been asked to
//! review it, as the forge cache last saw them.

use gpui::{AnyElement, Context, div, prelude::*, px};

use super::Detail;
use crate::backend;
use crate::theme::Theme;
use crate::ui::review as ui_review;
use crate::ui::{ButtonStyle, Icon, button, h_flex, icon, muted, shortcut, small_icon, v_flex};

/// A branch's pull request in two lines, as t3code shows one: what it is, then its number,
/// state and checks, with a way to open it.
pub(super) fn render_review(
    review: &backend::Review,
    theme: &Theme,
    cx: &Context<Detail>,
) -> AnyElement {
    let (glyph, color) = ui_review::state_glyph(review.state, theme);
    let title = if review.title.is_empty() {
        format!("Pull request #{}", review.number)
    } else {
        review.title.clone()
    };
    v_flex()
        .flex_none()
        .px(px(16.))
        .py(px(12.))
        .gap(px(4.))
        .border_b_1()
        .border_color(theme.border_3)
        .child(
            h_flex()
                .gap(px(8.))
                .child(icon(glyph).text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(title),
                )
                .when_some(review.url.clone(), |el, url| {
                    el.child(
                        button("open-review", "Open")
                            .icon(Icon::Link)
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key(
                                "Open the pull request",
                                shortcut(&crate::app::OpenPullRequest, cx),
                            )
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    )
                }),
        )
        .child(
            h_flex()
                .pl(px(24.))
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(format!("#{}", review.number))
                .child("·")
                .child(ui_review::state_label(review.state))
                .when_some(review.checks, |el, checks| {
                    let (glyph, color, text) = ui_review::checks_summary(checks, theme);
                    el.child("·")
                        .child(small_icon(glyph).text_color(color))
                        .child(text)
                }),
        )
        .when(!review.labels.is_empty(), |el| {
            el.child(h_flex().pl(px(24.)).gap(px(4.)).flex_wrap().children(
                review.labels.iter().map(|label| {
                    div()
                        .px(px(6.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(theme.border_2)
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(label.clone())
                }),
            ))
        })
        .when(!review.failed_checks.is_empty(), |el| {
            el.child(failed_checks(&review.failed_checks, theme))
        })
        .when(!review.reviewers.is_empty(), |el| {
            let reviewers = review.reviewers.join(", ");
            el.child(muted(format!("Review requested from {reviewers}"), theme).pl(px(24.)))
        })
        .into_any_element()
}

/// The checks that failed, a line each, opening its log when clicked.
fn failed_checks(failed: &[(String, String)], theme: &Theme) -> impl IntoElement {
    /// More than this many say little more than the count does.
    const SHOWN: usize = 5;
    v_flex()
        .pl(px(24.))
        .text_size(px(12.))
        .children(
            failed
                .iter()
                .take(SHOWN)
                .enumerate()
                .map(|(ix, (name, url))| {
                    let url = url.clone();
                    h_flex()
                        .id(("failed-check", ix))
                        .gap(px(6.))
                        .min_w(px(0.))
                        .text_color(theme.danger)
                        .cursor_pointer()
                        .tooltip(|_, cx| {
                            crate::ui::Tooltip::with_key("Open why it failed".into(), None, cx)
                        })
                        .child(small_icon(Icon::Cross).text_color(theme.danger))
                        .child(div().min_w(px(0.)).truncate().child(name.clone()))
                        .on_click(move |_, _, cx| cx.open_url(&url))
                }),
        )
}
