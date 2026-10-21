//! The column beside the diff and the header above both: what's selected, its files,
//! and the actions that fit it.

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, Focusable as _, Pixels, Point,
    SharedString, Stateful, UniformListScrollHandle, Window, div, prelude::*, px, uniform_list,
};

use super::Detail;
use crate::app::StartCommit;
use crate::backend::{self, CommitState, PushState, short_ref};
use crate::overlays::MenuItem;
use crate::state::{Cut, MeshPick, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::person::{byline_avatar, byline_text};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    Button, ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, commit_title,
    empty_state, faint, field, h_flex, icon, icon_button, list_row, local_time, plural,
    section_label, shortcut, small_icon, v_flex,
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
        if let Some(selection) = selection
            && let Some(entry) = store.project(selection.project())
        {
            crumbs.push((
                entry.project.name.clone().into(),
                Some(Selection::Project(entry.id.clone())),
            ));
            let workspace = entry.workspace.as_ref();
            // The worktree a branch, stack or commit is in, named as its navigator heading is.
            let heading = |item: &Selection| -> Crumb {
                let project = item.project().clone();
                match store.worktree_of(item) {
                    Some(worktree) => (
                        worktree.name.to_string().into(),
                        Some(Selection::Worktree(project, worktree.name.clone())),
                    ),
                    None => (
                        "Main worktree".into(),
                        Some(Selection::Uncommitted(project)),
                    ),
                }
            };
            match selection {
                Selection::Project(project) => self.upstream_crumbs(project, &mut crumbs, cx),
                Selection::Uncommitted(_) => match workspace.and_then(|ws| ws.editing.as_ref()) {
                    Some(editing) => {
                        let title = commit_title(&editing.title);
                        crumbs.push((format!("Editing {title}").into(), None));
                    }
                    None => crumbs.push(("Main worktree".into(), None)),
                },
                Selection::Branch(project, ref_name) => {
                    let branch = workspace.and_then(|ws| ws.find_branch(ref_name));
                    let name = match branch {
                        Some(branch) => store.name_of(project, branch),
                        None if ref_name.is_empty() => "Unnamed branch",
                        None => short_ref(ref_name),
                    };
                    crumbs.push(heading(selection));
                    crumbs.push((name.to_owned().into(), None));
                }
                Selection::Branches(project) => {
                    crumbs.push(("Branches".into(), Some(selection.clone())));
                    self.branches_crumbs(project, &mut crumbs);
                }
                Selection::Worktree(_, name) => {
                    crumbs.push((name.to_string().into(), None));
                }
                Selection::Mesh(project, id, pick) => {
                    if let Some(checkout) = store.mesh_checkout(project, id) {
                        // Each but the last goes to what it names.
                        let at = |pick| Some(Selection::Mesh(project.clone(), id.clone(), pick));
                        let up = |on: bool, pick| if on { at(pick) } else { None };
                        let below = pick.branch().is_some();
                        crumbs.push((checkout.label().into(), up(below, MeshPick::Checkout)));
                        if let Some(name) = pick.branch() {
                            let on = matches!(pick, MeshPick::Commit(..));
                            crumbs.push((
                                name.to_owned().into(),
                                up(on, MeshPick::Branch(name.into())),
                            ));
                        }
                        if let MeshPick::Commit(_, commit) = pick {
                            crumbs.push((commit.short().into(), None));
                        }
                    }
                }
                Selection::Stack(_, bottom) => {
                    let n = workspace
                        .and_then(|ws| ws.lane(bottom))
                        .map_or(0, <[_]>::len);
                    crumbs.push(heading(selection));
                    crumbs.push((format!("Stack of {n}").into(), None));
                }
                Selection::History(_) => crumbs.push(("Operations history".into(), None)),
                // After what has it, the branch or the target, which go to their pages.
                Selection::Upstream(project, ref_name, id) => {
                    match ref_name
                        .as_ref()
                        .and_then(|r| Some((r, workspace?.find_branch(r)?)))
                    {
                        Some((ref_name, branch)) => {
                            let branch_selection =
                                Selection::Branch(project.clone(), ref_name.clone());
                            crumbs.push(heading(&branch_selection));
                            let name = store.name_of(project, branch).to_owned();
                            crumbs.push((name.into(), Some(branch_selection)));
                        }
                        None => {
                            let target = workspace.and_then(|ws| ws.target.as_ref());
                            let name = target.map_or("Target", |t| t.name.as_str());
                            let page = Selection::Project(project.clone());
                            crumbs.push((name.to_owned().into(), Some(page)));
                        }
                    }
                    crumbs.push((id.short().into(), None));
                }
                Selection::Commit(project, id) => {
                    if let Some((branch, commit)) = workspace.and_then(|ws| ws.find_commit(*id)) {
                        let branch_selection =
                            Selection::Branch(project.clone(), branch.ref_name.clone());
                        crumbs.push(heading(selection));
                        let name = store.name_of(project, branch).to_owned();
                        crumbs.push((name.into(), Some(branch_selection)));
                        crumbs.push((commit.id.short().into(), None));
                    }
                }
            }
        }
        let last = crumbs.len().saturating_sub(1);
        // As ui-react's LocationBar: on the window's second ground, the navigator's toggle, a
        // hairline, then the trail, every crumb quiet and joined by chevrons.
        let toggle = icon_button("toggle-navigator", Icon::Sidebar)
            .tooltip_with_key(
                "Toggle navigator",
                shortcut(&crate::app::ToggleNavigator, cx),
            )
            .dispatch(&crate::app::ToggleNavigator);
        h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px(px(12.))
            .when(self.beside_traffic_lights, |el| el.pl(px(84.)))
            .gap(px(12.))
            .bg(theme.bg_2)
            .border_b_1()
            .border_color(theme.border_3)
            .text_color(theme.text_2)
            .child(toggle)
            .child(div().flex_none().w(px(1.)).h(px(16.)).bg(theme.border_3))
            .child(
                h_flex().flex_1().min_w(px(0.)).gap(px(6.)).children(
                    crumbs
                        .into_iter()
                        .enumerate()
                        .flat_map(|(ix, (crumb, target))| {
                            let sep = (ix > 0).then(|| {
                                small_icon(Icon::ChevronRight)
                                    .size(px(10.))
                                    .text_color(theme.text_3)
                                    .into_any_element()
                            });
                            // Shrinking by length as well as width, long crumbs give way first and
                            // short ones, like the project's name or a commit's ID, stay whole.
                            let shrink = crumb.chars().count() as f32;
                            let el = match target.filter(|_| ix != last) {
                                Some(target) => {
                                    let app = self.app.clone();
                                    // A step darker under the pointer: it's still a quiet trail.
                                    let hover = theme.text_2.blend(theme.text_1.opacity(0.5));
                                    div()
                                        .id(("crumb", ix))
                                        .hover(move |el| el.text_color(hover))
                                        .truncate()
                                        .map(|mut el| {
                                            el.style().flex_shrink = Some(shrink);
                                            el
                                        })
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
                                    .truncate()
                                    .map(|mut el| {
                                        el.style().flex_shrink = Some(shrink);
                                        el
                                    })
                                    .child(crumb)
                                    .into_any_element(),
                            };
                            sep.into_iter().chain([el])
                        }),
                ),
            )
    }

    /// The menu of a commit that isn't in the workspace, on the Branches page or only on a
    /// branch's remote: cherry-pick it, and those picked with it, or copy its ID.
    pub(super) fn commit_menu(
        &self,
        project: &ProjectId,
        ids: Arc<[backend::CommitId]>,
        id: backend::CommitId,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let label = match ids.len() {
            1 => "Cherry-pick".to_owned(),
            n => format!("Cherry-pick {n} commits"),
        };
        let project = project.clone();
        let items = vec![
            MenuItem::new(label, move |app, _, cx| {
                app.cut(Cut::CherryPick(project.clone(), ids.clone()), cx)
            }),
            MenuItem::new("Copy commit ID", move |app, _, cx| {
                app.copy_text(id.0.to_string(), cx)
            }),
        ];
        (self.app)
            .update(cx, |app, cx| app.open_menu(at, items, cx))
            .ok();
    }

    /// A side list's filter field, then the button whose `menu` picks what the list shows,
    /// lit while that leaves some out.
    pub(super) fn filter_bar(
        &self,
        input: &Entity<TextInput>,
        tooltip: &'static str,
        menu: fn(&Self, Point<Pixels>, &mut Context<Self>),
        narrowed: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let focused = input.focus_handle(cx).is_focused(window);
        h_flex()
            .flex_none()
            .m(px(10.))
            .gap(px(4.))
            .child(field(input, focused, Theme::get(cx)).flex_1())
            .child(
                icon_button("list-filters", Icon::Filter)
                    .small()
                    .selected(narrowed)
                    .tooltip(tooltip)
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        menu(this, event.position(), cx)
                    })),
            )
    }

    /// Fetch, as ⌥⇧F does, to go beside a list's filter: nothing for a project whose folder is
    /// gone. Without a remote, a fetch says there's none.
    pub(super) fn fetch_button(&self, project: &ProjectId, cx: &Context<Self>) -> Option<Button> {
        let store = self.store.read(cx);
        if !store.project(project)?.project.path.exists() {
            return None;
        }
        let busy = store.fetching(project);
        Some(
            icon_button("fetch", Icon::Refresh)
                .small()
                .tooltip_with_key(
                    "Fetch, and check pull requests and CI again",
                    shortcut(&crate::app::Fetch, cx),
                )
                .disabled_because(busy.then_some("Fetching already"))
                .dispatch(&crate::app::Fetch),
        )
    }

    /// Cancel and Finish for the commit `project` has checked out for editing. Finishing
    /// applies what's picked first, and with files still conflicted asks, as their markers
    /// would go into the commit.
    fn editing_actions(&self, project: &ProjectId, cx: &Context<Self>) -> [AnyElement; 2] {
        let cancel = project.clone();
        let finish = project.clone();
        [
            button("cancel-editing", "Cancel")
                .small()
                .style(ButtonStyle::Ghost)
                .tooltip("Put the commit back as it was")
                .on_click(cx.listener(move |this, _, _, cx| {
                    (this.store).update(cx, |store, cx| store.cancel_editing(&cancel, false, cx))
                }))
                .into_any_element(),
            button("finish-editing", "Finish")
                .small()
                .icon(Icon::Tick)
                .style(ButtonStyle::Pop)
                .tooltip("Commit the files as they are in its place, and rebase what's above it")
                .on_click(cx.listener(move |this, _, window, cx| {
                    let project = finish.clone();
                    let left = (this.store.read(cx).workspace(&project)).map_or(0, |ws| {
                        (ws.uncommitted.iter())
                            .filter(|f| f.conflicted && !this.picks_settle(&f.path))
                            .count()
                    });
                    // What's picked goes in first, as finishing means the picking is done;
                    // finishing waits for it, so a resolve that fails finishes nothing.
                    let finish = move |this: &mut Self, cx: &mut Context<Self>| {
                        if !this.apply_resolutions(true, cx) {
                            (this.store).update(cx, |store, cx| store.finish_editing(&project, cx))
                        }
                    };
                    if left == 0 {
                        return finish(this, cx);
                    }
                    let detail = cx.entity();
                    (this.app.update(cx, |app, cx| {
                        app.confirm_then(
                            format!("Finish with {} still conflicted?", plural(left, "file")),
                            "Their conflict markers go into the commit as they are.",
                            "Finish",
                            window,
                            cx,
                            move |_, cx| detail.update(cx, finish),
                        )
                    }))
                    .ok();
                }))
                .into_any_element(),
        ]
    }

    /// What an agent left uncommitted in the project's own checkout or the linked
    /// `worktree`, laid out to read first. The commit box stays closed until asked for, and
    /// only then do checkboxes appear.
    pub(super) fn render_uncommitted_side(
        &self,
        project: &ProjectId,
        worktree: Option<&Arc<str>>,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return empty_state(None, "Loading…", "", theme, cx);
        };
        let (files, git_operation) = match worktree.map(|name| workspace.find_worktree(name)) {
            None => (&workspace.uncommitted, workspace.git_operation),
            Some(Some(worktree)) => (&worktree.uncommitted, worktree.git_operation),
            Some(None) => {
                return empty_state(
                    Some(Illustration::Papers),
                    "Worktree not found",
                    "",
                    theme,
                    cx,
                );
            }
        };
        // A commit being edited, in the main checkout, is finished or cancelled where a commit
        // would be made.
        let editing = worktree.is_none() && workspace.editing.is_some();
        let footer = if editing {
            h_flex()
                .flex_none()
                .h(px(super::FOOTER_HEIGHT))
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
                        .child("Editing the commit"),
                )
                .children(self.editing_actions(project, cx))
                .into_any_element()
        } else if !self.composing {
            let target = self.effective_target(project, cx);
            let target = match (target.as_deref(), worktree) {
                (Some(target), _) => format!("Commit to {}", short_ref(target)),
                (None, None) => "Commit to a new branch".to_owned(),
                (None, Some(_)) => "No branch checked out".to_owned(),
            };
            h_flex()
                .flex_none()
                .h(px(super::FOOTER_HEIGHT))
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
                        .child(target),
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
            self.render_composer(project, files, theme, window, cx)
        };

        let banner = self.conflict_banner(cx.entity().downgrade(), cx);
        // Committing here while Git is partway through something leaves it there.
        let unfinished = git_operation.map(|op| unfinished_banner(op, theme));
        v_flex()
            .size_full()
            .children(unfinished)
            .children(banner.map(|banner| side_header(theme).child(banner)))
            .child(self.render_uncommitted_files(project, worktree, files, theme, cx))
            .child(footer)
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
            return empty_state(
                Some(Illustration::Papers),
                "Commit not found",
                "",
                theme,
                cx,
            );
        };
        let files = self.render_diff_file_list(theme, cx);
        let banner = commit
            .conflicted
            .then(|| self.conflict_banner(cx.entity().downgrade(), cx))
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
        let (title, message) = store.message_of(project, commit);
        let body = (message.split_once('\n'))
            .map(|(_, body)| body.trim().to_owned())
            .unwrap_or_default();
        let target = (store.workspace(project).and_then(|ws| ws.target.as_ref()))
            .map_or("the target", |t| t.name.as_str());
        let (state, color) = match commit.state {
            CommitState::Local => ("not pushed".to_owned(), theme.blue),
            CommitState::Pushed => ("on the remote".to_owned(), theme.safe),
            CommitState::Integrated => (format!("in {target}"), theme.text_2),
        };
        // A long title wraps rather than hiding its end: it's shown nowhere else.
        let title = page_title(Icon::Commit, theme)
            .id("commit-title")
            .child(
                h_flex()
                    .items_start()
                    .min_w(px(0.))
                    .gap(px(6.))
                    .child(title_text(commit_title(title)))
                    .when(!body.is_empty(), |el| {
                        let open = self.message_open;
                        el.child(
                            icon_button("full-message", Icon::TextBlock)
                                .small()
                                .selected(open)
                                .tooltip(if open {
                                    "Hide full message"
                                } else {
                                    "Show full message"
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
        let meta = meta_row(theme)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(byline_avatar(
                        &commit.author,
                        &commit.email,
                        commit.agent.as_ref(),
                        16.,
                        theme,
                    ))
                    .child(format!(
                        "{} at {}",
                        byline_text(&commit.author, commit.agent.as_ref()),
                        local_time(commit.time)
                    )),
            )
            .child(self.sha_chip(id, theme, cx))
            .when_some(self.signers.get(&id).copied().flatten(), |el, signer| {
                el.child(
                    meta_item(Icon::Lock, theme)
                        .id("commit-signed")
                        .text_color(theme.text_3)
                        .child("Signed")
                        .tooltip(Tooltip::text(format!("Signed with {signer}, not verified"))),
                )
            })
            .child(div().text_color(color).child(state))
            .when(commit.conflicted, |el| {
                el.child(div().text_color(theme.danger).child("conflicted"))
            });
        let rewording = (self.rewording.as_ref()).filter(|(key, ..)| *key == commit.key);
        let main = v_flex()
            .gap(px(10.))
            .map(|el| match rewording {
                Some((_, input, _)) => el.child(self.reword_editor(input, theme)),
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
        let target = Selection::Commit(project.clone(), id);
        let apply = super::resolve::apply_button(self, cx.entity().downgrade(), cx);
        self.page_header(main, target, apply, None, theme, cx)
    }

    /// A commit's short ID in its line's ink, as ui-react's `CopyableId`, which copies the
    /// whole one on a click.
    pub(super) fn sha_chip(
        &self,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let sha = id.0.to_string();
        let hover = theme.text_2.blend(theme.text_1.opacity(0.5));
        h_flex()
            .id("commit-sha")
            .gap(px(4.))
            .group("commit-sha")
            .hover(move |el| el.text_color(hover))
            .child(
                icon(Icon::Hash)
                    .size(px(14.))
                    .text_color(theme.text_2.opacity(0.7))
                    .group_hover("commit-sha", move |el| el.text_color(hover.opacity(0.7))),
            )
            .child(id.short())
            .tooltip(Tooltip::text("Copy commit ID"))
            .on_click(cx.listener(move |this, _, _, cx| {
                let sha = sha.clone();
                this.app.update(cx, |app, cx| app.copy_text(sha, cx)).ok();
            }))
    }

    /// The header of a commit only a remote has, branch `ref_name`'s or with `None` the
    /// target's: its title, who wrote it and when, its ID, and which remote has it.
    pub(super) fn render_upstream_header(
        &self,
        project: &ProjectId,
        ref_name: Option<&str>,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(ws) = self.store.read(cx).workspace(project) else {
            return div().into_any_element();
        };
        let Some(commit) = ws.upstream(ref_name).iter().find(|c| c.id == id) else {
            return div().into_any_element();
        };
        let remote = match ref_name {
            Some(ref_name) => ws.find_branch(ref_name).and_then(|b| b.remote.clone()),
            None => ws.target.as_ref().map(|target| target.name.clone()),
        };
        let title =
            page_title(Icon::ArrowDown, theme).child(title_text(commit_title(&commit.title)));
        let meta = meta_row(theme)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(byline_avatar(
                        &commit.author,
                        &commit.email,
                        commit.agent.as_ref(),
                        16.,
                        theme,
                    ))
                    .child(format!(
                        "{} at {}",
                        byline_text(&commit.author, commit.agent.as_ref()),
                        local_time(commit.time)
                    )),
            )
            .child(self.sha_chip(id, theme, cx))
            .child(div().text_color(theme.warn).child(match remote {
                Some(remote) => format!("only on {remote}"),
                None => "only on the remote".to_owned(),
            }));
        let main = v_flex().gap(px(10.)).child(title).child(meta);
        let target = Selection::Upstream(project.clone(), ref_name.map(str::to_owned), id);
        self.page_header(main, target, None, None, theme, cx)
    }

    /// The header across a page, over its files and its diff: `main` beside `extra`, if
    /// any, and a ⋯ with what right-clicking `target`'s row offers, for when the navigator is
    /// out of reach; as but.dev lays them out, so stepping between pages moves nothing below.
    /// `tabs`, if any, run along its bottom edge.
    pub(super) fn page_header(
        &self,
        main: Div,
        target: Selection,
        extra: Option<AnyElement>,
        tabs: Option<AnyElement>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let menu = icon_button("page-menu", Icon::Kebab).on_click(cx.listener(
            move |this, event: &ClickEvent, _, cx| {
                let position = event.position();
                (this.app)
                    .update(cx, |app, cx| app.open_row_menu(&target, position, cx))
                    .ok();
            },
        ));
        self.header_frame(main, menu, extra, tabs, theme)
    }

    /// [`Self::page_header`]'s frame, with `menu` for its ⋯: also the header beside a list,
    /// over the diff of what's selected in it.
    pub(super) fn header_frame(
        &self,
        main: Div,
        menu: impl IntoElement,
        extra: Option<AnyElement>,
        tabs: Option<AnyElement>,
        theme: &Theme,
    ) -> AnyElement {
        let menu = h_flex().h(px(22.)).gap(px(8.)).children(extra).child(menu);
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
        v_flex()
            .relative()
            .flex_none()
            .gap(px(10.))
            .px(px(16.))
            .pt(px(16.))
            .pb(px(if tabs.is_some() { 0. } else { 16. }))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                h_flex()
                    .items_start()
                    .gap(px(16.))
                    .child(main.flex_1().min_w(px(0.)))
                    .child(menu),
            )
            .children(tabs)
            .child(measure)
            .into_any_element()
    }

    /// A branch's header: its name, how many commits it has, where they stand against its
    /// remote, and any conflicted.
    pub(super) fn render_branch_header(
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
            return div().into_any_element();
        };
        let target = ws.target.as_ref().map_or("the target", |t| t.name.as_str());
        let (state, color) = match branch.push {
            PushState::Integrated => (format!("in {target}"), theme.text_2),
            // The remote has commits it doesn't: pulling comes first.
            _ if !branch.incoming.is_empty() => (branch.remote_state(), theme.warn),
            PushState::NothingToPush => (branch.remote_state(), theme.safe),
            _ => (branch.remote_state(), theme.blue),
        };
        let name = store.name_of(project, branch);
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        let meta = meta_row(theme).map(|el| match branch.commits.len() {
            0 => el.child(
                meta_item(Icon::Commit, theme)
                    .text_color(theme.text_3)
                    .child("No commits"),
            ),
            n => el
                .child(meta_item(Icon::Commit, theme).child(plural(n, "commit")))
                .child(div().text_color(color).child(state))
                .when(conflicted > 0, |el| {
                    el.child(
                        div()
                            .text_color(theme.danger)
                            .child(format!("{conflicted} conflicted")),
                    )
                }),
        });
        let main = v_flex()
            .gap(px(10.))
            .child(page_title(Icon::Branch, theme).child(title_text(name.to_owned())))
            .child(meta);
        let target = Selection::Branch(project.clone(), ref_name.to_owned());
        let tabs = (branch.review.as_ref()).map(|review| self.review_tabs(review, theme, cx));
        self.page_header(main, target, None, tabs, theme, cx)
    }

    /// A stack's header: how many branches it has, which, and their commits.
    pub(super) fn render_stack_header(
        &self,
        project: &ProjectId,
        bottom: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(branches) = store.workspace(project).and_then(|ws| ws.lane(bottom)) else {
            return div().into_any_element();
        };
        let names: Vec<&str> = (branches.iter())
            .map(|branch| store.name_of(project, branch))
            .collect();
        let commits = branches.iter().map(|branch| branch.commits.len()).sum();
        let meta = meta_row(theme)
            .child(meta_item(Icon::Commit, theme).child(match commits {
                0 => "No commits".to_owned(),
                n => plural(n, "commit"),
            }))
            .child(
                meta_item(Icon::Branch, theme)
                    .min_w(px(0.))
                    .child(div().min_w(px(0.)).truncate().child(names.join(" ← "))),
            );
        let title = format!("Stack of {}", names.len());
        let main = v_flex()
            .gap(px(10.))
            .child(page_title(Icon::Stack, theme).child(title_text(title)))
            .child(meta);
        let target = Selection::Stack(project.clone(), bottom.to_owned());
        self.page_header(main, target, None, None, theme, cx)
    }

    /// The box editing a commit's message, with how to save or leave it.
    fn reword_editor(&self, input: &Entity<TextInput>, theme: &Theme) -> AnyElement {
        v_flex()
            .gap(px(8.))
            .child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(theme.focus_border())
                    .line_height(px(19.))
                    .child(input.clone()),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .justify_end()
                    .child(faint("⌘↩ to save, esc to cancel", theme).flex_1())
                    .child({
                        let input = input.clone();
                        button("cancel-reword", "Cancel")
                            .small()
                            .on_click(move |_, _, cx| {
                                input.update(cx, |_, cx| cx.emit(InputEvent::Cancel))
                            })
                    })
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
            return empty_state(
                Some(Illustration::Papers),
                "Branch not found",
                "",
                theme,
                cx,
            );
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
                plural(conflicted, "conflicted commit"),
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
        // What the branch is heads its page: here, only what's to do about it.
        v_flex()
            .size_full()
            .children(resolve_oldest.map(|banner| side_header(theme).child(banner)))
            .child(files)
            .into_any_element()
    }
}

impl Detail {
    /// What a stack's branches change together: its branches, top first, and how many
    /// commits they have, over their files.
    pub(super) fn render_stack_side(
        &self,
        project: &ProjectId,
        bottom: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        if store
            .workspace(project)
            .and_then(|ws| ws.lane(bottom))
            .is_none()
        {
            return empty_state(Some(Illustration::Papers), "Stack not found", "", theme, cx);
        }
        // Its header says what the stack is; here, its files.
        self.render_diff_file_list(theme, cx)
    }
}

/// A crumb of the header: what it names, and what clicking it selects.
pub(super) type Crumb = (SharedString, Option<Selection>);

/// How tall a commit's header across the page is with its message closed, until it's drawn.
pub(super) const COMMIT_HEADER_HEIGHT: f32 = 85.;

/// The title row of a page's header: `icon`, then the title, which a caller adds, wrapping
/// rather than hiding its end.
pub(super) fn page_title(icon_name: Icon, theme: &Theme) -> Div {
    h_flex().items_start().gap(px(8.)).child(
        div()
            .h(px(22.))
            .flex()
            .items_center()
            .child(icon(icon_name).text_color(theme.text_1)),
    )
}

/// A page's title, as its header sets it.
pub(super) fn title_text(text: impl IntoElement) -> Div {
    div()
        .min_w(px(0.))
        .line_height(px(22.))
        .text_size(px(16.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(text)
}

/// The line under a page's title saying what it is.
pub(super) fn meta_row(theme: &Theme) -> Div {
    h_flex()
        .h(px(20.))
        .gap(px(12.))
        .text_size(px(13.))
        .text_color(theme.text_2)
}

/// One of a header's facts in its `meta_row`: its glyph at 0.7 of the line's ink, as
/// ui-react's meta glyphs are, then what's added after.
pub(super) fn meta_item(glyph: Icon, theme: &Theme) -> Div {
    h_flex().gap(px(3.)).child(
        icon(glyph)
            .size(px(14.))
            .text_color(theme.text_2.opacity(0.7)),
    )
}

/// The block at the top of a side panel, saying what's selected.
pub(super) fn side_header(theme: &Theme) -> Div {
    v_flex()
        .flex_none()
        .p(px(16.))
        .gap(px(6.))
        .border_b_1()
        .border_color(theme.border_3)
}

/// Says a checkout is stopped partway through Git's `op`, which committing there leaves
/// as it is.
pub(super) fn unfinished_banner(op: &str, theme: &Theme) -> Div {
    side_header(theme).child(super::resolve::warn_banner(
        format!("Git is partway through {op}"),
        format!("Finish it in a terminal with `{op} --continue`, or stop it with `{op} --abort`."),
        None,
        theme,
    ))
}

/// One of the lists beside the diff, drawn only as far as it's on screen, with a
/// scrollbar whose state goes by `scrollbar`. `row` draws the item at an index.
pub(super) fn side_list(
    id: &'static str,
    count: usize,
    scroll: &UniformListScrollHandle,
    scrollbar: &'static str,
    cx: &Context<Detail>,
    row: impl Fn(&Detail, usize, &Theme, &Window, &Context<Detail>) -> Option<AnyElement> + 'static,
) -> Div {
    let list = uniform_list(
        id,
        count,
        cx.processor(move |this, range: Range<usize>, window, cx| {
            let theme = Theme::get(cx);
            range
                .filter_map(|ix| row(this, ix, theme, window, cx))
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

/// A row of a page's side list: its selection solid while the list holds the focus, as the
/// navigator's and the file list's are.
pub(super) fn side_row(
    id: impl Into<ElementId>,
    selected: bool,
    solid: bool,
    theme: &Theme,
) -> Stateful<Div> {
    list_row(id, selected, theme).when(selected && solid, |el| {
        el.bg(theme.fill_gray).text_color(theme.text_inverted)
    })
}

/// `el`, a row's secondary text, softened against a solid selection `on` it.
pub(super) fn quiet(el: gpui::Div, on: bool, theme: &Theme) -> gpui::Div {
    el.flex_none()
        .when(on, |el| el.text_color(theme.text_inverted.opacity(0.7)))
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

/// What a side list shows in place of its rows, in the room they'd take.
pub(super) fn list_empty(
    drawing: Illustration,
    title: String,
    detail: &'static str,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    div()
        .flex_1()
        .min_h(px(0.))
        .child(empty_state(Some(drawing), title, detail, theme, cx))
        .into_any_element()
}
