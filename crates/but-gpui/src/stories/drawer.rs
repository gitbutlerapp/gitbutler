use gpui::{AnyElement, Context, Div, Window, div, prelude::*};

use super::sidebar_rows::repository;
use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::{
    ButtonStyle, Drawer, Fold, Icon, MetaCountType, MetaStatusVariant, button, dp, drawer,
    empty_state_tidy, group_label, icon_button, meta_count, meta_status, repo_item, terminal_item,
    tree_guides, v_flex,
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

/// Puts a project the sidebar hides back among its cards.
fn show(id: &'static str) -> AnyElement {
    icon_button(id, Icon::EyeClosed)
        .small()
        .style(ButtonStyle::Outline)
        .tooltip("Show project")
        .into_any_element()
}

/// The hidden projects, each in its own part of the card with its counts and a way back into the
/// sidebar.
fn projects(theme: &Theme) -> Div {
    let count = |kind, n: &str| meta_count(kind, Some(n.to_owned().into()), theme);
    let project = |name: &'static str, counts: Vec<Div>, last: bool| {
        let row = counts.into_iter().fold(
            repo_item(name, name, theme).fold(Fold::Folded),
            |row, count| row.meta(count),
        );
        repository(theme, last, vec![(0, row.action(show(name)))])
    };
    card(
        theme,
        [
            project(
                "but-dev",
                vec![
                    count(MetaCountType::Uncommitted, "4"),
                    count(MetaCountType::Unpushed, "2"),
                    count(MetaCountType::Branch, "3"),
                    count(MetaCountType::Worktrees, "2"),
                ],
                false,
            ),
            project(
                "gitbutler-docs",
                vec![
                    count(MetaCountType::Behind, "7"),
                    count(MetaCountType::Branch, "1"),
                ],
                false,
            ),
            project(
                "homebrew-tap",
                vec![
                    count(MetaCountType::Unpushed, "1"),
                    count(MetaCountType::Branch, "2"),
                ],
                true,
            ),
        ],
    )
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
            let mut item = drawer("drawer", "Hidden projects", Some(24), true);
            div().w(dp(334.)).child(item.item(theme)).into_any_element()
        },
    },
    Example {
        id: "components-draweritem--with-buttons",
        name: "With Buttons",
        draw: |theme| {
            let mut item = drawer("drawer", "Hidden projects", Some(24), true)
                .action(ghost("add", Icon::Plus, "Add project"))
                .hover_action(ghost("sort", Icon::Mixer, "Sort and show"));
            div().w(dp(334.)).child(item.item(theme)).into_any_element()
        },
    },
];

/// A drawer's card with nothing in it, saying so, and its one step: as the stories' `EmptyCard`,
/// 28 above and below and 16 at the sides.
fn empty_card(title: &'static str, detail: &'static str, step: &'static str, theme: &Theme) -> Div {
    let step = button(step, step)
        .small()
        .style(ButtonStyle::Gray)
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

/// The stories of `Drawer.stories.tsx` this client can draw. `SidebarDrawers` isn't here yet.
pub(super) const EXAMPLES: [Example; 3] = [
    Example {
        id: "components-drawer--default",
        name: "Default",
        draw: |theme| {
            on_page(
                drawer("drawer", "Hidden projects", Some(3), false).content(projects(theme)),
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
                    .hover_action(ghost("new", Icon::Plus, "New terminal…"))
                    .content(terminals),
                theme,
            )
        },
    },
    Example {
        id: "components-drawer--empty",
        name: "Empty",
        draw: |theme| {
            let projects = drawer("projects", "Hidden projects", Some(0), false)
                .hover_action(ghost("add", Icon::Plus, "Add project"))
                .content(empty_card(
                    "Every project is showing",
                    "Hide one and it waits here, or add another",
                    "Add project",
                    theme,
                ));
            let terminals = drawer("terminals", "Terminals", Some(0), false)
                .hover_action(ghost("new", Icon::Plus, "New terminal…"))
                .last()
                .content(empty_card(
                    "No terminals open",
                    "New ones open in gitbutler / Main worktree",
                    "New terminal",
                    theme,
                ));
            v_flex()
                .w(dp(334.))
                .child(projects.build(theme))
                .child(terminals.build(theme))
                .into_any_element()
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
