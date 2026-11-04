use gpui::{Context, Window, div, prelude::*};

use super::{framed, page, section, variant};
use crate::theme::Theme;
use crate::ui::{
    ButtonStyle, blink_once, button, dot, dp, glow_once, h_flex, list_row, pulsing, spinning,
    v_flex,
};

/// What moves while work is under way, and the washes that mark a row once.
#[derive(Default)]
pub struct ActivityStory {
    /// How many times the washes were asked again: each a new id, so each starts over.
    replays: usize,
}

impl Render for ActivityStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let n = self.replays;
        let rows = v_flex()
            .w(dp(240.))
            .py(dp(6.))
            .rounded(dp(6.))
            .bg(theme.bg_2)
            .child(
                list_row("glow-row", false, theme)
                    .relative()
                    .child(glow_once(("glow", n), theme))
                    .child("An agent's commit, just in"),
            )
            .child(
                list_row("blink-row", false, theme)
                    .relative()
                    .child(blink_once(
                        ("blink", n),
                        div().absolute().inset_0().rounded(dp(4.)),
                        theme.pop,
                    ))
                    .child("What y just copied"),
            );
        let replay = button("replay", "Play again")
            .small()
            .style(ButtonStyle::Outline)
            .on_click(cx.listener(|this, _, _, cx| {
                this.replays += 1;
                cx.notify();
            }));
        page([
            section(
                "Under way",
                theme,
                [
                    variant("Spinning", theme, spinning(theme.text_2, true)),
                    variant("Pulsing", theme, pulsing(dot(theme.pop))),
                ],
            ),
            section(
                "Once, on a row",
                theme,
                [framed(
                    "Glow, then blink",
                    theme,
                    v_flex()
                        .gap(dp(10.))
                        .child(rows)
                        .child(h_flex().child(replay)),
                )],
            ),
        ])
    }
}
