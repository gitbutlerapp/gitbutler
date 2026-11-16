//! Compiles in but-ui's tokens (`packages/but-ui/tokens`) and its type scale (`styles/text.css`)
//! as constants, so the theme draws in the colours and sizes but.dev does.
#![expect(
    clippy::disallowed_methods,
    reason = "it runs while building, with no UI to block"
)]

use std::{env, fs, path::Path};

fn main() {
    let out = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    color_tokens(Path::new(&out));
}

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
    text_tokens(out);
}

/// The type scale, from but-ui's text classes (`styles/text.css`, `.text-13`) and the font tokens
/// they read (`tokens/tokens.css`, `--but-font-weight-regular`): each base class as a `TextClass`,
/// and each weight and line height as a constant, for `text.rs`.
fn text_tokens(out: &Path) {
    let [css, tokens] = ["styles/text.css", "tokens/tokens.css"].map(|file| {
        let path = Path::new("../../packages/but-ui").join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        fs::read_to_string(&path).unwrap_or_else(|_| panic!("but-ui has {file}"))
    });
    // `--but-font-weight-regular: 460;` and `--but-font-line-height-default: 120%;`.
    let token = |prefix: &str| -> Vec<(String, f32)> {
        (tokens.lines())
            .filter_map(|line| {
                let (name, value) = line.trim().strip_prefix(prefix)?.split_once(':')?;
                let value = value.trim().trim_end_matches(';');
                let number = match value.strip_suffix('%') {
                    Some(percent) => percent.parse::<f32>().ok()? / 100.,
                    None => value.parse().ok()?,
                };
                Some((name.to_owned(), number))
            })
            .collect()
    };
    let weights = token("--but-font-weight-");
    let line_heights = token("--but-font-line-height-");
    let lookup = |table: &[(String, f32)], value: &str, what: &str| -> f32 {
        if let Some(name) = value.strip_prefix(&format!("var(--but-font-{what}-")) {
            let name = name.trim_end_matches(')');
            let found = table.iter().find(|(n, _)| n == name);
            found
                .unwrap_or_else(|| panic!("tokens.css has --but-font-{what}-{name}"))
                .1
        } else {
            value
                .parse()
                .unwrap_or_else(|_| panic!("a {what} is a token or a number: {value}"))
        }
    };
    // The base classes, up to the modifiers, each `.text-x { property: value; … }`.
    let base = css.split("/* MODIFIERS */").next().unwrap_or(&css);
    let classes: String = (base.split(".text-").skip(1))
        .filter_map(|block| {
            let (name, body) = block.split_once('{')?;
            let body = body.split('}').next()?;
            let property = |key: &str| {
                body.lines().find_map(|line| {
                    line.trim().strip_prefix(key)?.trim().strip_suffix(';').map(str::to_owned)
                })
            };
            let rem: f32 = property("font-size:")?.strip_suffix("rem")?.parse().ok()?;
            let weight = lookup(&weights, &property("font-weight:")?, "weight");
            let line_height = lookup(&line_heights, &property("line-height:")?, "line-height");
            let name = name.trim().to_uppercase().replace('-', "_");
            Some(format!(
                "pub const TEXT_{name}: TextClass = TextClass {{ size: {:?}, weight: {weight:?}, line_height: {line_height:?} }};\n",
                rem * 16.
            ))
        })
        .collect();
    let constants = |table: &[(String, f32)], prefix: &str| -> String {
        (table.iter())
            .map(|(name, value)| {
                let name = name.to_uppercase().replace('-', "_");
                format!("pub const {prefix}_{name}: f32 = {value:?};\n")
            })
            .collect()
    };
    let generated =
        classes + &constants(&weights, "FONT_WEIGHT") + &constants(&line_heights, "LINE_HEIGHT");
    fs::write(out.join("text_tokens.rs"), generated).expect("OUT_DIR is writable");
}

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

fn settings() -> serde_json::Value {
    let path = Path::new("../../packages/but-ui/tokens/settings.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let settings = fs::read_to_string(path).expect("but-ui's settings are in packages/but-ui");
    serde_json::from_str(&settings).expect("settings.json is JSON")
}

fn modes(token: &str, modes: &serde_json::Value) -> String {
    let [light, dark] = ["light", "dark"].map(|mode| {
        (modes[mode].as_str().and_then(rgba))
            .unwrap_or_else(|| panic!("{token} has a {mode} colour as #rrggbb or #rrggbbaa"))
    });
    format!("[{light:#010x}, {dark:#010x}]")
}

/// The grounds a person can pick, each with only the colours that differ from gray's.
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
                let alpha = modes[mode]["alpha"].as_f64().unwrap_or(1.);
                format!(
                    "({:?}, {:?}, {:?})",
                    number("lightness") as f32,
                    number("chroma") as f32,
                    alpha as f32
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
         /// How the custom accent draws a token: its lightness, its share of the chroma, its alpha.\n\
         pub type Drawn = (f32, f32, f32);\n\
         pub const CUSTOM_ACCENT: &[(&str, [Drawn; 2])] = &[{drawn}];\n\
         pub const CUSTOM_ACCENT_FALLBACK: [u32; 2] = {};\n\
         pub const CUSTOM_ACCENT_CHROMA_CAP: f32 = {cap:?};\n",
        color(fallback)
    )
}

fn rgba(hex: &str) -> Option<u32> {
    let digits = hex.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(value << 8 | 0xff),
        8 => Some(value),
        _ => None,
    }
}
