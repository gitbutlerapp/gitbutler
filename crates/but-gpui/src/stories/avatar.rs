use gpui::{Context, Window, div, prelude::*};

use super::{Example, example, page, section, variant};
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::{avatar, dp, h_flex, picture_button};

const LOGINS: [&str; 8] = [
    "krlvi",
    "schacon",
    "Caleb-T-Owens",
    "mtsgrd",
    "slarse",
    "samhh",
    "estib-vega",
    "PavelLaptev",
];

/// The stories of `Avatar.stories.tsx` this client can draw. The rest load a photo from a URL,
/// which is the app's to fetch.
pub(super) const EXAMPLES: [Example; 1] = [Example {
    id: "components-avatar--no-picture",
    name: "No Picture",
    draw: |theme| {
        div()
            .grid()
            .grid_cols(4)
            .gap(dp(12.))
            .children(LOGINS.map(|login| {
                h_flex()
                    .gap(dp(8.))
                    .children([14., 16., 18.].map(|size| avatar(login, size, None, theme)))
            }))
            .into_any_element()
    },
}];

/// A person's picture, as but-ui-react's `Avatar`: their initial on a colour picked from who they
/// are, where there's no photo. Pressed, it opens a menu, as the account's does.
pub struct AvatarStory;

impl Render for AvatarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page([
            example(EXAMPLES[0], theme),
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
