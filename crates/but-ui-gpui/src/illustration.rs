//! The design system's empty-state drawings, as but-ui-react's `Illustration`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Hsla, ImageSource, Img, RenderImage, Rgba, SvgRenderer, img, prelude::*};

use crate::dp;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Illustration {
    /// A list with nothing in it.
    Cactus,
    /// Looked and found nothing, or a state the app can't name.
    Papers,
    /// All good with nothing to do, in a large view only.
    Waving,
    Terminal,
}

impl Illustration {
    /// The SVG, and the width and height it was drawn at.
    fn source(self) -> (&'static str, f32, f32) {
        match self {
            Illustration::Cactus => (
                include_str!("../../../packages/but-ui-react/src/illustrations/cactus.svg"),
                96.,
                82.,
            ),
            Illustration::Papers => (
                include_str!("../../../packages/but-ui-react/src/illustrations/papers.svg"),
                98.,
                86.,
            ),
            Illustration::Waving => (
                include_str!("../../../packages/but-ui-react/src/illustrations/waving.svg"),
                186.,
                215.,
            ),
            Illustration::Terminal => (
                include_str!("../../../packages/but-ui-react/src/illustrations/terminal.svg"),
                79.,
                59.,
            ),
        }
    }
}

thread_local! {
    /// Each drawing per theme, keyed by the theme's ground.
    static DRAWN: RefCell<HashMap<(Illustration, u32), Arc<RenderImage>>> = RefCell::default();
}

pub fn illustration(which: Illustration, theme: &Theme, cx: &App) -> Img {
    let (_, width, height) = which.source();
    let drawn = DRAWN.with_borrow_mut(|drawn| {
        let draw = || draw(&cx.svg_renderer(), which, theme);
        drawn
            .entry((which, rgb(theme.bg_1)))
            .or_insert_with(draw)
            .clone()
    });
    img(ImageSource::Render(drawn))
        .w(dp(width))
        .h(dp(height))
        // `img` draws it contained, so it shrinks whole where there's no room.
        .flex_shrink(1.)
        .min_h_0()
        .max_w_full()
}

/// `which` with the CSS colours but-ui-react styles its drawings by swapped for `theme`'s; there,
/// `currentColor` is `--but-border-hover`.
#[expect(
    clippy::expect_used,
    reason = "the drawings are compiled in, and a test draws each"
)]
fn draw(renderer: &SvgRenderer, which: Illustration, theme: &Theme) -> Arc<RenderImage> {
    let (svg, ..) = which.source();
    let themed = [
        ("currentColor", theme.border_1),
        ("var(--but-background)", theme.bg_1),
        ("var(--but-muted)", theme.bg_2),
        ("var(--but-border)", theme.border_2),
        ("var(--but-muted-foreground)", theme.text_2),
    ]
    .into_iter()
    .fold(svg.to_owned(), |svg, (role, color)| {
        svg.replace(role, &format!("#{:06x}", rgb(color)))
    });
    (renderer.render_single_frame(themed.as_bytes(), 1.))
        .expect("the design system's drawings draw, as a test checks")
}

fn rgb(color: Hsla) -> u32 {
    let Rgba { r, g, b, .. } = color.into();
    let channel = |c: f32| (c * 255.).round() as u32;
    channel(r) << 16 | channel(g) << 8 | channel(b)
}

#[cfg(test)]
mod tests {
    use super::{Illustration, draw};

    #[test]
    fn every_drawing_draws_in_every_theme() {
        use crate::theme::{CodeSize, ColorScheme, Shade, Theme};
        let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(crate::Assets));
        let rgb = |r, g, b| but_tty::RgbColor { r, g, b };
        let dark_scheme = but_tty::Colors {
            background: Some(rgb(0x1e, 0x1e, 0x2e)),
            foreground: Some(rgb(0xcd, 0xd6, 0xf4)),
            ..Default::default()
        };
        let ghostty = but_tty::Ghostty {
            light: Default::default(),
            dark: dark_scheme,
            ..but_tty::Ghostty::default()
        };
        for (choice, scheme) in [
            (Shade::Light, ColorScheme::GitButler),
            (Shade::Dark, ColorScheme::GitButler),
            (Shade::Dark, ColorScheme::Ghostty),
        ] {
            let theme = Theme::new(choice, &scheme, CodeSize::Medium, &ghostty);
            for which in [
                Illustration::Cactus,
                Illustration::Papers,
                Illustration::Waving,
                Illustration::Terminal,
            ] {
                draw(&renderer, which, &theme);
            }
        }
    }
}
