use gpui::{Context, Div, Window, div, prelude::*};

use super::{framed, page, section};
use crate::theme::Theme;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::{ButtonStyle, button, dp, h_flex, muted, popover, v_flex};

/// The cards menus, tooltips and dialogs float on, drawn in place rather than over a
/// backdrop.
pub struct PopoverStory;

impl Render for PopoverStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page([
            section(
                "Popover",
                theme,
                [framed(
                    "What menus, toasts and tooltips float on",
                    theme,
                    popover(theme)
                        .w(dp(240.))
                        .p(dp(12.))
                        .child(muted("Anything floating goes on this card.", theme)),
                )],
            ),
            section(
                "Modal",
                theme,
                [
                    framed(
                        "Medium: a form around one task",
                        theme,
                        dialog(
                            modal(ModalSize::Medium, theme),
                            "Create a pull request",
                            Some("Create pull request"),
                            theme,
                        ),
                    ),
                    framed(
                        "Large: more than one group, as Settings",
                        theme,
                        dialog(modal(ModalSize::Large, theme), "Settings", None, theme),
                    ),
                ],
            ),
        ])
    }
}

/// `card` with a title, a line and, with a `confirm`, Cancel beside it, laid out as the pull
/// request modal is.
fn dialog(card: Div, title: &'static str, confirm: Option<&'static str>, theme: &Theme) -> Div {
    card.child(
        v_flex()
            .gap(dp(4.))
            .px(dp(20.))
            .pt(dp(18.))
            .pb(dp(12.))
            .child(
                div()
                    .text_size(dp(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(muted("What it's for, in a line.", theme)),
    )
    .when_some(confirm, |el, confirm| {
        el.child(
            h_flex()
                .justify_end()
                .gap(dp(8.))
                .px(dp(20.))
                .pb(dp(16.))
                .child(button((title, 0_usize), "Cancel"))
                .child(button((title, 1_usize), confirm).style(ButtonStyle::Pop)),
        )
    })
}
