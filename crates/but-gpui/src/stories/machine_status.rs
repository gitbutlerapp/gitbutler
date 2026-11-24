use gpui::{Context, Window, prelude::*};

use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::{Text, dp, h_flex, machine_status, v_flex};

const MACHINES: [(&str, bool, Option<&str>); 3] = [
    ("macbook", true, None),
    ("closet-pc", true, None),
    ("old-mac", false, Some("2 days ago")),
];

/// The stories of `MachineStatus.stories.tsx`.
pub(super) const EXAMPLES: [Example; 2] = [
    Example {
        id: "components-machinestatus--default",
        name: "Default",
        draw: |theme| machine_status("status", true, theme).into_any_element(),
    },
    Example {
        id: "components-machinestatus--machines",
        name: "Machines",
        draw: |theme| {
            v_flex()
                .gap(dp(8.))
                .children(MACHINES.map(|(name, online, seen)| {
                    let status = machine_status(name, online, theme);
                    h_flex()
                        .gap(dp(6.))
                        .text_14()
                        .text_color(theme.foreground)
                        .child(name)
                        .child(match seen {
                            Some(seen) => status
                                .tooltip(crate::ui::Tooltip::text(format!("Last seen {seen}"))),
                            None => status,
                        })
                }))
                .into_any_element()
        },
    },
];

pub struct MachineStatusStory;

impl Render for MachineStatusStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(EXAMPLES.into_iter().map(|e| example(e, theme)))
    }
}
