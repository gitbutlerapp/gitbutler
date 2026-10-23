//! Compiles in GitButler's file-type icons from the shared UI package, so this app draws the
//! same marks as Lite and but.dev: a `match` from each icon's name to its SVG.

use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new("../../packages/ui-react/src/file-icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let out = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let mut names: Vec<String> = fs::read_dir(dir.join("svg"))
        .expect("the file icons are in packages/ui-react")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_stem()?.to_str()?.to_owned();
            (path.extension()? == "svg").then_some(name)
        })
        .collect();
    names.sort();
    let arms: String = names
        .iter()
        .map(|name| {
            let svg = fs::read_to_string(dir.join("svg").join(format!("{name}.svg")))
                .expect("the icon is readable");
            let path = Path::new(&out).join(format!("file-icon-{name}.svg"));
            fs::write(&path, plain_colors(&svg)).expect("OUT_DIR is writable");
            format!("        {name:?} => Some(include_bytes!({path:?})),\n")
        })
        .collect();
    fs::write(
        Path::new(&out).join("file_icons.rs"),
        format!(
            "fn icon_svg(name: &str) -> Option<&'static [u8]> {{\n    match name {{\n{arms}        _ => None,\n    }}\n}}\n"
        ),
    )
    .expect("OUT_DIR is writable");
}

/// `svg` with each `var(--name, #hex)` replaced by its `#hex`: the icons are coloured for CSS,
/// which the native SVG renderer doesn't read, and would draw black.
fn plain_colors(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let call = &rest[start..];
        let Some(end) = call.find(')') else { break };
        let fallback = call[..end].split_once(',').map(|(_, color)| color.trim());
        out.push_str(fallback.unwrap_or("currentColor"));
        rest = &call[end + 1..];
    }
    out.push_str(rest);
    out
}
