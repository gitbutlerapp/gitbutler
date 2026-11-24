use gpui::{AnyElement, Context, Div, Window, div, prelude::*};

use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::{
    ButtonStyle, Drawer, Fold, Icon, MetaCountType, MetaStatusVariant, SidebarRow, button, dp,
    drawer, empty_state_tidy, group_label, icon_button, meta_count, meta_status, repo_item,
    terminal_item, tree_guides, v_flex,
};

/// The card a drawer holds, on the panel's ground under its row.
fn card(theme: &Theme, children: impl IntoIterator<Item = Div>) -> Div {
    v_flex()
        .overflow_hidden()
        .border_1()
        .border_color(theme.border_subtle)
        .rounded(dp(theme.shape.card))
        .bg(theme.background)
        .children(children)
}

/// A ghost icon button at a drawer's end, regular as React's, with its tooltip.
fn ghost(id: &'static str, icon: Icon, label: &'static str) -> AnyElement {
    icon_button(id, icon).tooltip(label).into_any_element()
}

/// The stories' `TerminalGroup`: a group's label over its terminals, one level under it.
fn terminal_group(
    theme: &Theme,
    label: &'static str,
    terminals: Vec<(&'static str, &'static str, Option<Div>)>,
    last: bool,
) -> Div {
    let depths: Vec<usize> = std::iter::once(0)
        .chain(terminals.iter().map(|_| 1))
        .collect();
    let mut guides = tree_guides(&depths).into_iter();
    let head = group_label(label, label, None, Some(false), theme)
        .depth(0, guides.next().unwrap_or_default())
        .build(theme);
    let rows = terminals
        .into_iter()
        .zip(guides)
        .map(|((name, place, meta), guides)| {
            let row = terminal_item(name, name, Some(place.into()), theme).depth(1, guides);
            match meta {
                Some(meta) => row.meta(meta),
                None => row,
            }
            .build(theme)
        });
    v_flex()
        .gap(dp(1.))
        .p(dp(8.))
        .when(!last, |el| {
            el.border_b_1().border_color(theme.border_subtle)
        })
        .child(head)
        .children(rows)
}

/// A drawer on the stories' 334px page.
fn on_page(drawer: Drawer, theme: &Theme) -> AnyElement {
    div()
        .w(dp(334.))
        .child(drawer.build(theme))
        .into_any_element()
}

/// The stories of `DrawerItem.stories.tsx`.
pub(super) const ITEM_EXAMPLES: [Example; 2] = [
    Example {
        id: "components-draweritem--default",
        name: "Default",
        draw: |theme| {
            let mut item = drawer("drawer", "Terminals", Some(5), true);
            div().w(dp(334.)).child(item.item(theme)).into_any_element()
        },
    },
    Example {
        id: "components-draweritem--with-buttons",
        name: "With Buttons",
        draw: |theme| {
            let mut item = drawer("drawer", "Terminals", Some(5), true)
                .action(ghost("group", Icon::Mixer, "Group terminals"))
                .action(ghost("new", Icon::Plus, "New terminal…"));
            div().w(dp(334.)).child(item.item(theme)).into_any_element()
        },
    },
];

/// A drawer's card with nothing in it, saying so, and its one step: as the stories' `EmptyCard`,
/// 28 above and below and 16 at the sides.
fn empty_card(title: &'static str, detail: &'static str, step: &'static str, theme: &Theme) -> Div {
    let step = button(step, step)
        .medium()
        .style(ButtonStyle::Outline)
        .icon(Icon::Plus)
        .into_any_element();
    card(
        theme,
        [div()
            .py(dp(28.))
            .px(dp(16.))
            .child(empty_state_tidy(title, detail, [step], theme))],
    )
}

/// The stories of `Drawer.stories.tsx` this client can draw. `HiddenProjectsCard` isn't here
/// yet.
pub(super) const EXAMPLES: [Example; 3] = [
    Example {
        id: "components-drawer--default",
        name: "Default",
        draw: |theme| {
            let terminals = card(
                theme,
                [terminal_group(
                    theme,
                    "Idle",
                    vec![
                        ("review-cards", "gpui-row-buttons", None),
                        ("Terminal", "Main worktree", None),
                    ],
                    true,
                )],
            );
            on_page(
                drawer("drawer", "Terminals", Some(2), false)
                    .action(ghost("group", Icon::Mixer, "Group terminals"))
                    .action(ghost("new", Icon::Plus, "New terminal…"))
                    .content(terminals),
                theme,
            )
        },
    },
    Example {
        id: "components-drawer--grouped-terminals",
        name: "Grouped Terminals",
        draw: |theme| {
            let blocked = meta_status(MetaStatusVariant::Failed, None, theme);
            let terminals = card(
                theme,
                [
                    terminal_group(
                        theme,
                        "Idle",
                        vec![
                            ("review-cards", "gpui-row-buttons", None),
                            ("Terminal", "Main worktree", None),
                        ],
                        false,
                    ),
                    terminal_group(
                        theme,
                        "Blocked",
                        vec![("codex", "Main worktree", Some(blocked))],
                        true,
                    ),
                ],
            );
            on_page(
                drawer("drawer", "Terminals", Some(3), false)
                    .action(ghost("group", Icon::Mixer, "Group terminals"))
                    .action(ghost("new", Icon::Plus, "New terminal…"))
                    .content(terminals),
                theme,
            )
        },
    },
    Example {
        id: "components-drawer--empty",
        name: "Empty",
        draw: |theme| {
            on_page(
                drawer("terminals", "Terminals", Some(0), false)
                    .action(ghost("group", Icon::Mixer, "Group terminals"))
                    .action(ghost("new", Icon::Plus, "New terminal…"))
                    .last()
                    .content(empty_card(
                        "No terminals open",
                        "New ones open in gitbutler / Main worktree",
                        "New terminal",
                        theme,
                    )),
                theme,
            )
        },
    },
];

pub struct DrawerStory;

impl Render for DrawerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(
            (ITEM_EXAMPLES.into_iter())
                .chain(EXAMPLES)
                .map(|e| example(e, theme)),
        )
    }
}
