//! A snackbar, as but-ui-react's `Snackbar`.

use gpui::{AnyElement, Div, ElementId, Role, SharedString, Stateful, div, prelude::*};

use crate::theme::{Theme, tokens};
use crate::*;

/// How the news landed, which only the glyph says: the surface and the sentence stay the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnackbarVariant {
    /// Just the news: no glyph, unless one is given with [`snackbar_with_icon`].
    Info,
    /// Done, but not quite as asked.
    Warning,
    /// It failed.
    Danger,
    /// It went through.
    Safe,
}

/// A glyph (none for Info), a sentence, and at most one `action` behind a divider:
/// [`snackbar_undo`] or [`snackbar_dismiss`].
pub fn snackbar(
    id: impl Into<ElementId>,
    message: SharedString,
    variant: SnackbarVariant,
    action: Option<AnyElement>,
    theme: &Theme,
) -> Stateful<Div> {
    let icon = match variant {
        SnackbarVariant::Info => None,
        SnackbarVariant::Warning => Some(Icon::Warning),
        SnackbarVariant::Danger => Some(Icon::Danger),
        SnackbarVariant::Safe => Some(Icon::Tick),
    };
    draw(id, message, variant, icon, action, theme)
}

/// A snackbar with `icon` in place of its variant's glyph, as React's `icon`, in the variant's
/// colour: for the odd Info that needs one, or a glyph that says more than the variant's.
pub fn snackbar_with_icon(
    id: impl Into<ElementId>,
    message: SharedString,
    variant: SnackbarVariant,
    icon: Icon,
    action: Option<AnyElement>,
    theme: &Theme,
) -> Stateful<Div> {
    draw(id, message, variant, Some(icon), action, theme)
}

fn draw(
    id: impl Into<ElementId>,
    message: SharedString,
    variant: SnackbarVariant,
    icon: Option<Icon>,
    action: Option<AnyElement>,
    theme: &Theme,
) -> Stateful<Div> {
    let color = match variant {
        SnackbarVariant::Info => theme.snackbar_muted_foreground,
        SnackbarVariant::Warning => theme.derived.snackbar_warning,
        SnackbarVariant::Danger => theme.snackbar_danger,
        SnackbarVariant::Safe => theme.snackbar_success,
    };
    let glyph = icon.map(|icon| (icon, color));
    h_flex()
        .id(id)
        // A failure interrupts; the rest are there to be read whenever the reader gets to them.
        .role(match variant {
            SnackbarVariant::Danger => Role::Alert,
            SnackbarVariant::Info | SnackbarVariant::Warning | SnackbarVariant::Safe => {
                Role::Status
            }
        })
        .aria_label(message.clone())
        // but-ui's paddings, each a pixel less for the border. A sentence with no glyph before
        // it starts as far in as it ends. An action reaches into the wider end padding, as far
        // from the end as from the top and bottom.
        .h(dp(tokens::SIZE_SNACKBAR))
        .pl(dp(match glyph {
            Some(_) => tokens::PADDING_SNACKBAR_START,
            None => tokens::PADDING_SNACKBAR_END,
        } - 1.))
        .pr(dp(match action.is_some() {
            true => tokens::PADDING_SNACKBAR_BLOCK,
            false => tokens::PADDING_SNACKBAR_END,
        } - 1.))
        .gap(dp(tokens::GAP_SNACKBAR))
        .rounded(dp(theme.shape.popup))
        .border_1()
        .border_color(theme.snackbar_border)
        .bg(theme.snackbar)
        .text_color(theme.snackbar_foreground)
        .text_12()
        .shadow_lg()
        .when_some(glyph, |el, (glyph, color)| {
            el.child(icon_at(glyph, 14.).flex_none().text_color(color))
        })
        .child(div().min_w_0().truncate().child(message))
        .when_some(action, |el, action| {
            // Its lane has room only on the sentence's side, so the button sits nearer the
            // line than the words do.
            el.child(
                div()
                    .flex_none()
                    .ml(dp(3.5))
                    .mr(dp(-0.5))
                    .w(fixed(1.))
                    .h(dp(20.))
                    .bg(theme.snackbar_divider),
            )
            .child(action)
        })
}

/// Takes back what the snackbar says happened.
pub fn snackbar_undo(id: impl Into<ElementId>, theme: &Theme) -> Button {
    snackbar_action(button(id, "Undo").icon(Icon::Undo), theme)
}

/// Closes a snackbar that waits to be dismissed by hand.
pub fn snackbar_dismiss(id: impl Into<ElementId>, theme: &Theme) -> Button {
    snackbar_action(icon_button(id, Icon::Cross).tooltip("Dismiss"), theme)
}

/// A small ghost button in the snackbar's own colours, which stay dark in both themes.
fn snackbar_action(button: Button, theme: &Theme) -> Button {
    button
        .small()
        .style(ButtonStyle::Ghost)
        .ink(theme.snackbar_foreground, theme.snackbar_hover)
}
