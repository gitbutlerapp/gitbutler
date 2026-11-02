use gpui::{Context, Toggled, Window, prelude::*};

use super::{page, section, variant};
use crate::theme::Theme;
use crate::ui::{checkbox, dp, h_flex, v_flex};

const FILES: [&str; 3] = ["src/app.rs", "src/theme.rs", "Cargo.toml"];

/// Files to pick, as the composer picks them, under one box for them all.
#[derive(Default)]
pub struct CheckboxStory {
    picked: [bool; FILES.len()],
}

impl Render for CheckboxStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let all = match self.picked.iter().filter(|picked| **picked).count() {
            0 => Toggled::False,
            n if n == FILES.len() => Toggled::True,
            _ => Toggled::Mixed,
        };
        let header = h_flex()
            .gap(dp(8.))
            .child(
                checkbox("all", all, theme).on_click(cx.listener(move |this, _, _, cx| {
                    this.picked = [all != Toggled::True; FILES.len()];
                    cx.notify();
                })),
            )
            .child("All files");
        let rows = (FILES.iter().zip(self.picked).enumerate()).map(|(ix, (file, picked))| {
            let check = if picked {
                Toggled::True
            } else {
                Toggled::False
            };
            h_flex()
                .gap(dp(8.))
                .pl(dp(22.))
                .child(checkbox(("file", ix), check, theme).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(picked) = this.picked.get_mut(ix) {
                            *picked = !*picked;
                        }
                        cx.notify();
                    },
                )))
                .child(*file)
        });
        page([
            section(
                "States",
                theme,
                [
                    variant("Empty", theme, checkbox("empty", Toggled::False, theme)),
                    variant("Ticked", theme, checkbox("ticked", Toggled::True, theme)),
                    variant(
                        "Some of a group",
                        theme,
                        checkbox("mixed", Toggled::Mixed, theme),
                    ),
                ],
            ),
            section(
                "Try it",
                theme,
                [v_flex()
                    .gap(dp(8.))
                    .child(header)
                    .children(rows)
                    .into_any_element()],
            ),
        ])
    }
}
