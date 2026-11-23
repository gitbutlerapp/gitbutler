//! A placeholder for content that is still loading, as but-ui-react's `SkeletonBone`.

use std::time::Duration;

use gpui::{Animation, AnimationExt as _, App, ElementId, Window, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// What a bone stands in for, as React's `shape`.
#[derive(Clone, Copy)]
enum Shape {
    /// A line of text in `class`, as wide as the text would be.
    Text { class: TextClass, width: f32 },
    /// An avatar or an icon `size` across.
    Circle { size: f32 },
}

/// A placeholder for content that is still loading, pulsing until it lands, as React's
/// `SkeletonBone`: `inverse` at a tenth, rising to 16% and back every 1.6s, still under reduced
/// motion. Draw the row as it will look, with a bone where each piece goes, so nothing moves when
/// the content arrives.
#[derive(IntoElement)]
pub struct SkeletonBone {
    id: ElementId,
    shape: Shape,
}

/// A line of text in `class` that is still loading, `width` wide: one line box of that class, with
/// a bar as tall as its font in the middle, so the text replaces it in place.
pub fn skeleton_text(id: impl Into<ElementId>, class: TextClass, width: f32) -> SkeletonBone {
    SkeletonBone {
        id: id.into(),
        shape: Shape::Text { class, width },
    }
}

/// An avatar or an icon `size` across that is still loading.
pub fn skeleton_circle(id: impl Into<ElementId>, size: f32) -> SkeletonBone {
    SkeletonBone {
        id: id.into(),
        shape: Shape::Circle { size },
    }
}

impl RenderOnce for SkeletonBone {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx);
        let bone = div().flex_none().bg(theme.inverse);
        let bone = match self.shape {
            Shape::Text { class, .. } => bone
                .w_full()
                .h(dp(class.size))
                .rounded(dp(theme.shape.control)),
            Shape::Circle { size } => bone.size(dp(size)).rounded_full(),
        };
        let bone = match cx.reduce_motion() {
            true => bone.opacity(0.1).into_any_element(),
            false => bone
                .with_animation(
                    self.id,
                    Animation::new(Duration::from_millis(1600))
                        .repeat()
                        .with_easing(gpui::pulsating_between(0.1, 0.16)),
                    |el, delta| el.opacity(delta),
                )
                .into_any_element(),
        };
        match self.shape {
            Shape::Text { class, width } => div()
                .flex_none()
                .w(dp(width))
                .h(dp((class.size * class.line_height).round()))
                .flex()
                .items_center()
                .child(bone)
                .into_any_element(),
            Shape::Circle { .. } => bone,
        }
    }
}
