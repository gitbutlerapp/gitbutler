//! A table of a description: its cells in columns, under a header row.

use gpui::{AnyElement, IntoElement, ParentElement, Styled, prelude::FluentBuilder, relative};

use super::{Block, Gallery, Kind, Laid, element};
use crate::theme::Theme;
use crate::ui::{TextPoint, dp, h_flex, v_flex};

/// Each column's share of a table's width, by the root of its longest cell's length: a long
/// column gets more room, and a short one still has enough for its words. `cells` are the
/// table's blocks.
fn column_shares(cells: &[Block]) -> Vec<f32> {
    // Shorter cells than this all want the same room, and an image wants as much as a sentence.
    const SHORTEST: usize = 16;
    const IMAGE: usize = 100;
    let mut longest = Vec::new();
    for block in cells {
        let Some(cell) = block.cell else {
            continue;
        };
        let length = match block.kind {
            Kind::Images(_) => IMAGE,
            _ => block.text.chars().count(),
        };
        longest.resize(longest.len().max(cell.column + 1), SHORTEST);
        if let Some(column) = longest.get_mut(cell.column) {
            *column = length.max(*column);
        }
    }
    let weights = longest.iter().map(|length| (*length as f32).sqrt());
    let total: f32 = weights.clone().sum();
    weights.map(|weight| weight / total).collect()
}

/// A table, `first` the index of its first block: its rows, each cell its column's share of
/// the width so the columns line up, and each row under a line that closes the one above.
pub(super) fn draw(
    id: &str,
    first: usize,
    cells: &[Block],
    gallery: &Gallery,
    range: Option<(TextPoint, TextPoint)>,
    texts: &mut Vec<Laid>,
    theme: &Theme,
) -> AnyElement {
    let shares = column_shares(cells);
    let mut table = (v_flex().border_b_1().border_x_1()).border_color(theme.border_3);
    let mut next = first;
    let row_of = |block: &Block| block.cell.map(|cell| cell.row);
    for row in cells.chunk_by(|a, b| row_of(a) == row_of(b)) {
        let head = (row.first().and_then(|block| block.cell)).is_some_and(|cell| cell.row == 1);
        let mut line = (h_flex().items_stretch().border_t_1())
            .border_color(theme.border_3)
            // On the grey code sits on.
            .when(head, |el| el.bg(theme.fill_gray.opacity(0.08)));
        for (column, share) in shares.iter().enumerate() {
            let blocks = (row.iter().enumerate())
                .filter(|(_, block)| block.cell.is_some_and(|cell| cell.column == column));
            line = line.child(
                (v_flex().flex_none().w(relative(*share)))
                    .px(dp(10.))
                    .py(dp(6.))
                    .gap(dp(8.))
                    .when(column > 0, |el| {
                        el.border_l_1().border_color(theme.border_3)
                    })
                    .children(blocks.map(|(n, block)| {
                        element(id, next + n, block, gallery, range, texts, theme)
                    })),
            );
        }
        table = table.child(line);
        next += row.len();
    }
    table.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    #[test]
    fn columns_share_the_width_by_what_they_hold() {
        let blocks = parse(&format!(
            "| n | {} | ![a](https://github.com/a.png) |\n|---|---|---|\n",
            "word ".repeat(80).trim_end()
        ));
        let shares = column_shares(&blocks);
        assert!(
            (shares.iter().sum::<f32>() - 1.).abs() < 0.001,
            "together they fill the table: {shares:?}"
        );
        assert!(
            shares[0] < shares[2] && shares[2] < shares[1],
            "a short column takes least, and a long one most: {shares:?}"
        );
        assert!(
            shares[0] > 0.1,
            "the short column keeps room for its words: {shares:?}"
        );
    }
}
