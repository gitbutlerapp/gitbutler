//! On Windows, gives the executable the desktop app's icon and a larger main-thread stack.
//! but-ui-gpui compiles in the colour tokens and the file-type icons.
#![expect(
    clippy::disallowed_methods,
    reason = "it runs while building, with no UI to block"
)]

use std::{env, fs, path::Path};

fn main() {
    let out = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
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
