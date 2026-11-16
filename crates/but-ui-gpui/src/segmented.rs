//! A choice of one, as but-ui-react's `SegmentedControl`.

use gpui::{Div, Role, Stateful, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// `segments`, each with whether it's the picked one.
pub fn segmented(
    small: bool,
    theme: &Theme,
    segments: impl IntoIterator<Item = (Stateful<Div>, bool)>,
) -> Div {
    let (text, hover, line) = (theme.text_1, theme.hover, theme.control_border);
    let segments: Vec<_> = segments.into_iter().collect();
    let picked: Vec<bool> = segments.iter().map(|(_, picked)| *picked).collect();
    h_flex()
        .flex_none()
        .h(dp(match small {
            true => theme.sizes.control_sm + 4.,
            false => theme.sizes.control,
        }))
        .rounded(dp(theme.shape.control))
        .bg(theme.bg_2)
        .map(|el| if small { el.text_12() } else { el.text_13() })
        .semibold()
        .whitespace_nowrap()
        .children(
            segments
                .into_iter()
                .enumerate()
                .flat_map(|(ix, (segment, on))| {
                    // Always there, so picking another moves nothing.
                    let before = ix.checked_sub(1).and_then(|before| picked.get(before));
                    let rule = before.map(|before| {
                        let drawn = !before && !on;
                        (div().flex_none().w(fixed(1.)).h(dp(12.)))
                            .when(drawn, |el| el.bg(theme.border_2))
                            .into_any_element()
                    });
                    let segment = tab_stop(segment, theme).role(Role::Button);
                    let segment = (segment.h_full().flex().items_center())
                        .px(dp(if small { 7. } else { 9. } + theme.sizes.padding))
                        .rounded(dp(theme.shape.control))
                        .border_1()
                        .border_color(if on { line } else { gpui::transparent_black() })
                        .text_color(if on { text } else { theme.text_2 })
                        .when(on, |el| el.bg(theme.bg_1))
                        .when(!on, |el| el.hover(move |el| el.bg(hover).text_color(text)));
                    rule.into_iter().chain([segment.into_any_element()])
                }),
        )
}
