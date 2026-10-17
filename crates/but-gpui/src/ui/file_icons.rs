//! GitButler's file-type marks, shared with Lite and but.dev: which one a file gets, by the
//! same map and rule as `@gitbutler/ui-react`.

use std::collections::HashMap;
use std::sync::LazyLock;

use gpui::{Img, SharedString, img, prelude::*, px};

include!(concat!(env!("OUT_DIR"), "/file_icons.rs"));

/// Where the marks are served from, for [`gpui::img`].
const PREFIX: &str = "file-icons/";

/// The icon names by whole file name, then by extension, from the UI package's map.
static TYPES: LazyLock<[HashMap<&'static str, &'static str>; 2]> = LazyLock::new(|| {
    let source = include_str!("../../../../packages/ui-react/src/file-icons/typeMap.ts");
    let (extensions, names) = (source.split_once("symbolFileNamesToIcons"))
        .expect("the map has file names after extensions");
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
pub fn file_icon(path: &str) -> Img {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let suffixes =
        std::iter::once(name.as_str()).chain(name.match_indices('.').map(|(i, _)| &name[i + 1..]));
    let icon = suffixes
        .flat_map(|suffix| TYPES.iter().filter_map(move |map| map.get(suffix)))
        .find(|icon| icon_svg(icon).is_some())
        .copied()
        .unwrap_or("document");
    img(SharedString::from(format!("{PREFIX}{icon}.svg")))
        .size(px(14.))
        .flex_none()
}

/// The SVG of a mark, by its asset path.
pub(super) fn load(path: &str) -> Option<&'static [u8]> {
    icon_svg(path.strip_prefix(PREFIX)?.strip_suffix(".svg")?)
}

#[cfg(test)]
mod tests {
    use super::{TYPES, icon_svg};

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
}
