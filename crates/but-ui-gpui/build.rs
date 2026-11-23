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
    file_icons(&out);
}

/// The file-type icons from the shared UI package, so the app draws the same marks as but.dev:
/// a `match` from each icon's name to its SVG.
fn file_icons(out: &str) {
    let dir = Path::new("../../packages/but-ui-react/src/file-icons");
    println!("cargo:rerun-if-changed={}", dir.display());
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
    color_fields(out, &data);
    text_tokens(out);
}

/// `ColorTokens`, the theme's colours: a field for each of but-ui's semantic tokens, named as
/// the token and described as it is in `semantic.tokens.json`, in that file's order.
fn color_fields(out: &Path, data: &serde_json::Value) {
    let path = Path::new("../../packages/but-ui/tokens/json/semantic.tokens.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let semantic =
        fs::read_to_string(path).expect("but-ui's semantic tokens are in packages/but-ui");
    let semantic: serde_json::Value =
        serde_json::from_str(&semantic).expect("semantic.tokens.json is JSON");
    let mut found = Vec::new();
    semantic_tokens(&semantic["semantic"], &mut Vec::new(), &mut found);
    let (mut fields, mut filled, mut named) = (String::new(), String::new(), String::new());
    for (token, description) in &found {
        assert!(
            data["colors"][token].is_object(),
            "tokens.json has the semantic colour {token}"
        );
        let field = token.replace('-', "_");
        let constant = field.to_uppercase();
        for line in description.lines() {
            let line = line
                .replace('[', "\\[")
                .replace(']', "\\]")
                .replace('<', "\\<");
            fields += &format!("    /// {}\n", line.trim());
        }
        fields += &format!("    pub {field}: Hsla,\n");
        filled += &format!("            {field}: colour({constant:?}, tokens::{constant}),\n");
        named += &format!("            {token:?} => Some(&mut self.{field}),\n");
    }
    let generated = format!(
        "/// but-ui's semantic colours, a field for each token, named as the token: `muted-foreground`\n\
         /// is `muted_foreground`. [`Theme`] derefs to them.\n\
         #[derive(Debug, Clone, Copy, PartialEq)]\n\
         pub struct ColorTokens {{\n{fields}}}\n\n\
         impl ColorTokens {{\n\
         \x20   /// Each token as `colour` draws it, from its constant's name and `[light, dark]` value.\n\
         \x20   fn from_tokens(colour: impl Fn(&str, [u32; 2]) -> Hsla) -> Self {{\n\
         \x20       Self {{\n{filled}        }}\n\
         \x20   }}\n\n\
         \x20   /// The token as but-ui names it, `primary-soft`.\n\
         \x20   fn named(&mut self, token: &str) -> Option<&mut Hsla> {{\n\
         \x20       match token {{\n{named}            _ => None,\n        }}\n\
         \x20   }}\n\
         }}\n"
    );
    fs::write(out.join("color_fields.rs"), generated).expect("OUT_DIR is writable");
}

/// Each token under `node` with its description, `muted` before `muted-nested`, as the
/// Figma export nests them: a group's own colour is its `$root`.
fn semantic_tokens(
    node: &serde_json::Value,
    at: &mut Vec<String>,
    found: &mut Vec<(String, String)>,
) {
    let Some(node) = node.as_object() else { return };
    if node.contains_key("$type") {
        let description = node["$description"].as_str().unwrap_or_default();
        found.push((at.join("-"), description.to_owned()));
        return;
    }
    for (key, child) in node {
        match key.as_str() {
            "$root" => semantic_tokens(child, at, found),
            key if key.starts_with('$') => {}
            key => {
                at.push(key.to_owned());
                semantic_tokens(child, at, found);
                at.pop();
            }
        }
    }
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
        &[
            "radius-xs",
            "radius-sm",
            "radius-control",
            "radius-card",
            "radius-popup",
        ],
    );
    let densities = steps(
        "density",
        &[
            "size-control",
            "size-control-md",
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
        "pub const CORNERS: &[(&str, [f32; 5])] = &[\n{corners}];\n\
         pub const DENSITIES: &[(&str, [f32; 5])] = &[\n{densities}];\n\
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
