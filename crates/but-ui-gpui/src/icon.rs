//! The icons, compiled in from but-ui-react, as but-ui-react's `Icon`.

use std::borrow::Cow;

use gpui::AssetSource;

use gpui::{SharedString, Svg, prelude::*, svg};

use crate::*;

/// The icons of `packages/but-ui-react`, so they're the ones but.dev draws. A glyph the client
/// needs is drawn in 🍑 But UI and exported there, never added here.
macro_rules! icons {
    ($($variant:ident => $name:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon {
            $($variant,)*
        }

        impl Icon {
            pub const ALL: &[Icon] = &[$(Icon::$variant,)*];

            pub fn path(self) -> &'static str {
                match self {
                    $(Icon::$variant => concat!("icons/", $name, ".svg"),)*
                }
            }
        }

        fn icon_bytes(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!(
                    "../../../packages/but-ui-react/src/icons/", $name, ".svg"
                ))),)*
                _ => None,
            }
        }
    };
}

icons! {
    Absorb => "absorb",
    Ai => "ai",
    Anchor => "anchor",
    ArrowDoubleDown => "arrow-double-down",
    ArrowDown => "arrow-down",
    ArrowInBox => "arrow-in-box",
    ArrowLineDown => "arrow-line-down",
    ArrowRight => "arrow-right",
    ArrowUp => "arrow-up",
    ArrowUpRight => "arrow-up-right",
    Bin => "bin",
    Branch => "branch",
    BranchMerge => "branch-merge",
    BulletDisc => "bullet-disc",
    Burger => "burger",
    Camera => "camera",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronUp => "chevron-up",
    ChevronRight => "chevron-right",
    Clock => "clock",
    Contrast => "contrast",
    Copy => "copy",
    Cpu => "cpu",
    CpuDanger => "cpu-danger",
    Commit => "commit",
    CommitFill => "commit-fill",
    Danger => "danger",
    Diff => "diff",
    Discord => "discord",
    Docs => "docs",
    DragVertical => "drag-vertical",
    Desktop => "desktop",
    Edit => "edit",
    Eye => "eye",
    EyeClosed => "eye-closed",
    CollapseVertical => "collapse-vertical",
    Cross => "cross",
    ExpandVertical => "expand-vertical",
    File => "file",
    Filter => "filter",
    FileDiff => "file-diff",
    FilesSidebar => "files-sidebar",
    Folder => "folder",
    FullscreenEnter => "fullscreen-enter",
    FullscreenExit => "fullscreen-exit",
    FolderCopy => "folder-copy",
    FolderFork => "folder-fork",
    Fork => "fork",
    Globe => "globe",
    Hash => "hash",
    Info => "info",
    Kebab => "kebab",
    Keyboard => "keyboard",
    Lab => "lab",
    Laptop => "laptop",
    Link => "link",
    Lock => "lock",
    Minus => "minus",
    Mixer => "mixer",
    NoSignal => "no-signal",
    Plug => "plug",
    Pending => "pending",
    Plus => "plus",
    PopOut => "pop-out",
    Pr => "pr",
    PrClose => "pr-close",
    PrDraft => "pr-draft",
    Question => "question",
    Refresh => "refresh",
    Repo => "repo",
    Robot => "robot",
    Search => "search",
    Settings => "settings",
    Signal => "signal",
    SplitBottom => "split-bottom",
    SplitRight => "split-right",
    SidebarNarrow => "sidebar-narrow",
    Spinner => "spinner",
    Stack => "stack",
    Tag => "tag",
    TextBlock => "text-block",
    TextCode => "text-code",
    TextWrap => "text-wrap",
    Terminal => "terminal",
    Tick => "tick",
    Undo => "undo",
    Warning => "warning",
}

/// The size the icons are drawn at in Figma, and their viewBox.
const ICON_PX: f32 = 16.;

impl Icon {
    /// Off its designed size, the path carries the size for [`Assets`] to keep its strokes as wide.
    pub fn path_at(self, px: f32) -> SharedString {
        if px == ICON_PX {
            self.path().into()
        } else {
            format!("{}@{px}", self.path()).into()
        }
    }
}

/// Serves the compiled-in icons, those asked for at a size (`icons/x.svg@12`) through
/// `keep_strokes`.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let Some((path, px)) = path.split_once('@') else {
            return Ok(icon_bytes(path).map(Cow::Borrowed));
        };
        let px: f32 = px.parse()?;
        let svg = icon_bytes(path).map(std::str::from_utf8).transpose()?;
        Ok(svg.map(|svg| Cow::Owned(keep_strokes(svg, px).into_bytes())))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// Rewidens strokes marked `vector-effect="non-scaling-stroke"` for an icon drawn `px` across,
