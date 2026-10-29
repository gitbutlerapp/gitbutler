//! GitButler's file-type marks, shared with Lite and but.dev: which one a file gets, by the
//! same map and rule as `@gitbutler/ui-react`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use gpui::{App, ImageSource, Img, RenderImage, SvgRenderer, img, prelude::*};

use super::dp;

include!(concat!(env!("OUT_DIR"), "/file_icons.rs"));

thread_local! {
    /// Each mark, drawn the first time it's needed. Loaded as an asset instead, a mark new
    /// to the screen stayed blank in a view gpui caches until something else redrew it.
    static DRAWN: RefCell<HashMap<&'static str, Arc<RenderImage>>> = RefCell::default();
}

/// The icon names by whole file name, then by extension, from the UI package's map.
static TYPES: LazyLock<[HashMap<&'static str, &'static str>; 2]> = LazyLock::new(|| {
    let source = include_str!("../../../../packages/ui-react/src/file-icons/typeMap.ts");
    // File names follow extensions, as a test checks.
    let (extensions, names) = source
        .split_once("symbolFileNamesToIcons")
        .unwrap_or((source, ""));
    [entries(names), entries(extensions)]
});

/// The `key: "value",` lines of one of the map's objects.
fn entries(source: &'static str) -> HashMap<&'static str, &'static str> {
    (source.lines())
        .filter_map(|line| {
            let (key, value) = line.trim().strip_suffix(',')?.split_once(": ")?;
            Some((key.trim_matches('"'), value.trim_matches('"')))
        })
        .collect()
}

/// The mark for the file at `path`: by its whole name, else by its longest extension that has
/// one ("d.ts" before "ts"), else a plain document.
pub fn file_icon(path: &str, cx: &App) -> Img {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let suffixes = std::iter::once(name.as_str())
        .chain((name.match_indices('.')).filter_map(|(i, _)| name.get(i + 1..)));
    let icon = suffixes
        .flat_map(|suffix| TYPES.iter().filter_map(move |map| map.get(suffix)))
        .find(|icon| icon_svg(icon).is_some())
        .copied()
        .unwrap_or("document");
    let drawn = DRAWN.with_borrow_mut(|drawn| {
        let draw = || draw(&cx.svg_renderer(), icon);
        drawn.entry(icon).or_insert_with(draw).clone()
    });
    img(ImageSource::Render(drawn)).size(dp(14.)).flex_none()
}

/// Mark `icon` drawn as `img` draws an SVG asset.
#[expect(
    clippy::expect_used,
    reason = "only compiled-in marks are drawn, and a test draws each"
)]
fn draw(renderer: &SvgRenderer, icon: &str) -> Arc<RenderImage> {
    let svg = icon_svg(icon).expect("only compiled-in marks are drawn");
    (renderer.render_single_frame(svg, 1.)).expect("the compiled-in marks draw, as a test checks")
}

#[cfg(test)]
mod tests {
    use super::{TYPES, draw, icon_svg};

    #[test]
    fn the_shared_map_reads_and_names_icons_that_exist() {
        let [names, extensions] = &*TYPES;
        assert_eq!(
            extensions.get("rs"),
            Some(&"rust"),
            "extensions read from the map"
        );
        assert_eq!(
            names.get("yarn.lock"),
            Some(&"yarn"),
            "whole names read from the map"
        );
        assert!(icon_svg("rust").is_some(), "the icons are compiled in");
        assert!(icon_svg("document").is_some(), "the fallback exists");
    }

    #[test]
    fn every_mark_draws() {
        let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(crate::ui::Assets));
        for icon in TYPES
            .iter()
            .flat_map(|map| map.values())
            .filter(|i| icon_svg(i).is_some())
        {
            draw(&renderer, icon);
        }
    }
}
