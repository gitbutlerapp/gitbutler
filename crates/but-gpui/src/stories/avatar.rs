use gpui::{Context, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::picture_button;

/// A person's picture, as but-ui-react's `Avatar`: their initial on a colour picked from who they
/// are, where there's no photo. Pressed, it opens a menu, as the account's does.
pub struct AvatarStory;

impl Render for AvatarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page([
            section(
                "Pictures",
                theme,
                [("16", 16.), ("18", 18.), ("24", 24.)]
                    .map(|(name, size)| variant(name, theme, person_avatar("Ada", size, theme))),
            ),
            section(
                "Pressable, opening a menu",
                theme,
                [variant(
                    "Dims under the pointer, ringed when Tab reaches it",
                    theme,
                    picture_button("pressable", person_avatar("Ada", 18., theme))
                        .tooltip("Settings")
                        .on_click(|_, _, _| {}),
                )],
            ),
        ])
    }
}
