//! What a row's split holds, as but-ui-react's `SplitPreview`: the small map of its tiles on
//! every row it shows, [`split_preview_map`], and the card naming them, [`SplitPreview`].

use gpui::{AnyElement, Context, Div, SharedString, Window, div, prelude::*};

use crate::theme::{Theme, tokens};
use crate::*;

/// How wide and tall the map is: an icon's width.
const MAP: (f32, f32) = (16., 12.);

/// A tile of a split, in shares of the split's room from its top left corner, and whether it's
/// the tile of the row the map is on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitCell {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub here: bool,
}

/// The mark on every row a split shows, as React's `SplitPreviewMap`: the split's layout at 16
/// by 12, a hairline between tiles, the tile of the row it's on inked and the rest faint. It goes
/// in the row's `split` cell; it is a mark, not a button, as the row opens the split.
pub fn split_preview_map(cells: impl IntoIterator<Item = SplitCell>, theme: &Theme) -> Div {
    // Half the hairline each side of an edge two tiles share; none at the map's own edges.
    let inset = |at: f32| {
        if at <= 1e-4 || at >= 1. - 1e-4 {
            0.
        } else {
            0.5
        }
    };
    let cells = cells.into_iter().map(|cell| {
        let (x0, x1) = (cell.left, cell.left + cell.width);
        let (y0, y1) = (cell.top, cell.top + cell.height);
        let left = x0 * MAP.0 + inset(x0);
        let top = y0 * MAP.1 + inset(y0);
        let right = x1 * MAP.0 - inset(x1);
        let bottom = y1 * MAP.1 - inset(y1);
        div()
            .absolute()
            .left(dp(left))
            .top(dp(top))
            .w(dp((right - left).max(1.)))
            .h(dp((bottom - top).max(1.)))
            .rounded(dp(theme.shape.xs))
            .bg(theme.subtle_foreground)
            .when(!cell.here, |el| el.opacity(0.45))
    });
    div()
        .relative()
        .flex_none()
        .w(dp(MAP.0))
        .h(dp(MAP.1))
        .children(cells)
}

/// A tile as the card names it: its row's glyph and name, as the sidebar says them, its
/// status, and whether it's the tile of the row the card was opened from.
#[derive(Debug, Clone)]
pub struct SplitTile {
    pub icon: Icon,
    pub name: SharedString,
    pub status: Option<MetaStatusVariant>,
    pub here: bool,
}

/// What a row's split holds, as React's `SplitPreview`: named after the row it was made on, the
/// rows its tiles show, each with its state, the one it was opened from marked. It answers "what
/// did I put there?", so it names the tiles and says nothing of their content. A view, so a
/// map's hover can open it.
pub struct SplitPreview {
    pub home: SharedString,
    pub tiles: Vec<SplitTile>,
}

/// The card [`SplitPreview`] draws: `tiles` under "Split on" `home`.
pub fn split_preview(home: &str, tiles: &[SplitTile], theme: &Theme) -> Div {
    let items = tiles.iter().map(|tile| {
        let status = tile
            .status
            .map(|variant| meta_status(variant, None, theme).into_any_element());
        popup_item(Some(tile.icon), tile.name.clone(), status, tile.here, theme).into_any_element()
    });
    popover(theme)
        .w(dp(tokens::SIZE_TOOLTIP_MAX))
        .overflow_hidden()
        .child(popup_section(
            Some(format!("Split on {home}").into()),
            items.collect::<Vec<AnyElement>>(),
            theme,
        ))
}

impl Render for SplitPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        split_preview(&self.home, &self.tiles, Theme::get(cx))
    }
}
