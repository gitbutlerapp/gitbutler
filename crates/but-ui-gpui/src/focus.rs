//! Tab order and the focus ring every control shares.

use gpui::{Div, MouseButton, Pixels, Stateful, prelude::*};

use crate::theme::Theme;
use crate::*;

/// `el` in the window's tab order, ringed while Tab gave it the keys, as a dialog's controls
/// are on macOS. A click leaves the keys where they were, as clicking a button does there,
/// so a list keeps its arrows and Return doesn't press it again.
pub fn tab_stop(el: Stateful<Div>, theme: &Theme) -> Stateful<Div> {
    // Inside its edge: a shadow spread outside it fills a control without a ground of its own.
    let ring = gpui::BoxShadow::new(fixed(0.), fixed(0.), theme.focus_border)
        .spread_radius(fixed(2.))
        .inset();
    shown_when_focused(el.tab_index(0))
        .focus_visible(move |el| el.shadow(vec![ring]))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
}

gpui::actions!(
    ui,
    [
        /// Scroll what Tab just focused into view, sent where the keys are.
        ShowFocused
    ]
);

/// Where the control Tab just focused was drawn, from it to the `ui::scrollbar::scrolled` list
/// it's in, as [`ShowFocused`] goes up from one to the other.
pub struct FocusedAt(pub gpui::Bounds<Pixels>);

impl gpui::Global for FocusedAt {}

/// `el`, which tells the scrolled list around it where it was drawn when Tab focuses it, so
/// the list can scroll it into view.
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
