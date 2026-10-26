//! Picking spans off an activity strip of numbered slots, as a list picks its rows: the days
//! of the project's page, the quarters of an hour of a checkout's.

use std::collections::BTreeSet;

use gpui::prelude::*;
use gpui::{
    Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Stateful, Window,
};

use super::Detail;
use crate::theme::Theme;
use crate::ui::dp;

/// What the pointer did on a strip's slot: pressed it, with shift or ⌘ (`toggle`) held or
/// not; moved onto it while pressed; or let go.
#[derive(Debug, Clone, Copy)]
pub(super) enum SlotGesture {
    Press {
        slot: i64,
        shift: bool,
        toggle: bool,
    },
    Drag(i64),
    Release,
}

/// Slots picked off a strip, none for every slot: one by a click, another added or taken away
/// by ⌘-click, and a span by shift-clicking from the one clicked before or by dragging across
/// them.
#[derive(Debug, Default)]
pub(super) struct SlotPick {
    pub(super) slots: BTreeSet<i64>,
    /// The slot a shift-click picks the span from: the one clicked before.
    anchor: Option<i64>,
    /// While a press is held: the slot the span picked runs from to the one under the pointer.
    drag_from: Option<i64>,
}

impl SlotPick {
    /// Apply `gesture`: a click picks a slot alone, or lets go of the one slot picked; ⌘ adds
    /// or takes away one; shift picks the slots from the one clicked before; holding the
    /// press, the pointer moving across them picks those it spans from where it was pressed.
    /// Whether the slots picked changed.
    pub(super) fn apply(&mut self, gesture: SlotGesture) -> bool {
        let before = self.slots.clone();
        let span = |a: i64, b: i64| (a.min(b)..=a.max(b)).collect::<BTreeSet<_>>();
        match gesture {
            SlotGesture::Press {
                slot, shift: true, ..
            } if self.anchor.is_some() => {
                let from = self.anchor.unwrap_or(slot);
                self.slots = span(from, slot);
                self.drag_from = Some(from);
            }
            SlotGesture::Press {
                slot, toggle: true, ..
            } => {
                if !self.slots.remove(&slot) {
                    self.slots.insert(slot);
                }
                self.anchor = Some(slot);
                self.drag_from = None;
            }
            SlotGesture::Press { slot, .. } => {
                // Clicking the one slot picked lets go of it, with nothing to drag from then.
                let clears = self.slots.len() == 1 && self.slots.contains(&slot);
                self.drag_from = (!clears).then_some(slot);
                self.slots = self.drag_from.into_iter().collect();
                self.anchor = Some(slot);
            }
            SlotGesture::Drag(slot) => {
                if let Some(from) = self.drag_from {
                    self.slots = span(from, slot);
                }
            }
            SlotGesture::Release => self.drag_from = None,
        }
        self.slots != before
    }

    /// Whether `slot` is picked, with every slot picked while none is.
    pub(super) fn keeps(&self, slot: i64) -> bool {
        self.slots.is_empty() || self.slots.contains(&slot)
    }
}

/// `slot`'s column of a strip, picking as it's pressed, ⌘- or shift-pressed, or dragged
/// across, `gesture` applying each to the strip's pick. A run of the slots `picked` reads as
/// one band, rounded only at its ends, and the column under the pointer lights up.
pub(super) fn picks_slot(
    column: Stateful<Div>,
    slot: i64,
    picked: &BTreeSet<i64>,
    theme: &Theme,
    gesture: impl Fn(&mut Detail, SlotGesture, &mut Context<Detail>) + Clone + 'static,
    cx: &Context<Detail>,
) -> Stateful<Div> {
    let drag = gesture.clone();
    let joined = |next: i64| picked.contains(&slot) && picked.contains(&next);
    column
        .when(!joined(slot - 1), |el| el.rounded_l(dp(3.)))
        .when(!joined(slot + 1), |el| el.rounded_r(dp(3.)))
        .when(picked.contains(&slot), |el| {
            el.bg(theme.selected_unfocused())
        })
        .hover(|el| el.bg(theme.hover()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                let (shift, toggle) = (event.modifiers.shift, event.modifiers.secondary());
                let press = SlotGesture::Press {
                    slot,
                    shift,
                    toggle,
                };
                gesture(this, press, cx)
            }),
        )
        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
            if event.pressed_button == Some(MouseButton::Left) {
                drag(this, SlotGesture::Drag(slot), cx)
            }
        }))
}

/// A strip's slots, ending a drag across them wherever the pointer's let go.
pub(super) fn ends_drag<E: InteractiveElement>(
    strip: E,
    gesture: impl Fn(&mut Detail, SlotGesture, &mut Context<Detail>) + Clone + 'static,
    cx: &Context<Detail>,
) -> E {
    let release =
        move |this: &mut Detail, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Detail>| {
            gesture(this, SlotGesture::Release, cx)
        };
    strip
        .on_mouse_up(MouseButton::Left, cx.listener(release.clone()))
        .on_mouse_up_out(MouseButton::Left, cx.listener(release))
}

#[cfg(test)]
mod tests {
    use super::{SlotGesture, SlotPick};

    #[test]
    fn slots_are_picked_by_click_cmd_click_shift_click_and_drag() {
        use SlotGesture::{Drag, Release};
        let press = |slot| SlotGesture::Press {
            slot,
            shift: false,
            toggle: false,
        };
        let shift = |slot| SlotGesture::Press {
            slot,
            shift: true,
            toggle: false,
        };
        let cmd = |slot| SlotGesture::Press {
            slot,
            shift: false,
            toggle: true,
        };
        let mut pick = SlotPick::default();
        let mut slots = |gestures: &[SlotGesture]| {
            for gesture in gestures {
                pick.apply(*gesture);
            }
            pick.slots.iter().copied().collect::<Vec<_>>()
        };
        assert_eq!(slots(&[press(10), Release]), [10], "a click picks a slot");
        assert_eq!(
            slots(&[shift(13), Release]),
            [10, 11, 12, 13],
            "shift picks the slots between"
        );
        assert_eq!(
            slots(&[cmd(20), Release, cmd(11), Release]),
            [10, 12, 13, 20],
            "⌘ adds or takes away one"
        );
        assert_eq!(
            slots(&[press(20), Drag(18), Drag(17), Release]),
            [17, 18, 19, 20],
            "dragging picks the slots crossed"
        );
        assert_eq!(slots(&[Drag(5)]), [17, 18, 19, 20], "only while pressed");
        assert_eq!(
            slots(&[press(12), Release, press(12), Drag(12), Release]),
            [] as [i64; 0],
            "clicking the one slot picked again lets go, though the pointer moves a little"
        );
    }
}
