//! A count in a row's meta, as but-ui-react's `MetaCount`.

use gpui::{Div, ElementId, Hsla, SharedString, Stateful, prelude::*};

use crate::*;

pub fn meta_count(glyph: Icon, n: usize, color: Hsla, tip: String) -> Option<Stateful<Div>> {
    let tip = SharedString::from(tip);
    (n > 0).then(|| {
        h_flex()
            .id(ElementId::Name(tip.clone()))
            .flex_none()
            .gap(dp(3.))
            .text_11()
            .text_color(color)
            .child(small_icon(glyph).text_color(color))
            .child(n.to_string())
            .aria_label(tip.clone())
            .tooltip(Tooltip::text(tip))
    })
}
