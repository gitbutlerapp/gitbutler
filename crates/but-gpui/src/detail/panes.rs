//! The column beside the diff and the header above both: what's selected, its files,
//! and the actions that fit it.

use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, SharedString, UniformListScrollHandle,
    Window, div, prelude::*, px, uniform_list,
};

use super::Detail;
use super::pull_request::render_review;
use crate::app::StartCommit;
use crate::backend::{self, CommitState, FileChange, PushState, short_ref};
use crate::state::{ProjectId, Review, Selection};
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, commit_title, empty_state,
    faint, h_flex, icon, icon_button, local_time, muted, plural, section_label, shortcut,
    small_icon, v_flex,
};

impl Detail {
    pub(super) fn render_header(
        &self,
        selection: Option<&Selection>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let store = self.store.read(cx);
        // Each crumb but the last links to what it names.
        let mut crumbs: Vec<Crumb> = Vec::new();
        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(selection) = selection
            && let Some(entry) = store.project(selection.project())
        {
            crumbs.push((
                entry.project.name.clone().into(),
                Some(Selection::Project(entry.id.clone())),
            ));
            let workspace = entry.workspace.as_ref();
            match selection {
                Selection::Project(_) => {
                    let needs_update = workspace
                        .and_then(|ws| ws.target.as_ref())
                        .filter(|target| target.needs_update)
                        .map(|target| target.name.clone());
                    actions.push(
                        button("fetch", "Fetch")
                            .icon(Icon::Refresh)
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key(
                                "Fetch from the project's remotes",
                                shortcut(&crate::app::Fetch, cx),
                            )
                            .dispatch(&crate::app::Fetch)
                            .into_any_element(),
                    );
                    if let Some(target) = needs_update {
                        actions.push(
                            button("update", format!("Update from {target}"))
                                .icon(Icon::ArrowDown)
                                .style(ButtonStyle::Pop)
                                .tooltip_with_key(
                                    "Rebase every stack onto the target's new commits",
                                    shortcut(&crate::app::UpdateFromTarget, cx),
                                )
                                .dispatch(&crate::app::UpdateFromTarget)
                                .into_any_element(),
                        );
                    }
                }
                Selection::Uncommitted(_) => {
                    crumbs.push(("Uncommitted changes".into(), None));
                    actions.extend(self.mark_all_reviewed(cx));
                }
                Selection::Branch(_, ref_name) => {
                    crumbs.push((short_ref(ref_name).to_owned().into(), None));
                    // An unnamed branch has no ref to open a pull request from.
                    let can_create = !ref_name.is_empty()
                        && workspace
                            .and_then(|ws| ws.find_branch(ref_name))
                            .is_some_and(|b| b.review.is_none() && !b.commits.is_empty())
                        && crate::pr::on_forge(store, selection);
                    let pushable = workspace
                        .filter(|ws| ws.find_branch(ref_name).is_some_and(|b| b.push.can_push()));
                    let pushing = store.pushing(selection.project(), ref_name);
                    let busy = pushing.then_some("Wait for the push to finish");
                    // Landing directly takes the place of a pull request, forge or not.
                    let land = (store.lands_directly(selection.project()))
                        .then_some(workspace)
                        .flatten()
                        .and_then(|ws| {
                            let whole = crate::edits::land_scope(ws, ref_name)?;
                            Some((whole, &ws.target.as_ref()?.name))
                        });
                    let next = match land {
                        Some((whole, target)) => Some(
                            button(
                                "land",
                                match whole {
                                    true => format!("Land the stack on {target}"),
                                    false => format!("Land on {target}"),
                                },
                            )
                            .icon(Icon::BranchMerge)
                            .dispatch(&crate::edits::LandBranch),
                        ),
                        None if can_create => Some(
                            button(
                                "create-pr",
                                match store.publishing(selection.project(), ref_name) {
                                    true => "Creating…",
                                    false => "Create pull request",
                                },
                            )
                            .icon(Icon::Pr)
                            .dispatch(&crate::pr::CreatePullRequest),
                        ),
                        None => None,
                    };
                    if let Some(next) = next {
                        // The next step once nothing's left to push.
                        let style = match pushable {
                            Some(_) => ButtonStyle::Ghost,
                            None => ButtonStyle::Pop,
                        };
                        actions.push(next.style(style).disabled_because(busy).into_any_element());
                    }
                    if let Some(ws) = pushable {
                        // Pushing takes the branches below along, so they count too.
                        let label = if pushing {
                            "Pushing…"
                        } else if ws.needs_force(ref_name) {
                            "Force push"
                        } else {
                            "Push"
                        };
                        actions.push(
                            button("push", label)
                                .icon(Icon::ArrowUp)
                                .style(ButtonStyle::Pop)
                                .disabled_because(busy)
                                .tooltip_with_key(
                                    "Push this branch and the ones below it",
                                    shortcut(&crate::app::Push, cx),
                                )
                                .dispatch(&crate::app::Push)
                                .into_any_element(),
                        );
                    }
                }
                Selection::Branches(project) => {
                    crumbs.push(("Branches".into(), Some(selection.clone())));
                    self.branches_header(project, &mut crumbs, &mut actions, theme, cx);
                }
                Selection::Worktree(_, name) => {
                    crumbs.push((format!("Worktree {name}").into(), None));
                    actions.extend(self.mark_all_reviewed(cx));
                }
                Selection::History(project) => {
                    crumbs.push(("History".into(), None));
                    self.history_header(project, &mut actions, cx);
                }
                Selection::Commit(project, id) => {
                    if let Some((branch, commit)) = workspace.and_then(|ws| ws.find_commit(*id)) {
                        let branch_selection =
                            Selection::Branch(project.clone(), branch.ref_name.clone());
                        crumbs.push((branch.name.clone().into(), Some(branch_selection)));
                        crumbs.push((commit.id.short().into(), None));
                    }
                }
            }
        }
        let last = crumbs.len().saturating_sub(1);
        h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px(px(16.))
            .when(self.beside_traffic_lights, |el| el.pl(px(84.)))
            .gap(px(6.))
            .border_b_1()
            .border_color(theme.border_3)
            .children(
                crumbs
                    .into_iter()
                    .enumerate()
                    .flat_map(|(ix, (crumb, target))| {
                        let sep = (ix > 0)
                            .then(|| div().text_color(theme.text_3).child("/").into_any_element());
                        let el = match target.filter(|_| ix != last) {
                            Some(target) => {
                                let app = self.app.clone();
                                div()
                                    .id(("crumb", ix))
                                    .cursor_pointer()
                                    .hover(|el| el.underline().text_color(theme.text_1))
                                    .text_color(theme.text_2)
                                    .truncate()
                                    .child(crumb)
                                    .on_click(move |_, window, cx| {
                                        (app.update(cx, |app, cx| {
                                            app.select(target.clone(), window, cx)
                                        }))
                                        .ok();
                                    })
                                    .into_any_element()
                            }
                            None => div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_1)
                                .truncate()
                                .child(crumb)
                                .into_any_element(),
                        };
                        sep.into_iter().chain([el])
                    }),
            )
            .child(div().flex_1())
            .children(actions)
    }

    /// "Mark all reviewed" for the selected checkout's uncommitted files, while any still
    /// want a look. It marks them as they are when clicked.
    fn mark_all_reviewed(&self, cx: &App) -> Option<AnyElement> {
        let key = self.key_for_selection(cx)?;
        let scope = key.review_scope()?;
        let store = self.store.read(cx);
        let files = key.uncommitted(store)?;
        if store.unreviewed(&scope, files) == 0 {
            return None;
        }
        let store = self.store.clone();
        Some(
            button("review-all", "Mark all reviewed")
                .icon(Icon::Tick)
                .style(ButtonStyle::Ghost)
                .on_click(move |_, _, cx| {
                    store.update(cx, |store, cx| {
                        let files: Vec<FileChange> = (key.uncommitted(store).unwrap_or_default())
                            .iter()
                            .filter(|f| store.review(&scope, f) != Review::Reviewed)
                            .cloned()
                            .collect();
                        store.set_reviewed(&scope, &files, true, cx)
                    })
                })
                .into_any_element(),
        )
    }

    /// What an agent left uncommitted, laid out for reviewing first. The commit box
    /// stays closed until asked for, and only then do checkboxes appear.
    pub(super) fn render_uncommitted_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return empty_state("Loading…", "", theme);
        };
        let footer = if !self.composing {
            let target = self.effective_target(project, cx);
            let target = target.as_deref().map_or("a new branch", short_ref);
            h_flex()
                .flex_none()
                .h(px(44.))
                .px(px(12.))
                .gap(px(8.))
                .border_t_1()
                .border_color(theme.border_3)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(12.))
                        .text_color(theme.text_3)
                        .child(format!("Commit to {target}")),
                )
                .child(
                    button("open-composer", "Commit…")
                        .small()
                        .tooltip_with_key("Write a commit message", shortcut(&StartCommit, cx))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.focus_message(window, cx)),
                        ),
                )
                .into_any_element()
        } else {
            self.render_composer(project, theme, window, cx)
        };

        v_flex()
            .size_full()
            .child(self.render_uncommitted_files(project, &workspace.uncommitted, theme, cx))
            .when(!workspace.uncommitted.is_empty(), |el| el.child(footer))
            .into_any_element()
    }

    pub(super) fn render_commit_side(
        &self,
        project: &ProjectId,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((_, commit)) = store.find_commit(project, id) else {
            return empty_state("Commit not found", "", theme);
        };
        let files = self.render_diff_file_list(theme, cx);
        let banner = commit
            .conflicted
            .then(|| self.conflict_banner(cx))
            .flatten();
        v_flex()
            .size_full()
            .children(banner.map(|banner| side_header(theme).child(banner)))
            .child(files)
            .into_any_element()
    }

    /// The commit's header across the page, above its files and diff, as GitButler's web
    /// app draws it: its title, its body when opened, then who made it and when, its id and
    /// whether it's pushed. Editing the message happens here too.
    pub(super) fn render_commit_header(
        &self,
        project: &ProjectId,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((_, commit)) = store.find_commit(project, id) else {
            return div().into_any_element();
        };
        let body = (commit.message.split_once('\n'))
            .map(|(_, body)| body.trim().to_owned())
            .unwrap_or_default();
        let (state, color) = match commit.state {
            CommitState::Local => ("not pushed", theme.blue),
            CommitState::Pushed => ("on the remote", theme.safe),
            CommitState::Integrated => ("integrated", theme.text_2),
        };
        let sha = commit.id.0.to_string();
        let title = h_flex()
            .id("commit-title")
            .h(px(22.))
            .gap(px(8.))
            .child(icon(Icon::Commit).text_color(theme.text_1))
            .child(
                h_flex()
                    .min_w(px(0.))
                    .gap(px(6.))
                    .child(
                        div()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(16.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(commit_title(&commit.title)),
                    )
                    .when(!body.is_empty(), |el| {
                        let open = self.message_open;
                        el.child(
                            icon_button("full-message", Icon::TextBlock)
                                .small()
                                .selected(open)
                                .tooltip(if open {
                                    "Hide full message"
                                } else {
                                    "Full message"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message_open = !this.message_open;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    this.start_reword(window, cx)
                }
            }));
        let meta = h_flex()
            .h(px(20.))
            .gap(px(12.))
            .text_size(px(13.))
            .text_color(theme.text_2)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(person_avatar(&commit.author, 16., theme))
                    .child(format!("{} at {}", commit.author, local_time(commit.time))),
            )
            .child(
                h_flex()
                    .id("commit-sha")
                    .gap(px(4.))
                    .cursor_pointer()
                    .group("commit-sha")
                    .text_color(theme.text_3)
                    .hover(|el| el.text_color(theme.text_2))
                    .child(
                        small_icon(Icon::Hash)
                            .text_color(theme.text_3)
                            .group_hover("commit-sha", |el| el.text_color(theme.text_2)),
                    )
                    .child(commit.id.short())
                    .tooltip(Tooltip::text("Copy commit ID"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let sha = sha.clone();
                        this.app.update(cx, |app, cx| app.copy_text(sha, cx)).ok();
                    })),
            )
            .child(div().text_color(color).child(state))
            .when(commit.conflicted, |el| {
                el.child(div().text_color(theme.danger).child("conflicted"))
            });
        let main = v_flex()
            .gap(px(10.))
            .map(|el| match &self.rewording {
                Some((input, _)) => el.child(self.reword_editor(input, theme, cx)),
                None => el
                    .child(title)
                    // Further off the lines around it than they stand from each other.
                    .when(self.message_open && !body.is_empty(), |el| {
                        el.child(
                            div()
                                .mt(px(2.))
                                .mb(px(6.))
                                .max_w(px(580.))
                                .font_family(theme.mono_font.clone())
                                .text_size(px(12.))
                                .line_height(px(19.))
                                .child(body),
                        )
                    }),
            })
            .child(meta);
        // What right-clicking the commit's row offers, for when the navigator is out of reach.
        let target = Selection::Commit(project.clone(), id);
        let menu = h_flex()
            .h(px(22.))
            .gap(px(8.))
            .children(super::resolve::apply_button(
                self,
                cx.entity().downgrade(),
                cx,
            ))
            .child(
                icon_button("commit-menu", Icon::Kebab).on_click(cx.listener(
                    move |this, event: &ClickEvent, _, cx| {
                        let position = event.position();
                        (this.app)
                            .update(cx, |app, cx| app.open_row_menu(&target, position, cx))
                            .ok();
                    },
                )),
            );
        // The diff beside the files starts below this, however tall it's drawn.
        let (height, app) = (self.header_height.clone(), self.app.clone());
        let measure = gpui::canvas(
            move |bounds, _, cx| {
                // And the rule along its bottom, which this, inside the header, doesn't cover.
                let drawn = f32::from(bounds.size.height) + 1.;
                // After this frame: a view told to draw while one is drawing isn't drawn again.
                if height.replace(drawn) != drawn {
                    cx.defer(move |cx| {
                        app.update(cx, |_, cx| cx.notify()).ok();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        h_flex()
            .relative()
            .flex_none()
            .items_start()
            .gap(px(16.))
            .p(px(16.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(main.flex_1().min_w(px(0.)))
            .child(menu)
            .child(measure)
            .into_any_element()
    }

    /// The box editing a commit's message, with how to save or leave it.
    fn reword_editor(
        &self,
        input: &Entity<TextInput>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .gap(px(8.))
            .child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(theme.pop)
                    .line_height(px(19.))
                    .child(input.clone()),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .justify_end()
                    .child(faint("⌘↩ to save, esc to cancel", theme).flex_1())
                    .child(
                        button("cancel-reword", "Cancel")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.rewording = None;
                                cx.notify();
                            })),
                    )
                    .child({
                        let input = input.clone();
                        button("save-reword", "Save")
                            .small()
                            .style(ButtonStyle::Pop)
                            .on_click(move |_, _, cx| {
                                input.update(cx, |_, cx| cx.emit(InputEvent::Submit))
                            })
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_branch_side(
        &self,
        project: &ProjectId,
        ref_name: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((ws, branch)) =
            (store.workspace(project)).and_then(|ws| Some((ws, ws.find_branch(ref_name)?)))
        else {
            return empty_state("Branch not found", "", theme);
        };
        let push = match branch.push {
            PushState::NothingToPush => "Up to date with its remote",
            PushState::Unpushed => "Has unpushed commits",
            PushState::NeedsForce => "Rewritten, needs a force push",
            PushState::NeverPushed => "Not pushed yet",
            PushState::Integrated => "Integrated",
        };
        let files = self.render_diff_file_list(theme, cx);
        // Resolving the oldest conflict first, even in a branch below, rebases the rest onto
        // its result. Commits list newest first, and a stack's branches top first.
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        let oldest = (ws.stack_of(ref_name))
            .filter(|_| conflicted > 0)
            .and_then(|(stack, at)| {
                stack.branches[at..].iter().rev().find_map(|holder| {
                    let commit = holder.commits.iter().rev().find(|c| c.conflicted)?;
                    Some((holder, commit))
                })
            });
        let resolve_oldest = oldest.map(|(holder, commit)| {
            let app = self.app.clone();
            let (project, id) = (project.clone(), commit.id);
            super::resolve::warn_banner(
                match conflicted {
                    1 => "A commit has conflicts".to_owned(),
                    n => format!("{n} commits have conflicts"),
                },
                match holder.ref_name == ref_name {
                    true => {
                        "Start with the oldest: the ones above it are rebuilt on it.".to_owned()
                    }
                    false => format!("The oldest is in {}, below this branch.", holder.name),
                },
                Some(
                    super::resolve::banner_action("resolve", "Resolve", theme)
                        .on_click(move |_, window, cx| {
                            app.update(cx, |app, cx| {
                                app.resolve_commit(project.clone(), id, window, cx)
                            })
                            .ok();
                        })
                        .into_any_element(),
                ),
                theme,
            )
        });
        v_flex()
            .size_full()
            .child(
                side_header(theme)
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(icon(Icon::Branch).text_color(theme.text_2))
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_size(px(14.))
                                    .text_color(theme.blue)
                                    .truncate()
                                    .child(branch.name.clone()),
                            ),
                    )
                    .child(muted(
                        format!("{} · {push}", plural(branch.commits.len(), "commit")),
                        theme,
                    ))
                    .children(resolve_oldest),
            )
            .when_some(branch.review.as_ref(), |el, review| {
                el.child(render_review(review, theme, cx))
            })
            .when(!branch.incoming.is_empty(), |el| {
                let pull = ws
                    .can_pull(ref_name)
                    .then(|| (project.clone(), ref_name.to_owned()));
                el.child(render_incoming(&branch.incoming, pull, theme, cx))
            })
            .child(files)
            .into_any_element()
    }
}

/// The commits only the branch's remote has, as an agent or a teammate pushed them, and
/// a way to bring them in: `pull`, the branch's project and full ref, when it can.
fn render_incoming(
    titles: &[String],
    pull: Option<(ProjectId, String)>,
    theme: &Theme,
    cx: &Context<Detail>,
) -> AnyElement {
    let can_pull = pull.is_some();
    v_flex()
        .flex_none()
        .px(px(16.))
        .py(px(10.))
        .gap(px(4.))
        .border_b_1()
        .border_color(theme.border_3)
        .text_size(px(12.))
        .child(
            h_flex()
                .gap(px(8.))
                .child(div().flex_1().text_color(theme.warn).child(format!(
                    "{} on the remote, not here",
                    plural(titles.len(), "commit")
                )))
                .when_some(pull, |el, (project, ref_name)| {
                    el.child(
                        button("pull-remote", "Pull")
                            .small()
                            .tooltip("Bring them in, with this branch's own commits on top")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.store.update(cx, |store, cx| {
                                    store.pull_remote(&project, ref_name.clone(), false, cx)
                                })
                            })),
                    )
                }),
        )
        .when(!can_pull, |el| {
            el.child(
                div()
                    .text_color(theme.text_3)
                    .child("Pulling into a branch with others stacked on it isn't possible yet."),
            )
        })
        .children(titles.iter().map(|title| {
            h_flex()
                .gap(px(6.))
                .text_color(theme.text_2)
                .child(small_icon(Icon::ArrowDown).text_color(theme.text_3))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .child(commit_title(title)),
                )
        }))
        .into_any_element()
}

/// A crumb of the header: what it names, and what clicking it selects.
pub(super) type Crumb = (SharedString, Option<Selection>);

/// How tall a commit's header across the page is with its message closed, until it's drawn.
pub(super) const COMMIT_HEADER_HEIGHT: f32 = 85.;

/// The block at the top of a side panel, saying what's selected.
pub(super) fn side_header(theme: &Theme) -> Div {
    v_flex()
        .flex_none()
        .p(px(16.))
        .gap(px(6.))
        .border_b_1()
        .border_color(theme.border_3)
}

/// One of the lists beside the diff, drawn only as far as it's on screen, with a
/// scrollbar whose state goes by `scrollbar`. `row` draws the item at an index.
pub(super) fn side_list(
    id: &'static str,
    count: usize,
    scroll: &UniformListScrollHandle,
    scrollbar: &'static str,
    cx: &Context<Detail>,
    row: impl Fn(&Detail, usize, &Theme, &Context<Detail>) -> Option<AnyElement> + 'static,
) -> Div {
    let list = uniform_list(
        id,
        count,
        cx.processor(move |this, range: Range<usize>, _, cx| {
            let theme = Theme::get(cx);
            range
                .filter_map(|ix| row(this, ix, theme, cx))
                // A list item is as wide as its content unless something stretches it.
                .map(|row| div().w_full().child(row))
                .collect::<Vec<_>>()
        }),
    )
    .track_scroll(scroll)
    .size_full();
    let theme = Theme::get(cx);
    let scroll = Scroll::Div(scroll.0.borrow().base_handle.clone());
    scrolled(list, scrollbar, scroll, theme)
}

/// A group's heading in a side list, sitting on the rows under it.
pub(super) fn list_heading(text: impl IntoElement, theme: &Theme) -> AnyElement {
    section_label(text, theme)
        .w_full()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_end()
        .px(px(14.))
        .pb(px(4.))
        .into_any_element()
}
