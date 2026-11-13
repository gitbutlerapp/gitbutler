use gpui::{AnyElement, Context, SharedString, Window, div, img, prelude::*};

use super::{page, section};
use crate::theme::Theme;
use crate::ui::{Icon, dp, faint, h_flex, icon_at, segmented, v_flex};

/// The sizes the app draws icons at, so each can be looked at here. Their strokes must look as
/// wide at every one of them.
const SIZES: [f32; 3] = [12., 14., 16.];

/// Every icon the app has, at any size it draws them, named as in code.
pub struct IconsStory {
    /// The size the grids are drawn at, one of [`SIZES`].
    px: f32,
}

impl Default for IconsStory {
    fn default() -> Self {
        // 16px, what most of the app draws.
        Self { px: 16. }
    }
}

impl Render for IconsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let px = self.px;
        let (filled, plain): (Vec<Icon>, Vec<Icon>) =
            Icon::ALL.iter().partition(|glyph| filled(**glyph));
        let sizes = SIZES.map(|size| {
            let segment = (div().id(("size", size as usize)))
                .child(format!("{size:.0}px"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.px = size;
                    cx.notify();
                }));
            (segment, size == px)
        });
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
                    icon_at(glyph, px)
                        .text_color(theme.text_1)
                        .into_any_element(),
                )
            })
            .collect();
        let filled = (filled.into_iter())
            .map(|glyph| cell(glyph, img(glyph.path()).size(dp(px)).into_any_element()))
            .collect();
        page([
            section(
                "Size",
                theme,
                [segmented(false, theme, sizes).into_any_element()],
            ),
            section(
                "In the text colour, their strokes as wide at every size",
                theme,
                [grid(plain)],
            ),
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
    matches!(glyph, Icon::FolderFill)
}
