use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{Icon, icon_toggle, toggle_group};

const TOGGLES: [(Icon, &str); 3] = [
    (Icon::TextWrap, "Wrap lines"),
    (Icon::FilesSidebar, "Show files"),
    (Icon::Eye, "Show whitespace"),
];

#[derive(Default)]
pub struct ToggleGroupStory {
    pressed: [bool; TOGGLES.len()],
}

impl Render for ToggleGroupStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let toggles =
            (TOGGLES.iter().zip(self.pressed).enumerate()).map(|(ix, ((glyph, tip), pressed))| {
                icon_toggle(("toggle", ix), *glyph, tip, None, pressed, theme).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if let Some(pressed) = this.pressed.get_mut(ix) {
                            *pressed = !*pressed;
                        }
                        cx.notify();
                    }),
                )
            });
        let fixed = [false, true].map(|pressed| {
            let id = if pressed { "pressed" } else { "unpressed" };
            icon_toggle(id, Icon::TextWrap, "Wrap lines", None, pressed, theme)
        });
        page([
            section(
                "States",
                theme,
                [variant("Off, then on", theme, toggle_group(theme, fixed))],
            ),
            section(
                "Try it",
                theme,
                [variant(
                    "Each on its own, named by its tooltip",
                    theme,
                    toggle_group(theme, toggles),
                )],
            ),
        ])
    }
}
