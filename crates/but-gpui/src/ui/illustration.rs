//! The design system's empty-state drawings, shared with Lite and but.dev and drawn as
//! `@gitbutler/ui-react` draws them: each at the size it was drawn, never scaled, its outline
//! in `border-1` and its faces in the theme's own grounds.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Hsla, ImageSource, Img, RenderImage, Rgba, SvgRenderer, img, prelude::*};

use super::dp;
use crate::theme::Theme;

/// Which drawing, picked by what it means, as ui-react's empty states pick them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Illustration {
    /// A list with nothing in it.
    Cactus,
    /// Looked and found nothing, or a state the app can't name.
    Papers,
    /// All good with nothing to do, in a large view only.
    Waving,
    /// The command line.
    Terminal,
}

impl Illustration {
    /// The drawing, and the width and height it was drawn at.
    fn source(self) -> (&'static str, f32, f32) {
        match self {
            Illustration::Cactus => (
                include_str!("../../../../packages/ui-react/src/illustrations/cactus.svg"),
                96.,
                82.,
            ),
            Illustration::Papers => (
                include_str!("../../../../packages/ui-react/src/illustrations/papers.svg"),
                98.,
                86.,
            ),
            Illustration::Waving => (
                include_str!("../../../../packages/ui-react/src/illustrations/waving.svg"),
                186.,
                215.,
            ),
            Illustration::Terminal => (
                include_str!("../../../../packages/ui-react/src/illustrations/terminal.svg"),
                79.,
                59.,
            ),
        }
    }
}

thread_local! {
    /// Each drawing in each theme, drawn the first time it's needed there.
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
        // Smaller, and whole, where there's no room for it: `img` draws it contained.
        .flex_shrink(1.)
        .min_h_0()
        .max_w_full()
}

/// `which` in `theme`'s colours, for the roles ui-react's drawings are made of: the outline is
/// `currentColor`, set to `border-1`; enclosed areas the paper, `bg-1`; shaded faces the ground,
/// `bg-2`; a darker face `border-2`; and what stands for text `text-2`.
#[expect(
    clippy::expect_used,
    reason = "the drawings are compiled in, and a test draws each"
)]
fn draw(renderer: &SvgRenderer, which: Illustration, theme: &Theme) -> Arc<RenderImage> {
    let (svg, ..) = which.source();
    let themed = [
        ("currentColor", theme.border_1),
        ("var(--bg-1)", theme.bg_1),
        ("var(--bg-2)", theme.bg_2),
        ("var(--border-2)", theme.border_2),
        ("var(--text-2)", theme.text_2),
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
    fn every_drawing_draws_in_both_themes() {
        use crate::theme::{Appearance, CodeSize, Theme};
        let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(crate::ui::Assets));
        for choice in [Appearance::Light, Appearance::Dark] {
            let system = gpui::WindowAppearance::Light;
            let theme = Theme::for_appearance(choice, system, CodeSize::Medium);
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
