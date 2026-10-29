//! Compiles in GitButler's file-type icons from the shared UI package, so this app draws the
//! same marks as Lite and but.dev: a `match` from each icon's name to its SVG. On Windows, also
//! gives the executable the desktop app's icon and a larger main-thread stack.
#![expect(
    clippy::disallowed_methods,
    reason = "it runs while building, with no UI to block"
)]

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
    windows_exe(Path::new(&out));
}

/// Give a Windows executable its icon as resource 1, which GPUI shows on the window and the
/// taskbar: the desktop app's for the channel `CHANNEL` names, as `but_path::AppChannel` picks it.
/// Also a main-thread stack as large as macOS's. Building for elsewhere, it adds nothing.
fn windows_exe(out: &Path) {
    println!("cargo:rerun-if-env-changed=CHANNEL");
    let channel = match env::var("CHANNEL").as_deref() {
        Ok("nightly") => "nightly",
        Ok("release") => "release",
        _ => "dev",
    };
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let crates = Path::new(&crate_dir)
        .parent()
        .expect("the crate is in crates/");
    let icon = crates
        .join("gitbutler-tauri/icons")
        .join(channel)
        .join("icon.ico");
    let rc = out.join("icon.rc");
    // UTF-8, so a checkout under a path like C:\Users\Jürgen still compiles.
    let rc_text = format!(
        "#pragma code_page(65001)\n1 ICON {:?}\n",
        icon.display().to_string()
    );
    fs::write(&rc, rc_text).expect("OUT_DIR is writable");
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .expect("the icon compiles");
    // The UI runs on the main thread, which Windows gives 1 MB of stack where macOS gives 8.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
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
