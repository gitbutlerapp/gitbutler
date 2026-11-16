use gpui::{Context, Window, prelude::*};

use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::diff_stats;

/// The stories of `DiffStats.stories.tsx`.
pub(super) const EXAMPLES: [Example; 3] = [
    Example {
        id: "components-diffstats--default",
        name: "Default",
        draw: |theme| diff_stats(364, 20, false, theme).into_any_element(),
    },
    Example {
        id: "components-diffstats--only-additions",
        name: "Only Additions",
        draw: |theme| diff_stats(6, 0, false, theme).into_any_element(),
    },
    Example {
        id: "components-diffstats--only-deletions",
        name: "Only Deletions",
        draw: |theme| diff_stats(0, 12, false, theme).into_any_element(),
    },
];

pub struct DiffStatsStory;

impl Render for DiffStatsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(EXAMPLES.into_iter().map(|e| example(e, theme)))
    }
}
