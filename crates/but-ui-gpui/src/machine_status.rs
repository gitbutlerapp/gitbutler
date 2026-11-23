//! Whether a machine is online, beside its name, as but-ui-react's `MachineStatus`.

use gpui::{Div, ElementId, Stateful, prelude::*};

use crate::theme::Theme;
use crate::*;

/// A signal on green while the machine is `online`, a crossed-out signal on grey while it isn't,
/// in a 16px circle. Say more, such as when it was last seen, in a tooltip on it.
pub fn machine_status(id: impl Into<ElementId>, online: bool, theme: &Theme) -> Stateful<Div> {
    let (glyph, ground, colour) = match online {
        true => (Icon::Signal, theme.success_soft, theme.success_text),
        false => (Icon::NoSignal, theme.neutral_soft, theme.subtle_foreground),
    };
    h_flex()
        .id(id)
        .flex_none()
        .size(dp(16.))
        .justify_center()
        .rounded_full()
        .bg(ground)
        .child(icon_at(glyph, 12.).text_color(colour))
        .role(gpui::Role::Image)
        .aria_label(match online {
            true => "Online",
            false => "Offline",
        })
}
