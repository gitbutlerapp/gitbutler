//! A conflict's mark, as but-ui-react's `ConflictIcon`: the warning triangle for a conflicted
//! commit or file, filled in the warning amber, as a conflict waits to be resolved rather than
//! having failed. React's `plus` and `minus`, a conflicted hunk's two sides, aren't drawn here.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Div, ImageSource, RenderImage, Rgba, SvgRenderer, div, img, prelude::*};

use crate::dp;
use crate::file_icons::hex;
use crate::theme::Theme;

thread_local! {
    /// The mark, drawn once for each pair of colours the appearance gives it.
    static DRAWN: RefCell<HashMap<[String; 2], Arc<RenderImage>>> = RefCell::default();
}

/// `ConflictIcon.tsx`'s triangle and its mark, with the colours put in before it's drawn, as GPUI
/// can't colour an SVG's parts apart.
const TRIANGLE: &str = "M6.20252 2.82467C6.95455 1.40909 8.98295 1.40909 9.73498 2.82467L14.3765 \
     11.5617C15.0842 12.8938 14.1187 14.5 12.6103 14.5H3.32721C1.8188 14.5 0.853298 12.8938 \
     1.56098 11.5617L6.20252 2.82467Z";
const MARK: &str = "M7.96875 8.9375L7.96875 5.9375M7.96875 11.9375L7.96875 10.4375";

/// The conflict mark, `px` across.
pub fn conflict_icon(px: f32, theme: &Theme, cx: &App) -> Div {
    let colors = [theme.warn_bg, theme.on_warn].map(|color| hex(Rgba::from(color)));
    let drawn = DRAWN.with_borrow_mut(|drawn| {
        let draw = || draw(&cx.svg_renderer(), &colors);
        drawn.entry(colors.clone()).or_insert_with(draw).clone()
    });
    div()
        .flex_none()
        .size(dp(px))
        .child(img(ImageSource::Render(drawn)).size(dp(px)))
}

#[expect(
    clippy::expect_used,
    reason = "the mark is written here, and a test draws it"
)]
fn draw(renderer: &SvgRenderer, [fill, mark]: &[String; 2]) -> Arc<RenderImage> {
    let svg = format!(
        r#"<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="{TRIANGLE}" fill="{fill}"/><path d="{MARK}" stroke="{mark}" stroke-width="1.5"/></svg>"#
    );
    (renderer.render_single_frame(svg.as_bytes(), 1.)).expect("the mark draws, as a test checks")
}

#[cfg(test)]
mod tests {
    use super::draw;

    #[test]
    fn the_mark_draws() {
        let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(crate::Assets));
        draw(&renderer, &["#ff9500".to_owned(), "#100500".to_owned()]);
    }
}
