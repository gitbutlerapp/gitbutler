//! Compiles in GitButler's file-type icons from the shared UI package, so this app draws the
//! same marks as but.dev: a `match` from each icon's name to its SVG. Compiles in but-ui's
//! colour tokens the same way, so the theme draws in the colours but.dev does. On Windows, also
//! gives the executable the desktop app's icon and a larger main-thread stack.
#![expect(
    clippy::disallowed_methods,
    reason = "it runs while building, with no UI to block"
)]

use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new("../../packages/but-ui-react/src/file-icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let out = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let mut names: Vec<String> = fs::read_dir(dir.join("svg"))
        .expect("the file icons are in packages/but-ui-react")
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
    color_tokens(Path::new(&out));
    windows_exe(Path::new(&out));
}

/// Write but-ui's tokens as constants, from the data its build resolves them to: each colour
/// `[light, dark]` as `0xRRGGBBAA`, named as the token is, `primary-text` as `PRIMARY_TEXT`, and
/// each length in px.
/// A token the theme reads that but-ui drops then fails the build.
fn color_tokens(out: &Path) {
    let path = Path::new("../../packages/but-ui/tokens/tokens.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let data = fs::read_to_string(path).expect("but-ui's tokens are in packages/but-ui");
    let data: serde_json::Value = serde_json::from_str(&data).expect("tokens.json is JSON");
    let consts: String = (data["colors"].as_object())
        .expect("tokens.json has colors")
        .iter()
        .map(|(token, modes)| {
            let name = token.to_uppercase().replace('-', "_");
            let [light, dark] = ["light", "dark"].map(|mode| {
                let hex = modes[mode].as_str().and_then(rgba);
                hex.unwrap_or_else(|| panic!("{token} has a {mode} colour as #rrggbb or #rrggbbaa"))
            });
            format!("pub const {name}: [u32; 2] = [{light:#010x}, {dark:#010x}];\n")
        })
        .collect::<String>()
        + &lengths(&data)
        + &accents(&data)
        + &grounds(&data)
        + &shapes(&data);
    fs::write(out.join("color_tokens.rs"), consts).expect("OUT_DIR is writable");
}

/// but-ui's lengths, in px as designed, named as the token is: `radius-card` as `RADIUS_CARD`.
fn lengths(tokens: &serde_json::Value) -> String {
    (tokens["lengths"].as_object())
        .expect("tokens.json has lengths")
        .iter()
        .map(|(token, px)| {
            let name = token.to_uppercase().replace('-', "_");
            let px = px
                .as_f64()
                .unwrap_or_else(|| panic!("{token} is a number of px")) as f32;
            format!("pub const {name}: f32 = {px:?};\n")
        })
        .collect()
}

/// but-ui's settings, as its build writes them.
fn settings() -> serde_json::Value {
    let path = Path::new("../../packages/but-ui/tokens/settings.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let settings = fs::read_to_string(path).expect("but-ui's settings are in packages/but-ui");
    serde_json::from_str(&settings).expect("settings.json is JSON")
}

/// `#rrggbb` or `#rrggbbaa` for `light` and `dark` as Rust's `[light, dark]`.
fn modes(token: &str, modes: &serde_json::Value) -> String {
    let [light, dark] = ["light", "dark"].map(|mode| {
        (modes[mode].as_str().and_then(rgba))
            .unwrap_or_else(|| panic!("{token} has a {mode} colour as #rrggbb or #rrggbbaa"))
    });
    format!("[{light:#010x}, {dark:#010x}]")
}

/// The grounds a person can pick, each as the colours that come out differently on it than on
/// gray, named as their constants are.
fn grounds(tokens: &serde_json::Value) -> String {
    let grounds: String = (tokens["grounds"].as_object())
        .expect("tokens.json has grounds")
        .iter()
        .map(|(ground, colors)| {
            let colors: String = (colors.as_object())
                .expect("a ground has its colours")
                .iter()
                .map(|(token, value)| {
                    let name = token.to_uppercase().replace('-', "_");
                    format!("(\"{name}\", {}), ", modes(token, value))
                })
                .collect();
            format!("    (\"{ground}\", &[{colors}]),\n")
        })
        .collect();
    format!("pub const GROUNDS: &[(&str, Family)] = &[\n{grounds}];\n")
}

/// Each corner setting as the shape's roles `[xs, control, card, popup]`, which follow the steps
/// the roles name; each density as `[control, control-sm, bar, padding]`; and the presets as
/// `(id, name, corners, density)`.
fn shapes(tokens: &serde_json::Value) -> String {
    let settings = settings();
    let steps = |setting: &str, names: &[&str]| -> String {
        (settings[setting]["values"].as_object())
            .unwrap_or_else(|| panic!("settings.json has {setting}"))
            .iter()
            .map(|(choice, values)| {
                let px: Vec<String> = (names.iter())
                    .map(|&token| {
                        let step = tokens["aliases"][token].as_str().unwrap_or(token);
                        let px = (values[step].as_f64())
                            .unwrap_or_else(|| panic!("{choice} {setting} sets {step}"));
                        format!("{:?}", px as f32)
                    })
                    .collect();
                format!("    (\"{choice}\", [{}]),\n", px.join(", "))
            })
            .collect()
    };
    let corners = steps(
        "radius",
        &["radius-xs", "radius-control", "radius-card", "radius-popup"],
    );
    let densities = steps(
        "density",
        &[
            "size-control",
            "size-control-sm",
            "size-bar",
            "density-padding",
        ],
    );
    let path = Path::new("../../packages/but-ui/presets.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let presets = fs::read_to_string(path).expect("but-ui's presets are in packages/but-ui");
    let presets: serde_json::Value = serde_json::from_str(&presets).expect("presets.json is JSON");
    let presets: String = (presets.as_array())
        .expect("presets.json is a list")
        .iter()
        .map(|preset| {
            let field = |key: &str| preset[key].as_str().expect("a preset names each field");
            format!(
                "    (\"{}\", \"{}\", \"{}\", \"{}\"),\n",
                field("id"),
                field("name"),
                field("radius"),
                field("density")
            )
        })
        .collect();
    format!(
        "pub const CORNERS: &[(&str, [f32; 4])] = &[\n{corners}];\n\
         pub const DENSITIES: &[(&str, [f32; 4])] = &[\n{densities}];\n\
         pub const PRESETS: &[(&str, &str, &str, &str)] = &[\n{presets}];\n"
    )
}

/// The accents a person can pick, from but-ui's settings: each named one's primary family as
/// `[light, dark]`, resolved through `tokens`, and the numbers the any-colour one is drawn by.
fn accents(tokens: &serde_json::Value) -> String {
    let settings = settings();
    let accent = &settings["accent"];
    let color = |token: &str| -> String {
        let modes = &tokens["colors"][token];
        let [light, dark] = ["light", "dark"].map(|mode| {
            (modes[mode].as_str().and_then(rgba))
                .unwrap_or_else(|| panic!("{token} has a {mode} colour in tokens.json"))
        });
        format!("[{light:#010x}, {dark:#010x}]")
    };
    let named: String = (accent["values"].as_object())
        .expect("the accent setting has values")
        .iter()
        .map(|(name, family)| {
            let family: String = (family.as_object())
                .expect("an accent names a step for each token")
                .iter()
                .map(|(token, step)| {
                    let step = step.as_str().expect("an accent's token names a ramp step");
                    format!("(\"{token}\", {}), ", color(step))
                })
                .collect();
            format!("    (\"{name}\", &[{family}]),\n")
        })
        .collect();
    let custom = &accent["custom"];
    let drawn: String = (custom["tokens"].as_object())
        .expect("the custom accent has tokens")
        .iter()
        .map(|(token, modes)| {
            let [light, dark] = ["light", "dark"].map(|mode| {
                let number = |key: &str| {
                    (modes[mode][key].as_f64())
                        .unwrap_or_else(|| panic!("{token} has a {mode} {key}"))
                };
                format!(
                    "({:?}, {:?})",
                    number("lightness") as f32,
                    number("chroma") as f32
                )
            });
            format!("(\"{token}\", [{light}, {dark}]), ")
        })
        .collect();
    let fallback = custom["fallback"]
        .as_str()
        .expect("the custom accent has a fallback");
    let cap = custom["chromaCap"]
        .as_f64()
        .expect("the custom accent has a chroma cap") as f32;
    format!(
        "/// An accent's primary family: each token as `[light, dark]`.\n\
         pub type Family = &'static [(&'static str, [u32; 2])];\n\
         pub const ACCENTS: &[(&str, Family)] = &[\n{named}];\n\
         pub const CUSTOM_ACCENT: &[(&str, [(f32, f32); 2])] = &[{drawn}];\n\
         pub const CUSTOM_ACCENT_FALLBACK: [u32; 2] = {};\n\
         pub const CUSTOM_ACCENT_CHROMA_CAP: f32 = {cap:?};\n",
        color(fallback)
    )
}

/// `#rrggbb` or `#rrggbbaa` as `0xRRGGBBAA`, opaque when it has no alpha.
fn rgba(hex: &str) -> Option<u32> {
    let digits = hex.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(value << 8 | 0xff),
        8 => Some(value),
        _ => None,
    }
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