/// as a browser keeps them: GPUI's renderer ignores `vector-effect`.
fn keep_strokes(svg: &str, px: f32) -> String {
    let Some(scale) = view_box_width(svg).map(|width| width / px) else {
        return svg.to_owned();
    };
    if scale == 1. {
        return svg.to_owned();
    }
    let mut out = String::with_capacity(svg.len() + 64);
    let mut rest = svg;
    while let Some((text, from)) = rest.split_once('<') {
        out.push_str(text);
        out.push('<');
        let Some((tag, after)) = from.split_once('>') else {
            out.push_str(from);
            return out;
        };
        out.push_str(&scaled_stroke(tag, scale));
        out.push('>');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn scaled_stroke(tag: &str, scale: f32) -> Cow<'_, str> {
    const KEPT: &str = r#" vector-effect="non-scaling-stroke""#;
    if !tag.contains(KEPT) {
        return Cow::Borrowed(tag);
    }
    if let Some((before, after)) = tag.split_once(r#" stroke-width=""#)
        && let Some((width, after)) = after.split_once('"')
        && let Ok(width) = width.parse::<f32>()
    {
        return Cow::Owned(format!(
            r#"{before} stroke-width="{}"{after}"#,
            width * scale
        ));
    }
    // An unset stroke width is 1; a tag with no stroke only fills.
    let stroked = (tag.split_once(r#" stroke=""#)).is_some_and(|(_, v)| !v.starts_with("none\""));
    if stroked {
        Cow::Owned(tag.replacen(KEPT, &format!(r#" stroke-width="{scale}"{KEPT}"#), 1))
    } else {
        Cow::Borrowed(tag)
    }
}

fn view_box_width(svg: &str) -> Option<f32> {
    let (_, rest) = svg.split_once(r#"viewBox=""#)?;
    let (values, _) = rest.split_once('"')?;
    values.split_whitespace().nth(2)?.parse().ok()
}

/// `icon` drawn `px` across, its strokes as wide as at 16px.
pub fn icon_at(icon: Icon, px: f32) -> Svg {
    svg().path(icon.path_at(px)).size(dp(px)).flex_none()
}

pub fn icon(icon: Icon) -> Svg {
    icon_at(icon, ICON_PX)
}

pub fn small_icon(icon: Icon) -> Svg {
    icon_at(icon, 12.)
}

#[cfg(test)]
mod tests {
    use crate::Icon;
    use crate::icon::keep_strokes;

    #[test]
    fn icons_keep_their_stroke_widths_at_any_size() {
        let svg = concat!(
            r#"<svg viewBox="0 0 16 16" fill="none">"#,
            r#"<path d="M1 1" stroke="currentColor" stroke-width="1.5" vector-effect="non-scaling-stroke"/>"#,
            r#"<circle r="2" fill="currentColor" stroke="currentColor" vector-effect="non-scaling-stroke"/>"#,
            r#"<path d="M2 2" fill="currentColor" vector-effect="non-scaling-stroke"/>"#,
            r#"<path d="M3 3" stroke="currentColor" stroke-width="1.5"/>"#,
            "</svg>",
        );
        // A 12px icon's kept strokes are 16/12 as wide, one without a width as if it were 1;
        // fills and strokes that scale stay.
        snapbox::assert_data_eq!(
            keep_strokes(svg, 12.),
            snapbox::str![[
                r#"<svg viewBox="0 0 16 16" fill="none"><path d="M1 1" stroke="currentColor" stroke-width="2" vector-effect="non-scaling-stroke"/><circle r="2" fill="currentColor" stroke="currentColor" stroke-width="1.3333334" vector-effect="non-scaling-stroke"/><path d="M2 2" fill="currentColor" vector-effect="non-scaling-stroke"/><path d="M3 3" stroke="currentColor" stroke-width="1.5"/></svg>"#
            ]]
        );
        assert_eq!(
            keep_strokes(svg, 16.),
            svg,
            "at its viewBox's size nothing scales"
        );
    }

    #[test]
    fn every_icon_loads_at_a_size() {
        use gpui::AssetSource as _;
        for glyph in [Icon::Branch, Icon::Camera, Icon::Burger, Icon::SplitRight] {
            let path = glyph.path_at(12.);
            let svg = super::Assets.load(&path).unwrap();
            assert!(svg.is_some(), "{path} is the icon with its size on the end");
        }
        assert_eq!(
            Icon::Branch.path_at(16.),
            Icon::Branch.path(),
            "16px is as designed"
        );
    }

    #[test]
    fn button_heights_are_the_control_heights_density_moves() {
        let tokens: serde_json::Value =
            serde_json::from_str(include_str!("../../../packages/but-ui/tokens/tokens.json"))
                .unwrap();
        for (token, size) in [
            ("button-small-height", "size-control-sm"),
            ("button-regular-height", "size-control"),
        ] {
            assert_eq!(
                tokens["aliases"][token], size,
                "{token} is {size}, which Button reads from theme.sizes"
            );
        }
    }
}
