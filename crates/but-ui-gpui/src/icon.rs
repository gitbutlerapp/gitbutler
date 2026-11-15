//! The icons, compiled in from but-ui-react, as but-ui-react's `Icon`.

use std::borrow::Cow;

use gpui::AssetSource;

use gpui::{SharedString, Svg, prelude::*, svg};

use crate::*;

/// Every icon, compiled in from `packages/but-ui-react` so they're the ones but.dev draws, and
/// a build fails if one goes. The client draws none of its own: a glyph it needs is drawn in
/// 🍑 But UI and exported there, so the two can't drift.
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
    ArrowDoubleDown => "arrow-double-down",
    ArrowDown => "arrow-down",
    ArrowInBox => "arrow-in-box",
    ArrowLineDown => "arrow-line-down",
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
    Commit => "commit",
    CommitFill => "commit-fill",
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
    Fork => "fork",
    Globe => "globe",
    Hash => "hash",
    Kebab => "kebab",
    Keyboard => "keyboard",
    Lab => "lab",
    Link => "link",
    Lock => "lock",
    Minus => "minus",
    Mixer => "mixer",
    Plug => "plug",
    Plus => "plus",
    PopOut => "pop-out",
    Pr => "pr",
    PrClose => "pr-close",
    PrDraft => "pr-draft",
    Refresh => "refresh",
    Repo => "repo",
    Search => "search",
    Settings => "settings",
    SplitBottom => "split-bottom",
    SplitRight => "split-right",
    SidebarNarrow => "sidebar-narrow",
    Stack => "stack",
    Tag => "tag",
    TextBlock => "text-block",
    TextCode => "text-code",
    TextWrap => "text-wrap",
    Tick => "tick",
    Undo => "undo",
    Warning => "warning",
}

/// How wide the icons are drawn in Figma, and their viewBox.
const ICON_PX: f32 = 16.;

impl Icon {
    /// The icon's path when it's drawn `px` across: its own at the size it's designed at, and
    /// with the size on the end otherwise, for [`Assets`] to keep its strokes as wide.
    pub fn path_at(self, px: f32) -> SharedString {
        if px == ICON_PX {
            self.path().into()
        } else {
            format!("{}@{px}", self.path()).into()
        }
    }
}

/// Serves the icons compiled into the binary, those asked for at a size (`icons/x.svg@12`) with
/// their strokes kept as wide (see `keep_strokes`).
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

/// `svg` with each stroke `vector-effect="non-scaling-stroke"` marks rewidened so that drawn
/// `px` across, it's as wide as at the viewBox's own size, as a browser keeps it. GPUI's
/// renderer ignores `vector-effect` and scales every stroke with the icon, so a 1.5px stroke
/// would be 1.125px in a 12px icon.
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

/// One tag of an icon, its stroke `scale` times as wide if it's kept from scaling.
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
    // A stroke without a width is 1 wide; a tag kept that has no stroke only fills.
    let stroked = (tag.split_once(r#" stroke=""#)).is_some_and(|(_, v)| !v.starts_with("none\""));
    if stroked {
        Cow::Owned(tag.replacen(KEPT, &format!(r#" stroke-width="{scale}"{KEPT}"#), 1))
    } else {
        Cow::Borrowed(tag)
    }
}

/// The width of the icon's viewBox, what its strokes are measured against.
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
        // 16px across a 16-wide viewBox is 1:1, so a 12px icon's strokes are 16/12 as wide in
        // its viewBox; one without a width is 1 wide; fills and strokes that scale stay.
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
        // `Button` draws its heights from `theme.sizes`, which density moves, so they must stay
        // the size tokens but-ui's button tokens name.
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
