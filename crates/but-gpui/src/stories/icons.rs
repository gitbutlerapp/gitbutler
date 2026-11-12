use gpui::{AnyElement, Context, SharedString, Window, img, prelude::*};

use super::{page, section};
use crate::theme::Theme;
use crate::ui::{Icon, dp, faint, h_flex, icon, v_flex};

/// Every icon the app has, at the size it draws them, named as in code.
pub struct IconsStory;

impl Render for IconsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let (filled, plain): (Vec<Icon>, Vec<Icon>) =
            Icon::ALL.iter().partition(|glyph| filled(**glyph));
        let cell = |glyph: Icon, drawn: AnyElement| {
            v_flex()
                .w(dp(120.))
                .items_center()
                .gap(dp(6.))
                .child(drawn)
                .child(faint(SharedString::from(format!("{glyph:?}")), theme))
                .into_any_element()
        };
        let grid = |cells: Vec<AnyElement>| {
            h_flex()
                .flex_wrap()
                .gap_y(dp(16.))
                .children(cells)
                .into_any_element()
        };
        let plain = (plain.into_iter())
            .map(|glyph| {
                cell(
                    glyph,
                    icon(glyph).text_color(theme.text_1).into_any_element(),
                )
            })
            .collect();
        let filled = (filled.into_iter())
            .map(|glyph| cell(glyph, img(glyph.path()).size(dp(16.)).into_any_element()))
            .collect();
        page([
            section("16px, in the text colour", theme, [grid(plain)]),
            section(
                "In colours of their own, drawn as images",
                theme,
                [grid(filled)],
            ),
        ])
    }
}

/// The icons with their colours in the file, which `svg()` would draw in one.
fn filled(glyph: Icon) -> bool {
    matches!(
        glyph,
        Icon::FolderFill
            | Icon::RepoPop
            | Icon::RepoSafe
            | Icon::RepoPurple
            | Icon::RepoDanger
            | Icon::RepoWarn
            | Icon::RepoGray
    )
}
