use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::switch;

#[derive(Default)]
pub struct SwitchStory {
    on: bool,
}

impl Render for SwitchStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let why = Some("Sign in first");
        let live = switch("live", self.on, theme).on_click(cx.listener(|this, _, _, cx| {
            this.on = !this.on;
            cx.notify();
        }));
        page([
            section(
                "States",
                theme,
                [
                    variant("Off", theme, switch("off", false, theme)),
                    variant("On", theme, switch("on", true, theme)),
                    variant(
                        "Disabled, off",
                        theme,
                        switch("disabled-off", false, theme).disabled_because(why),
                    ),
                    variant(
                        "Disabled, on",
                        theme,
                        switch("disabled-on", true, theme).disabled_because(why),
                    ),
                ],
            ),
            section(
                "Large, for a settings row",
                theme,
                [
                    variant("Off", theme, switch("large-off", false, theme).large()),
                    variant("On", theme, switch("large-on", true, theme).large()),
                    variant(
                        "Disabled, on",
                        theme,
                        switch("large-disabled", true, theme)
                            .large()
                            .disabled_because(why),
                    ),
                ],
            ),
            section(
                "Try it",
                theme,
                [variant(if self.on { "On" } else { "Off" }, theme, live)],
            ),
        ])
    }
}
