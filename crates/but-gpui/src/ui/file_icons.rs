//! GitButler's file-type marks, shared with but.dev: which one a file gets, by the
//! same map and rule as `@gitbutler/but-ui-react`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use gpui::{
    App, Div, Hsla, ImageSource, Img, RenderImage, Rgba, SvgRenderer, div, img, prelude::*,
};

use super::dp;
use crate::theme::Theme;

include!(concat!(env!("OUT_DIR"), "/file_icons.rs"));

thread_local! {
    /// Each mark, drawn the first time it's needed. Loaded as an asset instead, a mark new
    /// to the screen stayed blank in a view gpui caches until something else redrew it.
    static DRAWN: RefCell<HashMap<&'static str, Arc<RenderImage>>> = RefCell::default();

    /// The folder, drawn once for each pair of gradient stops the accent and appearance give.
    static FOLDERS: RefCell<HashMap<[String; 2], Arc<RenderImage>>> = RefCell::default();
}

/// The icon names by whole file name, then by extension, from the UI package's map.
static TYPES: LazyLock<[HashMap<&'static str, &'static str>; 2]> = LazyLock::new(|| {
    let source = include_str!("../../../../packages/but-ui-react/src/file-icons/typeMap.ts");
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

/// A folder's mark, in the box a file's mark has. Its gradient is the accent taken toward its
/// foreground, as but-ui-react's `FolderIcon` mixes it in CSS, which GPUI can't: the stops go
/// into the mark's SVG before it's drawn, so it follows the accent and the appearance.
pub fn folder_icon(theme: &Theme, cx: &App) -> Div {
    // `color-mix(in srgb, var(--but-primary), var(--but-primary-foreground) 60%)` and 20%, as
    // `FolderIcon.module.css` writes them. `pop_bg` is the accent's primary, which the accent
    // setting replaces, so the folder follows it.
    let stops = [0.6, 0.2].map(|part| hex(toward(theme.pop_bg, theme.on_pop, part)));
    let drawn = FOLDERS.with_borrow_mut(|drawn| {
        let draw = || draw_folder(&cx.svg_renderer(), &stops);
        drawn.entry(stops.clone()).or_insert_with(draw).clone()
    });
    div()
        .flex_none()
        .size(dp(14.))
        .child(img(ImageSource::Render(drawn)).size(dp(14.)))
}

/// `from` taken `part` of the way to `to`, as CSS's `color-mix` in sRGB.
fn toward(from: Hsla, to: Hsla, part: f32) -> Rgba {
    let ([from, to], mix) = ([from, to].map(Rgba::from), |a: f32, b: f32| {
        a + (b - a) * part
    });
    Rgba {
        r: mix(from.r, to.r),
        g: mix(from.g, to.g),
        b: mix(from.b, to.b),
        a: 1.,
    }
}

/// A colour as the SVG writes it.
fn hex(Rgba { r, g, b, .. }: Rgba) -> String {
    let byte = |channel: f32| (channel.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

/// The folder mark with `stops` in place of the gradient's, top first.
#[expect(
    clippy::expect_used,
    reason = "the folder is compiled in, and a test draws it"
)]
fn draw_folder(renderer: &SvgRenderer, stops: &[String; 2]) -> Arc<RenderImage> {
    let svg = icon_svg("folder").expect("the folder is compiled in from but-ui-react");
    let svg = String::from_utf8_lossy(svg);
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg.as_ref();
    let mut ix = 0;
    while let Some((before, after)) = rest.split_once("stop-color=\"") {
        let Some((_, after)) = after.split_once('"') else {
            break;
        };
        out.push_str(before);
        out.push_str("stop-color=\"");
        out.push_str(stops.get(ix).map_or("currentColor", String::as_str));
        out.push('"');
        rest = after;
        ix += 1;
    }
    out.push_str(rest);
    (renderer.render_single_frame(out.as_bytes(), 1.)).expect("the folder draws, as a test checks")
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
