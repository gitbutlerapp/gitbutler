use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{SwitchButtonStyle, switch_button};

#[derive(Default)]
pub struct SwitchButtonStory {
    draft: bool,
    auto_merge: bool,
}

impl Render for SwitchButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let why = Some("Sign in first");
        let draft =
            switch_button("draft", "Draft", self.draft).on_click(cx.listener(|this, _, _, cx| {
                this.draft = !this.draft;
                cx.notify();
            }));
        let auto_merge = switch_button("auto-merge", "Auto-merge", self.auto_merge)
            .style(SwitchButtonStyle::Outline)
            .on_click(cx.listener(|this, _, _, cx| {
                this.auto_merge = !this.auto_merge;
                cx.notify();
            }));
        page([
            section(
                "Variants",
                theme,
                [
                    variant("Ghost", theme, draft),
                    variant("Outline", theme, auto_merge),
                ],
            ),
            section(
                "Disabled",
                theme,
                [
                    variant(
                        "Ghost, off",
                        theme,
                        switch_button("disabled-off", "Draft", false).disabled_because(why),
                    ),
                    variant(
                        "Ghost, on",
                        theme,
                        switch_button("disabled-on", "Draft", true).disabled_because(why),
                    ),
                    variant(
                        "Outline, on",
                        theme,
                        switch_button("disabled-outline", "Auto-merge", true)
                            .style(SwitchButtonStyle::Outline)
                            .disabled_because(why),
                    ),
                ],
            ),
        ])
    }
}
