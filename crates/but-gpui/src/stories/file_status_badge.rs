use gpui::{Context, Window, prelude::*};

use super::{Example, example, faint, page};
use crate::theme::Theme;
use crate::ui::{FileStatus, Text, dp, file_status_badge, h_flex, v_flex};

/// The stories of `FileStatusBadge.stories.tsx`.
pub(super) const EXAMPLES: [Example; 2] = [
    Example {
        id: "components-filestatusbadge--default",
        name: "Default",
        draw: |theme| file_status_badge(FileStatus::Addition, theme).into_any_element(),
    },
    Example {
        id: "components-filestatusbadge--all-statuses",
        name: "All Statuses",
        draw: |theme| {
            h_flex()
                .gap(dp(16.))
                .items_center()
                .children(
                    [
                        FileStatus::Addition,
                        FileStatus::Deletion,
                        FileStatus::Modification,
                        FileStatus::Rename,
                    ]
                    .map(|status| {
                        v_flex()
                            .gap(dp(8.))
                            .items_center()
                            .child(file_status_badge(status, theme))
                            .child(faint(status.word(), theme).text_11())
                    }),
                )
                .into_any_element()
        },
    },
];

pub struct FileStatusBadgeStory;

impl Render for FileStatusBadgeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(EXAMPLES.into_iter().map(|e| example(e, theme)))
    }
}
