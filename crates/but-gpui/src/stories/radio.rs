use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{dp, h_flex, radio, radio_disabled, v_flex};

const CHOICES: [&str; 3] = ["Merge", "Squash and merge", "Rebase and merge"];

#[derive(Default)]
pub struct RadioStory {
    picked: usize,
}

impl Render for RadioStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let choices = (CHOICES.iter().enumerate()).map(|(ix, label)| {
            h_flex()
                .id(("choice", ix))
                .gap(dp(8.))
                .child(radio(("choice-radio", ix), self.picked == ix, theme))
                .child(*label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.picked = ix;
                    cx.notify();
                }))
        });
        page([
            section(
                "States",
                theme,
                [
                    variant("Off", theme, radio("off", false, theme)),
                    variant("On", theme, radio("on", true, theme)),
                ],
            ),
            section(
                "Disabled, saying why in the tooltip",
                theme,
                [
                    variant(
                        "Off",
                        theme,
                        radio_disabled("disabled-off", false, "Not on this forge", theme),
                    ),
                    variant(
                        "On",
                        theme,
                        radio_disabled("disabled-on", true, "Not on this forge", theme),
                    ),
                ],
            ),
            section(
                "Try it",
                theme,
                [v_flex().gap(dp(8.)).children(choices).into_any_element()],
            ),
        ])
    }
}
