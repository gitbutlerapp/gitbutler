//! A surface with nothing in it, as but-ui-react's `EmptyState`.

use gpui::{AnyElement, App, SharedString, div, prelude::*};

use crate::illustration::{Illustration, illustration};
use crate::theme::Theme;
use crate::*;

/// A surface with nothing in it, as but-ui-react's EmptyState: a drawing picked by what the state
/// means, a title naming it, and a line saying what happens next, each left out when there's
/// none. Its foot is padded so the block sits a little above the middle, where it reads as
/// centred, and it caps its width so it stays a block in a wide pane. In a pane too short for
/// it, it's cut off rather than drawn over what's above.
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
                                    .text_size(dp(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.text_1)
                                    .child(title),
                            )
                        })
                        .when(!detail.is_empty(), |el| el.child(muted(detail, theme))),
                ),
        )
        .into_any_element()
}
