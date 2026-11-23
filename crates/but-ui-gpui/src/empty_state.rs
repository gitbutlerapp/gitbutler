//! A surface with nothing in it, as but-ui-react's `EmptyState`.

use gpui::{AnyElement, App, SharedString, div, prelude::*};

use crate::illustration::{Illustration, illustration};
use crate::theme::Theme;
use crate::*;

/// An empty `title` or `detail` is left out. The bottom padding lifts the block a little above
/// the middle, where it reads as centred.
pub fn empty_state(
    drawing: Option<Illustration>,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let (title, detail): (SharedString, SharedString) = (title.into(), detail.into());
    v_flex()
        .size_full()
        .overflow_hidden()
        .items_center()
        .justify_center()
        .px(dp(32.))
        .pb(dp(60.))
        .text_center()
        .child(
            v_flex()
                .max_w(dp(350.))
                // So a short pane shrinks the drawing rather than clip it.
                .min_h_0()
                .items_center()
                .gap(dp(24.))
                .children(drawing.map(|drawing| illustration(drawing, theme, cx)))
                .child(
                    v_flex()
                        .items_center()
                        .gap(dp(8.))
                        .when(!title.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_14()
                                    .semibold()
                                    .text_color(theme.foreground)
                                    .child(title),
                            )
                        })
                        .when(!detail.is_empty(), |el| el.child(muted(detail, theme))),
                ),
        )
        .into_any_element()
}

/// How tall [`empty_state_tidy`] is with a title, a detail and a row of small buttons, so a host
/// that lays out by its rows' heights, as a sidebar's drawer does, can count it.
pub const EMPTY_TIDY_HEIGHT: f32 =
    14. * crate::text::LINE_HEIGHT_BODY + 6. + 13. * crate::text::LINE_HEIGHT_BODY + 12. + 20.;

/// A few rows with nothing in them, as but-ui-react's `EmptyState` with `variant="tidy"`: the
/// title and the detail closed up, 6 apart, and 12 over the `actions`, small buttons, at most
/// two. As tall as its content and up to 320 wide, centred; the host's padding is the room
/// around it. Each line keeps to one, cut short, where React's wraps, so a host can count its
/// height ([`EMPTY_TIDY_HEIGHT`]).
pub fn empty_state_tidy(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    actions: impl IntoIterator<Item = AnyElement>,
    theme: &Theme,
) -> gpui::Div {
    let (title, detail): (SharedString, SharedString) = (title.into(), detail.into());
    let actions: Vec<AnyElement> = actions.into_iter().collect();
    let line = || div().w_full().min_w_0().truncate();
    v_flex()
        .w_full()
        .max_w(dp(320.))
        .mx_auto()
        .items_center()
        .text_center()
        .gap(dp(12.))
        .child(
            v_flex()
                .w_full()
                .items_center()
                .gap(dp(6.))
                .when(!title.is_empty(), |el| {
                    el.child(
                        line()
                            .text_14()
                            .semibold()
                            .body()
                            .text_color(theme.foreground)
                            .child(title),
                    )
                })
                .when(!detail.is_empty(), |el| {
                    el.child(
                        line()
                            .text_13()
                            .body()
                            .text_color(theme.muted_foreground)
                            .child(detail),
                    )
                }),
        )
        .when(!actions.is_empty(), |el| {
            el.child(h_flex().gap(dp(6.)).children(actions))
        })
}
