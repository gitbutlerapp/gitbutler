//! A project's checkout on another machine, as the mesh reports it: its uncommitted files and
//! its branches with their commits. Their diffs are on that machine, so the page links to
//! but.dev for them, and brings a branch here to work on it.

use gpui::{AnyElement, ClickEvent, Context, ElementId, SharedString, div, prelude::*, px};

use super::Detail;
use super::panes::{list_heading, meta_item, meta_row, side_header, title_text};
use crate::backend::mesh::{self, BranchStatus, Checkout};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::person::person_avatar;
use crate::ui::{
    ButtonStyle, Icon, Tooltip, age_column, button, commit_title, dot, empty_state, faint, h_flex,
    icon, just_now, list_row, plural, relative_time, small_icon, v_flex,
};

impl Detail {
    pub(super) fn render_machine(
        &self,
        project: &ProjectId,
        id: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return empty_state(
                Some(Illustration::Papers),
                "Not on the mesh anymore",
                "That machine has nothing unfinished here now",
                theme,
                cx,
            );
        };
        let server = store.mesh.connection().map(|c| c.server.clone());
        let managed = store.workspace(project).is_some_and(|ws| ws.managed);
        let header = side_header(theme)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(icon(Icon::Globe).text_color(theme.text_1))
                    .child(title_text(checkout.label()).flex_1())
                    .children(server.as_ref().map(|server| {
                        let url = mesh::link(server, &checkout.id, None);
                        button("machine-on-but-dev", "Open on but.dev")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .icon(Icon::Link)
                            .tooltip("Its changes, in the browser")
                            .on_click(move |_, _, cx| cx.open_url(&url))
                    })),
            )
            .child(
                meta_row(theme)
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .child(dot(match checkout.online {
                                true => theme.safe,
                                false => theme.text_3,
                            }))
                            .child(match checkout.online {
                                true => "Online",
                                false => "Offline",
                            }),
                    )
                    .child(
                        meta_item(Icon::Folder, theme)
                            .min_w(px(0.))
                            .child(div().truncate().child(checkout.path.clone())),
                    )
                    .children(checkout.last_activity.map(|at| {
                        meta_item(Icon::Clock, theme).child(match just_now(at) {
                            true => "Active now".to_owned(),
                            false => format!("Active {} ago", relative_time(at)),
                        })
                    })),
            )
            .children(checkout.error.as_ref().map(|err| {
                div()
                    .text_size(px(12.))
                    .text_color(theme.warn)
                    .child(format!("The mesh couldn't read all of it: {err}"))
            }))
            .when(!checkout.online, |el| {
                el.child(faint(
                    "As it was when the machine was last online. Its branches still come over \
                     from the copy the mesh keeps.",
                    theme,
                ))
            });
        v_flex()
            .size_full()
            .child(header)
            .child(
                v_flex()
                    .id("machine-page")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .pb(px(16.))
                    .when(!checkout.uncommitted.is_empty(), |el| {
                        el.child(list_heading(
                            plural(checkout.uncommitted.len(), "uncommitted file"),
                            theme,
                        ))
                        .children(
                            checkout.uncommitted.iter().enumerate().map(|(ix, file)| {
                                list_row(("machine-file", ix), false, theme)
                                    .child(small_icon(Icon::File).text_color(theme.warn))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .child(file.path.clone()),
                                    )
                                    .child(faint(
                                        format!("+{} −{}", file.additions, file.deletions),
                                        theme,
                                    ))
                            }),
                        )
                    })
                    .children(self.machine_branches(project, checkout, server, managed, theme, cx)),
            )
            .into_any_element()
    }

    /// Each branch of `checkout`, with what brings it here and opens it on but.dev, then its
    /// commits.
    fn machine_branches(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        server: Option<String>,
        managed: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if checkout.branches.is_empty() {
            return rows;
        }
        rows.push(list_heading(
            plural(checkout.branches.len(), "branch"),
            theme,
        ));
        for (bx, branch) in checkout.branches.iter().enumerate() {
            let (glyph, color) = branch_glyph(branch.status, theme);
            let unpushed = branch.commits.iter().filter(|c| !c.pushed).count();
            let apply = branch.mesh_id.clone().map(|mesh_id| {
                let (project, name) = (project.clone(), branch.name.clone());
                let why = (!managed).then_some("Only into a GitButler workspace");
                button(
                    ElementId::Name(format!("mesh-apply-{bx}").into()),
                    "Apply here",
                )
                .small()
                .style(ButtonStyle::Gray)
                .disabled_because(why)
                .tooltip("Bring its commits here and apply it to your workspace")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let (mesh_id, name) = (mesh_id.clone(), name.clone());
                    this.store.update(cx, |store, cx| {
                        store.mesh_apply(&project, mesh_id, name, cx)
                    })
                }))
            });
            let open = server.as_ref().map(|server| {
                let url = mesh::link(server, &checkout.id, Some(&branch.name));
                button(ElementId::Name(format!("mesh-open-{bx}").into()), "but.dev")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .icon(Icon::Link)
                    .tooltip("Open the branch on but.dev")
                    .on_click(move |_, _, cx| cx.open_url(&url))
            });
            let review = branch.review_url.clone().map(|url| {
                button(
                    ElementId::Name(format!("mesh-review-{bx}").into()),
                    "Pull request",
                )
                .small()
                .style(ButtonStyle::Ghost)
                .icon(Icon::Pr)
                .on_click(move |_, _, cx| cx.open_url(&url))
            });
            rows.push(
                h_flex()
                    .id(("machine-branch", bx))
                    .mx(px(6.))
                    .px(px(8.))
                    .pt(px(8.))
                    .pb(px(4.))
                    .gap(px(8.))
                    .child(icon(glyph).text_color(color))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(branch.name.clone()),
                    )
                    .when(branch.here, |el| {
                        el.child(
                            div()
                                .id(("machine-branch-here", bx))
                                .child(faint("here too", theme))
                                .tooltip(Tooltip::text("Its newest commit is in this repository")),
                        )
                    })
                    .when(unpushed > 0, |el| {
                        el.child(
                            h_flex()
                                .gap(px(3.))
                                .text_size(px(12.))
                                .text_color(theme.blue)
                                .child(small_icon(Icon::ArrowUp).text_color(theme.blue))
                                .child(unpushed.to_string()),
                        )
                    })
                    .children(review)
                    .children(open)
                    .children(apply)
                    .into_any_element(),
            );
            rows.extend(branch.commits.iter().enumerate().map(|(cx_ix, commit)| {
                let row_id = SharedString::from(format!("machine-commit-{bx}-{cx_ix}"));
                list_row(ElementId::Name(row_id), false, theme)
                    .pl(px(36.))
                    .child(small_icon(Icon::Commit).text_color(match commit.pushed {
                        true => theme.text_3,
                        false => theme.blue,
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(commit_title(&commit.title)),
                    )
                    .children(commit.author.as_ref().map(|author| {
                        div()
                            .id(ElementId::Name(
                                format!("machine-author-{bx}-{cx_ix}").into(),
                            ))
                            .flex_none()
                            .child(person_avatar(author, 14., theme))
                            .tooltip(Tooltip::text(format!("Authored by {author}")))
                    }))
                    .child(age_column(commit.created_at, theme))
                    .into_any_element()
            }));
        }
        rows
    }
}

/// What a branch on another machine shows as: merged, or still to finish.
pub(crate) fn branch_glyph(status: BranchStatus, theme: &Theme) -> (Icon, gpui::Hsla) {
    match status {
        BranchStatus::Integrated => (Icon::Tick, theme.safe),
        BranchStatus::Diverged => (Icon::Branch, theme.warn),
        BranchStatus::Pushed | BranchStatus::Unpushed | BranchStatus::Empty => {
            (Icon::Branch, theme.text_2)
        }
    }
}
