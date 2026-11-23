use gpui::{AnyElement, Context, Window, div, prelude::*};

use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::{
    Icon, MetaStatusVariant, SplitCell, SplitTile, dp, h_flex, split_preview, split_preview_map,
};

/// A tile of the stories' splits: its row's glyph and name, its status, and its place.
struct Story {
    icon: Icon,
    name: &'static str,
    status: Option<MetaStatusVariant>,
    at: (f32, f32, f32, f32),
}

const SHELL: &str = "pavellaptev@Pavels-MacBook-Pro:~/projects/shop";

/// A branch's page beside the agent that works on it and the tests it runs.
const THREE: [Story; 3] = [
    tile(Icon::Branch, "sales-tax", None, (0., 0., 0.5, 1.)),
    tile(
        Icon::Terminal,
        "claude",
        Some(MetaStatusVariant::Warning),
        (0.5, 0., 0.5, 0.5),
    ),
    tile(Icon::Terminal, "cargo test", None, (0.5, 0.5, 0.5, 0.5)),
];

/// A shell given nearly all the room, the branch's page squeezed to its glyph.
const NARROW: [Story; 2] = [
    tile(Icon::Branch, "free-shipping", None, (0., 0., 0.1, 1.)),
    tile(Icon::Terminal, SHELL, None, (0.1, 0., 0.9, 1.)),
];

/// Two columns of two: the page and its tests, the agent and the changes it made.
const FOUR: [Story; 4] = [
    tile(Icon::Branch, "sales-tax", None, (0., 0., 0.5, 0.5)),
    tile(
        Icon::Terminal,
        "cargo test",
        Some(MetaStatusVariant::Passed),
        (0., 0.5, 0.5, 0.5),
    ),
    tile(
        Icon::Terminal,
        "claude",
        Some(MetaStatusVariant::Warning),
        (0.5, 0., 0.5, 0.5),
    ),
    tile(
        Icon::Diff,
        "Uncommitted changes",
        None,
        (0.5, 0.5, 0.5, 0.5),
    ),
];

const fn tile(
    icon: Icon,
    name: &'static str,
    status: Option<MetaStatusVariant>,
    at: (f32, f32, f32, f32),
) -> Story {
    Story {
        icon,
        name,
        status,
        at,
    }
}

/// The card for `split`, opened from the tile named `here`, named after its first tile.
fn card(split: &[Story], here: Option<&str>, theme: &Theme) -> AnyElement {
    let tiles: Vec<SplitTile> = (split.iter())
        .map(|tile| SplitTile {
            icon: tile.icon,
            name: tile.name.into(),
            status: tile.status,
            here: Some(tile.name) == here,
        })
        .collect();
    let home = split.first().map_or("", |tile| tile.name);
    split_preview(home, &tiles, theme).into_any_element()
}

/// The map of `split` as the row of the tile named `here` marks it.
fn map(split: &[Story], here: &str, theme: &Theme) -> AnyElement {
    let cells = split.iter().map(|tile| {
        let (left, top, width, height) = tile.at;
        SplitCell {
            left,
            top,
            width,
            height,
            here: tile.name == here,
        }
    });
    split_preview_map(cells, theme).into_any_element()
}

/// The stories of `SplitPreview.stories.tsx` this client can draw.
pub(super) const EXAMPLES: [Example; 4] = [
    Example {
        id: "components-splitpreview--default",
        name: "Default",
        draw: |theme| card(&THREE, None, theme),
    },
    Example {
        id: "components-splitpreview--from-a-row",
        name: "From A Row",
        draw: |theme| card(&THREE, Some("claude"), theme),
    },
    Example {
        id: "components-splitpreview--layouts",
        name: "Layouts",
        draw: |theme| {
            h_flex()
                .flex_wrap()
                .items_start()
                .gap(dp(24.))
                .child(card(&THREE, None, theme))
                .child(card(&NARROW, None, theme))
                .child(card(&FOUR, None, theme))
                .into_any_element()
        },
    },
    Example {
        id: "components-splitpreview--map",
        name: "Map",
        draw: |theme| {
            h_flex()
                .gap(dp(16.))
                .children(FOUR.iter().map(|tile| map(&FOUR, tile.name, theme)))
                .into_any_element()
        },
    },
];

pub struct SplitPreviewStory;

impl Render for SplitPreviewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        div().child(page(EXAMPLES.into_iter().map(|e| example(e, theme))))
    }
}
