use gpui::{Context, Div, Stateful, Window, div, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::segmented;

const VIEWS: [&str; 2] = ["Unified", "Split"];
const WHOSE: [&str; 3] = ["All", "Mine", "Others"];

#[derive(Default)]
pub struct SegmentedStory {
    view: usize,
    whose: usize,
}

impl Render for SegmentedStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let views = segments(&VIEWS, self.view, |label, ix| {
            div()
                .id(("view", ix))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.view = ix;
                    cx.notify();
                }))
        });
        let whose = segments(&WHOSE, self.whose, |label, ix| {
            div()
                .id(("whose", ix))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.whose = ix;
                    cx.notify();
                }))
        });
        page([section(
            "Sizes",
            theme,
            [
                variant("Regular", theme, segmented(false, theme, views)),
                variant("Small", theme, segmented(true, theme, whose)),
            ],
        )])
    }
}

/// `labels` as segments built by `segment`, the one at `picked` picked.
fn segments(
    labels: &[&'static str],
    picked: usize,
    mut segment: impl FnMut(&'static str, usize) -> Stateful<Div>,
) -> Vec<(Stateful<Div>, bool)> {
    (labels.iter().enumerate())
        .map(|(ix, label)| (segment(label, ix), ix == picked))
        .collect()
}
