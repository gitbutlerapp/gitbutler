//! Tab order and the focus ring every control shares.

use gpui::{Div, MouseButton, Pixels, Stateful, prelude::*};

use crate::theme::Theme;
use crate::*;

/// The ring of a control the keys reached, as but-ui-react's `--but-focus-ring` with its offset:
/// 1.5px of `ring` just inside its edge.
pub fn focus_ring(theme: &Theme) -> gpui::BoxShadow {
    gpui::BoxShadow::new(fixed(0.), fixed(0.), theme.ring)
        .spread_radius(fixed(1.5))
        .inset()
}

/// A click leaves the keys where they were, as on macOS, so a list keeps its arrows and Return
/// doesn't press the control again.
pub fn tab_stop(el: Stateful<Div>, theme: &Theme) -> Stateful<Div> {
    // Inside its edge: a shadow spread outside it fills a control without a ground of its own.
    let ring = gpui::BoxShadow::new(fixed(0.), fixed(0.), theme.ring)
        .spread_radius(fixed(2.))
        .inset();
    shown_when_focused(el.tab_index(0))
        .focus_visible(move |el| el.shadow(vec![ring]))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
}

gpui::actions!(
    ui,
    [
        /// Scroll what Tab just focused into view.
        ShowFocused
    ]
);

/// Where the control Tab just focused was drawn, for the `ui::scrollbar::scrolled` list it's in.
pub struct FocusedAt(pub gpui::Bounds<Pixels>);

impl gpui::Global for FocusedAt {}

pub fn shown_when_focused(el: Stateful<Div>) -> Stateful<Div> {
    let drawn = std::rc::Rc::new(std::cell::Cell::new(None));
    let record = drawn.clone();
    el.on_action(move |_: &ShowFocused, _, cx| {
        if let Some(bounds) = drawn.get() {
            cx.set_global(FocusedAt(bounds));
        }
        cx.propagate();
    })
    .child(
        gpui::canvas(
            move |bounds, _, _| record.set(Some(bounds)),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}
