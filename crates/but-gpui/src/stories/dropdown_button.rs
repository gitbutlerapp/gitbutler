use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{ButtonStyle, button, dropdown_button};

/// As but-ui-react's DropdownButton: an action joined to a chevron that opens its menu.
const STYLES: [(&str, ButtonStyle); 3] = [
    ("Pop", ButtonStyle::Pop),
    ("Gray", ButtonStyle::Gray),
    ("Outline", ButtonStyle::Outline),
];

pub struct DropdownButtonStory;

impl Render for DropdownButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let split = |id: &'static str, ix: usize, style: ButtonStyle, why: Option<&str>| {
            dropdown_button(
                (button((id, ix), "Push").style(style))
                    .disabled_because(why)
                    .on_click(|_, _, _| {}),
                "Push options",
                |_, _, _| {},
            )
        };
        page([
            section(
                "Styles",
                theme,
                (STYLES.into_iter().enumerate())
                    .map(|(ix, (name, style))| {
                        variant(name, theme, split("style", ix, style, None))
                    })
                    .collect::<Vec<_>>(),
            ),
            section(
                "Disabled, saying why in the tooltip",
                theme,
                (STYLES.into_iter().enumerate())
                    .map(|(ix, (name, style))| {
                        variant(
                            name,
                            theme,
                            split("disabled", ix, style, Some("Nothing to push")),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        ])
    }
}
