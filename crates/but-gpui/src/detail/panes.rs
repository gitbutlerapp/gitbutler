//! The column beside the diff and the header above both: what's selected, its files,
//! and the actions that fit it.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, Entity, FocusHandle, Focusable as _,
    MouseDownEvent, Pixels, Point, SharedString, Stateful, UniformListScrollHandle, Window, div,
    prelude::*, uniform_list,
};

use super::upstream::noon;
use super::{Detail, machine};
use crate::app::{AppHandle, Collapse, Confirm, Expand, SelectNext, SelectPrevious, StartCommit};
use crate::backend::upstream::{self, ACTIVITY_DAYS};
use crate::backend::{self, Checkout, CommitId, CommitState, FullName, PushState, short_ref};
use crate::navigator::{SelectFirst, SelectLast};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Section, Selection};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::person::{author_avatar, byline};
use crate::ui::scrollbar::{Scroll, scrollbar, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    Button, ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, WINDOW_CONTROLS_WIDTH,
    above_drag, age_column, button, commit_title, dp, empty_state, faint, field, fixed, fold_row,
    h_flex, icon, icon_button, list_row, plural, quiet, section_label, side_row, small_icon,
    v_flex,
};

impl Detail {
    pub(super) fn render_header(
        &self,
        selection: Option<&Selection>,
        theme: &Theme,
        window: &Window,
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
                Some(Selection::Project(entry.id.clone()).into()),
            ));
            let workspace = entry.workspace.as_ref();
            // The worktree a branch, stack or commit is in, named as its navigator heading is.
            let heading = |item: &Selection| -> Crumb {
                let project = item.project().clone();
                match store.worktree_for(item) {
                    Some(worktree) => (
                        worktree.name.to_string().into(),
                        Some(Selection::Worktree(project, worktree.name.clone()).into()),
                    ),
                    None => (
                        "Main worktree".into(),
                        Some(Selection::Uncommitted(project).into()),
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
                    // While its changes show, it goes back to its overview, as Esc does.
                    None => crumbs.push((
                        "Main worktree".into(),
                        (!self.showing_now(cx)).then(|| selection.clone().into()),
                    )),
                },
                Selection::Branch(project, ref_name) => {
                    let branch = workspace.and_then(|ws| ws.find_branch(ref_name.as_ref()));
                    let name = match (branch, ref_name) {
                        (Some(branch), _) => store.name_of(project, branch).into(),
                        (None, None) => "Unnamed branch".into(),
                        (None, Some(ref_name)) => short_ref(ref_name),
                    };
                    crumbs.push(heading(selection));
                    crumbs.push((name.into_owned().into(), None));
                }
                Selection::Branches(project) => {
                    crumbs.push(("Branches".into(), Some(selection.clone().into())));
                    self.branches_crumbs(project, &mut crumbs);
                }
                Selection::Worktree(_, name) => {
                    let back = (!self.showing_now(cx)).then(|| selection.clone().into());
                    crumbs.push((name.to_string().into(), back));
                }
                Selection::Mesh(project, id, pick) => {
                    machine::mesh_crumbs(store, project, id, pick, &mut crumbs)
                }
                Selection::Stack(_, bottom) => {
                    let n = workspace
                        .and_then(|ws| ws.lane(bottom.as_ref()))
                        .map_or(0, |lane| lane.branches.len());
                    crumbs.push(heading(selection));
                    crumbs.push((format!("Stack of {n}").into(), None));
                }
                // Then the operation picked, as the Branches page names its branch.
                Selection::History(project) => {
                    let operation = self.selected_operation(project);
                    let back = operation.map(|_| selection.clone().into());
                    crumbs.push(("Operations history".into(), back));
                    if let Some(operation) = operation {
                        crumbs.push((operation.title.clone().into(), None));
                    }
                }
                Selection::Section(project, section) => {
                    let label = match section {
                        Section::Inactive => "Inactive worktrees".to_owned(),
                        Section::Mesh => "Network worktrees".to_owned(),
                        // After the branch, as its commits have it; the target's after the
                        // project.
                        Section::Upstream(ref_name) => match ref_name
                            .as_ref()
                            .and_then(|r| Some((r, workspace?.find_branch(Some(r))?)))
                        {
                            Some((ref_name, branch)) => {
                                let branch_selection =
                                    Selection::Branch(project.clone(), Some(ref_name.clone()));
                                crumbs.push(heading(&branch_selection));
                                let name = store.name_of(project, branch).to_owned();
                                crumbs.push((name.into(), Some(branch_selection.into())));
                                (branch.remote.as_ref())
                                    .map_or("On the remote".to_owned(), |r| format!("On {r}"))
                            }
                            None => {
                                let target = workspace.and_then(|ws| ws.target_name());
                                format!("New on {}", target.unwrap_or("the target".into()))
                            }
                        },
                    };
                    crumbs.push((label.into(), None));
                }
                // After what has it, the branch or the target, which go to their pages.
                Selection::Upstream(project, ref_name, id) => {
                    match ref_name
                        .as_ref()
                        .and_then(|r| Some((r, workspace?.find_branch(Some(r))?)))
                    {
                        Some((ref_name, branch)) => {
                            let branch_selection =
                                Selection::Branch(project.clone(), Some(ref_name.clone()));
                            crumbs.push(heading(&branch_selection));
                            let name = store.name_of(project, branch).to_owned();
                            crumbs.push((name.into(), Some(branch_selection.into())));
                        }
                        None => {
                            let name = workspace.and_then(|ws| ws.target_name());
                            let page = Selection::Project(project.clone());
                            let name = name.unwrap_or("Target".into()).into_owned();
                            crumbs.push((name.into(), Some(page.into())));
                        }
                    }
                    // A commit a merge brought in, after the merge, as the project's page has it.
                    if let Some((merge, _)) = store.merged_upstream(project, ref_name.as_ref(), *id)
                    {
                        let up = Selection::Upstream(project.clone(), ref_name.clone(), merge);
                        crumbs.push((merge.short().into(), Some(up.into())));
                    }
                    crumbs.push((id.short().into(), None));
                }
                Selection::Commit(project, id) => {
                    if let Some((branch, commit)) = workspace.and_then(|ws| ws.find_commit(*id)) {
                        let branch_selection = (branch.ref_name.clone()).map(|ref_name| {
                            Selection::Branch(project.clone(), Some(ref_name)).into()
                        });
                        crumbs.push(heading(selection));
                        let name = store.name_of(project, branch).to_owned();
                        crumbs.push((name.into(), branch_selection));
                        let last = match store.among_picks(project, *id) {
                            true => plural(store.picked.len(), "commit"),
                            false => commit.id.short(),
                        };
                        crumbs.push((last.into(), None));
                    }
                }
            }
        }
        // Then the file under the diff's cursor, while the diff of what's selected shows.
        let shown = (self.diff.as_ref())
            .is_some_and(|diff| Some(diff.key.clone()) == self.key_for_selection(cx));
        if let Some(path) = self
            .cursor_path()
            .filter(|_| shown && !self.diff_hidden(cx))
        {
            let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
            crumbs.push((name.into(), None));
        }
        let last = crumbs.len().saturating_sub(1);
        // As ui-react's LocationBar: on the window's second ground, the navigator's toggle, a
        // hairline, then the trail, every crumb quiet and joined by chevrons.
        let toggle = icon_button("toggle-navigator", Icon::Sidebar)
            .tooltip("Toggle navigator")
            .dispatch(&crate::app::ToggleNavigator);
        h_flex()
            .h(dp(HEADER_HEIGHT))
            .flex_none()
            .px(dp(12.))
            .when(
                self.beside_traffic_lights && cfg!(target_os = "macos"),
                |el| el.pl(fixed(84.)),
            )
            // Clear of the window's buttons, where the app draws them over its right end.
            .when(crate::ui::draws_window_controls(window), |el| {
                el.pr(fixed(WINDOW_CONTROLS_WIDTH + 12.))
            })
            .map(crate::ui::drags_window)
            .gap(dp(12.))
            .bg(theme.bg_2)
            .border_b_1()
            .border_color(theme.border_3)
            .text_color(theme.text_2)
            .child(above_drag(div().child(toggle)))
            .child(div().flex_none().w(fixed(1.)).h(dp(16.)).bg(theme.border_3))
            .child(
                h_flex().flex_1().min_w_0().gap(dp(6.)).children(
                    crumbs
                        .into_iter()
                        .enumerate()
                        .flat_map(|(ix, (crumb, target))| {
                            let sep = (ix > 0).then(|| {
                                small_icon(Icon::ChevronRight)
                                    .size(dp(10.))
                                    .text_color(theme.text_3)
                                    .into_any_element()
                            });
                            // Short ones, like the project's name or a commit's ID, stay whole
                            // while long ones can give way, by length as well as width.
                            let shrink = match crumb.chars().count() as f32 {
                                n if n <= 16. => 0.01,
                                n => n,
                            };
                            let text = (div().truncate()).flex_shrink(shrink).child(crumb);
                            let el = match target.filter(|_| ix != last) {
                                Some(target) => {
                                    // A step darker under the pointer: it's still a quiet trail.
                                    let hover = theme.text_2.blend(theme.text_1.opacity(0.5));
                                    above_drag(text.id(("crumb", ix)))
                                        .hover(move |el| el.text_color(hover))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            match &target {
                                                CrumbTo::Select(selection) => {
                                                    let selection = selection.clone();
                                                    this.app.update_later(
                                                        window,
                                                        cx,
                                                        move |app, window, cx| {
                                                            app.select(selection, window, cx)
                                                        },
                                                    )
                                                }
                                                CrumbTo::OtherBranch(ref_name) => this
                                                    .select_other_branch(
                                                        Some(ref_name.clone()),
                                                        cx,
                                                    ),
                                                CrumbTo::Merge(id) => {
                                                    this.pick_target_commit(*id, cx)
                                                }
                                            }
                                        }))
                                        .into_any_element()
                                }
                                None => text.into_any_element(),
                            };
                            sep.into_iter().chain([el])
                        }),
                ),
            )
    }

    /// A list's filter field, over the list.
    pub(super) fn filter_field(
        &self,
        input: &Entity<TextInput>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let focused = input.focus_handle(cx).is_focused(window);
        h_flex()
            .flex_none()
            .m(dp(10.))
            .gap(dp(4.))
            .child(field(input, focused, Theme::get(cx)).flex_1())
    }

    /// A side list's filter field, then the button whose menu of `items` picks what the list
    /// shows, lit while that leaves some out.
    pub(super) fn filter_bar(
        &self,
        input: &Entity<TextInput>,
        tooltip: &'static str,
        items: fn(&Self, &mut Context<Self>) -> Vec<MenuItem>,
        narrowed: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        self.filter_field(input, window, cx).child(
            icon_button("list-filters", Icon::Filter)
                .small()
                .selected(narrowed)
                .tooltip(tooltip)
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    this.open_menu(event.position(), move |this, cx| items(this, cx), cx)
                })),
        )
    }

    /// Fetch, as ⌘R does, to go beside a list's filter: nothing for a project whose folder is
    /// gone. Without a remote, a fetch says there's none.
    pub(super) fn fetch_button(&self, project: &ProjectId, cx: &Context<Self>) -> Option<Button> {
        let store = self.store.read(cx);
        if store.project(project)?.folder_gone {
            return None;
        }
        let busy = store.fetching(project);
        Some(
            icon_button("fetch", Icon::Refresh)
                .small()
                .tooltip_for(
                    "Fetch, and check pull requests and CI again",
                    &crate::app::Fetch,
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
                    let at = Selection::Uncommitted(project.clone());
                    let finish = move |this: &mut Self, cx: &mut Context<Self>| {
                        if !this.apply_resolutions(true, cx) {
                            (this.store).update(cx, |store, cx| store.finish_editing(&project, cx))
                        }
                    };
                    if left == 0 {
                        return finish(this, cx);
                    }
                    let detail = cx.entity();
                    this.app.update_later(window, cx, move |app, window, cx| {
                        app.confirm_then(
                            Some(at),
                            format!("Finish with {} still conflicted?", plural(left, "file")),
                            "They go into the commit as they are now, with any conflict markers.",
                            "Finish",
                            window,
                            cx,
                            move |_, cx| detail.update(cx, finish),
                        )
                    });
                }))
                .into_any_element(),
        ]
    }

    /// What an agent left uncommitted in the project's `checkout`, laid out to read first. The
    /// commit box stays closed until asked for, and only then do checkboxes appear.
    pub(super) fn render_uncommitted_side(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            // One that couldn't be read says so here too, rather than seeming to load.
            let entry = store.project(project);
            // Which branch is the target is asked beside.
            if entry.is_some_and(|entry| entry.target_choice.is_some()) {
                let said = "Its files show here once the target's picked";
                return empty_state(None, "", said, theme, cx);
            }
            return match entry.and_then(|entry| entry.error.clone()) {
                Some(err) => empty_state(None, "Couldn't read the project", err, theme, cx),
                None => crate::ui::loading_state("Loading…", theme, cx),
            };
        };
        let (files, git_operation) =
            match checkout.linked().map(|name| workspace.find_worktree(name)) {
                None => (&workspace.uncommitted, workspace.git_operation),
                Some(Some(worktree)) => (&worktree.uncommitted, worktree.git_operation),
                Some(None) => return not_found("Worktree not found", theme, cx),
            };
        // A commit being edited, in the main checkout, is finished or cancelled where a commit
        // would be made.
        let editing = !checkout.is_linked() && workspace.editing.is_some();
        let footer = if editing {
            footer(
                "Editing the commit",
                self.editing_actions(project, cx),
                theme,
            )
        } else if !self.composing {
            let target = self.effective_target(project, cx);
            let target = match (target.as_ref(), checkout) {
                (Some(target), _) => format!("Commit to {}", short_ref(target)),
                (None, Checkout::Main) => "Commit to a new branch".to_owned(),
                (None, Checkout::Linked(_)) => "No branch of its own checked out".to_owned(),
            };
            let open = button("open-composer", "Commit…")
                .small()
                .tooltip_for("Write a commit message", &StartCommit)
                .on_click(cx.listener(|this, _, window, cx| this.focus_message(window, cx)));
            footer(target, [open], theme)
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
            .child(self.render_uncommitted_files(project, checkout, files, theme, cx))
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
            return not_found("Commit not found", theme, cx);
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
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((_, commit)) = store.find_commit(project, id) else {
            return div().into_any_element();
        };
        let (title, message) = store.message_of(project, commit);
        let body = backend::subject_and_body(message).1;
        let target = (store.workspace(project).and_then(|ws| ws.target.as_ref()))
            .map_or("the target", |t| t.name.as_str());
        let (state, color) = match commit.state {
            CommitState::Local => ("not pushed".to_owned(), theme.blue),
            CommitState::Pushed => ("on the remote".to_owned(), theme.safe),
            CommitState::Integrated => (format!("in {target}"), theme.text_2),
        };
        // A long title wraps rather than hiding its end, as it's shown nowhere else, and
        // scrolls past a dozen lines.
        let title = page_title(Icon::Commit, theme).child(self.message_title(title, &body, cx));
        let meta = meta_row(theme)
            .child(byline(
                &commit.author,
                &commit.email,
                commit.agent.as_ref(),
                commit.time,
                theme,
            ))
            .child(self.sha_chip(id, theme, cx))
            .child(div().text_color(color).child(state))
            .when(commit.conflicted, |el| {
                el.child(div().text_color(theme.warn).child("conflicted"))
            })
            // Last, as it's read after the rest, so nothing moves when it shows.
            .when_some(self.signers.get(&id).copied().flatten(), |el, signer| {
                el.child(
                    meta_item(Icon::Lock, theme)
                        .id("commit-signed")
                        .text_color(theme.text_3)
                        .child("Signed")
                        .tooltip(Tooltip::text(format!("Signed with {signer}, not verified"))),
                )
            });
        let rewording = (self.rewording.as_ref())
            .filter(|(reworded, ..)| super::composer::rewords(reworded, project, commit, store));
        let title = match rewording {
            Some((_, input, _)) => self.reword_editor(input, theme, window, cx),
            // A double-click on its title or body edits the message.
            None => (self.message(title, body, theme))
                .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.start_reword(window, cx)
                    }
                }))
                .into_any_element(),
        };
        let target = Selection::Commit(project.clone(), id);
        let apply = super::resolve::apply_button(self, cx.entity().downgrade());
        self.page_header(title, meta, target, apply, None, theme, cx)
    }

    /// A commit's title, with a button that shows the rest of its message, `body`, if any.
    fn message_title(&self, title: &str, body: &str, cx: &Context<Self>) -> Div {
        let open = self.message_open;
        let toggle = (!body.is_empty()).then(|| {
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
                }))
        });
        (h_flex().items_start().min_w_0().gap(dp(6.)))
            .child(title_text(commit_title(title)))
            .children(toggle)
    }

    /// A commit's `title` and, once its button shows it, the rest of its message, `body`, under
    /// it, scrolling past a dozen lines: an agent's message can run to thousands of characters
    /// on one line, and the files and diff under the header stay in reach.
    fn message(&self, title: impl IntoElement, body: String, theme: &Theme) -> Stateful<Div> {
        (v_flex()
            .id("message")
            .gap(dp(10.))
            .max_h(dp(260.))
            .overflow_y_scroll())
        .child(title)
        .children(self.full_message(body, theme))
    }

    /// The rest of a commit's message under its title, once its button shows it.
    fn full_message(&self, body: String, theme: &Theme) -> Option<Div> {
        // Further off the lines around it than they stand from each other.
        (self.message_open && !body.is_empty()).then(|| {
            div()
                .mt(dp(2.))
                .mb(dp(6.))
                .max_w(dp(580.))
                .font_family(theme.mono_font.clone())
                .text_size(dp(12.))
                .line_height(dp(19.))
                .child(body)
        })
    }

    /// The header of the commits picked together: how many, the branches they're on and how
    /// many are conflicted. Their changes show together when they're one run in one lane;
    /// otherwise each one's in turn, which it says. `id` is the selected one.
    pub(super) fn render_picks_header(
        &self,
        project: &ProjectId,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return div().into_any_element();
        };
        let picked: Vec<_> = (ws.all_branches())
            .flat_map(|branch| branch.commits.iter().map(move |commit| (branch, commit)))
            .filter(|(_, commit)| store.picked.contains(&commit.id))
            .collect();
        let mut names: Vec<&str> = (picked.iter())
            .map(|(branch, _)| store.name_of(project, branch))
            .collect();
        names.dedup();
        let conflicted = picked
            .iter()
            .filter(|(_, commit)| commit.conflicted)
            .count();
        let one_run = store.picked_run(project).is_some();
        let meta = meta_row(theme)
            .child(
                meta_item(Icon::Branch, theme)
                    .min_w_0()
                    .child(div().min_w_0().truncate().child(names.join(", "))),
            )
            .children(conflicted_note(conflicted, theme))
            .when(!one_run, |el| {
                el.child(
                    div()
                        .text_color(theme.text_3)
                        .child("With others between them, so each one's changes show in turn"),
                )
            });
        let title =
            page_title(Icon::Commit, theme).child(title_text(plural(picked.len(), "commit")));
        let target = Selection::Commit(project.clone(), id);
        self.page_header(title, meta, target, None, None, theme, cx)
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
            .gap(dp(4.))
            .group("commit-sha")
            .hover(move |el| el.text_color(hover))
            .child(
                icon(Icon::Hash)
                    .size(dp(14.))
                    .text_color(theme.text_2.opacity(0.7))
                    .group_hover("commit-sha", move |el| el.text_color(hover.opacity(0.7))),
            )
            .child(id.short())
            .tooltip(Tooltip::text("Copy commit ID"))
            .on_click(cx.listener(move |this, _, window, cx| {
                let sha = sha.clone();
                (this.app).update_later(window, cx, move |app, _, cx| app.copy_text(sha, cx));
            }))
    }

    /// The header of a commit only a remote has, branch `ref_name`'s or with `None` the
    /// target's: its title, who wrote it and when, its ID, and which remote has it.
    pub(super) fn render_upstream_header(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return div().into_any_element();
        };
        // Listed, or brought in by a merge that is.
        let listed = ws.upstream(ref_name).iter().find(|c| c.id == id);
        let (title, body, author, email, agent, time) = match listed {
            Some(c) => (
                &c.title,
                &c.body,
                &c.author,
                &c.email,
                c.agent.as_ref(),
                c.time,
            ),
            None => {
                let Some((_, c)) = store.merged_upstream(project, ref_name, id) else {
                    return div().into_any_element();
                };
                (
                    &c.title,
                    &c.body,
                    &c.author,
                    &c.email,
                    c.agent.as_ref(),
                    c.time,
                )
            }
        };
        let remote = match ref_name {
            Some(ref_name) => ws
                .find_branch(Some(ref_name))
                .and_then(|b| b.remote.clone()),
            None => ws.target.as_ref().map(|target| target.name.clone()),
        };
        let title = page_title(Icon::ArrowDown, theme).child(self.message_title(title, body, cx));
        let meta = meta_row(theme)
            .child(byline(author, email, agent, time, theme))
            .child(self.sha_chip(id, theme, cx))
            .child(div().text_color(theme.warn).child(match remote {
                Some(remote) => format!("only on {remote}"),
                None => "only on the remote".to_owned(),
            }));
        let message = self.message(title, body.clone(), theme);
        let target = Selection::Upstream(project.clone(), ref_name.cloned(), id);
        let tabs = (self.selected_review(cx)).map(|review| self.review_tabs(review, theme, cx));
        self.page_header(message, meta, target, None, tabs, theme, cx)
    }

    /// The header across a page, over its files and its diff: `title` beside `extra`, if
    /// any, and a ⋯ with what right-clicking `target`'s row offers, for when the navigator is
    /// out of reach, over `body` across the page; as but.dev lays them out, so stepping between
    /// pages moves nothing below. `tabs`, if any, run along its bottom edge.
    #[expect(clippy::too_many_arguments)]
    pub(super) fn page_header(
        &self,
        title: impl IntoElement,
        body: Div,
        target: Selection,
        extra: Option<AnyElement>,
        tabs: Option<AnyElement>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let menu = icon_button("page-menu", Icon::Kebab).on_click(cx.listener(
            move |this, event: &ClickEvent, window, cx| {
                // What right-clicking its row in the navigator offers.
                let (target, at) = (target.clone(), event.position());
                this.app.update_later(window, cx, move |app, _, cx| {
                    let items = app.navigator.read(cx).menu_items(&target, cx);
                    app.open_menu(at, items, cx)
                });
            },
        ));
        self.header_frame(title, body, menu, extra, tabs, theme)
    }

    /// [`Self::page_header`]'s frame, with `menu` for its ⋯: also the header beside a list,
    /// over the diff of what's selected in it.
    pub(super) fn header_frame(
        &self,
        title: impl IntoElement,
        body: Div,
        menu: impl IntoElement,
        extra: Option<AnyElement>,
        tabs: Option<AnyElement>,
        theme: &Theme,
    ) -> AnyElement {
        let menu = h_flex().h(dp(22.)).gap(dp(8.)).children(extra).child(menu);
        // The diff beside the files starts below this, however tall it's drawn, and the rule
        // along its bottom, which this, inside the header, doesn't cover.
        let measure = measured(&self.header_height, &self.app, |bounds| {
            f32::from(bounds.size.height) + 1.
        });
        v_flex()
            .relative()
            .flex_none()
            .gap(dp(10.))
            .px(dp(16.))
            .pt(dp(16.))
            .pb(dp(if tabs.is_some() { 0. } else { 16. }))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                h_flex()
                    .items_start()
                    .gap(dp(16.))
                    .child(div().flex_1().min_w_0().child(title))
                    .child(menu),
            )
            .child(body)
            .children(tabs)
            .child(measure)
            .into_any_element()
    }

    /// A branch's header: its name, how many commits it has, where they stand against its
    /// remote, and any conflicted.
    pub(super) fn render_branch_header(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
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
                .children(conflicted_note(conflicted, theme)),
        });
        let target = Selection::Branch(project.clone(), ref_name.cloned());
        let create = crate::pr::create_offer(store, &target).map(|offer| {
            button("header-create-pr", "Create pull request")
                .small()
                .style(ButtonStyle::Pop)
                .tooltip("Push and open a pull request")
                .dispatch(&crate::pr::CreatePullRequest)
                .disabled_because(offer.err())
        });
        let force = ref_name.is_some_and(|r| ws.needs_force(r));
        let push = button("header-push", ref_name.map_or("Push", |r| ws.push_label(r)))
            .small()
            .tooltip(match force {
                true => "Replace the remote's version",
                false => "Push to the remote",
            })
            .dispatch(&crate::app::Push)
            .disabled_because(ws.push_blocked(ref_name));
        let meta = meta
            .child(div().flex_1())
            .child(h_flex().gap(dp(8.)).children(create).child(push));
        let title = page_title(Icon::Branch, theme).child(title_text(name.to_owned()));
        let tabs = (branch.review.as_ref()).map(|review| self.review_tabs(review, theme, cx));
        self.page_header(title, meta, target, None, tabs, theme, cx)
    }

    /// A stack's header: how many branches it has, which, and their commits.
    pub(super) fn render_stack_header(
        &self,
        project: &ProjectId,
        bottom: Option<&FullName>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(lane) = store.workspace(project).and_then(|ws| ws.lane(bottom)) else {
            return div().into_any_element();
        };
        let branches = &lane.branches;
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
                    .min_w_0()
                    .child(div().min_w_0().truncate().child(names.join(" ← "))),
            );
        let title = format!("Stack of {}", names.len());
        let title = page_title(Icon::Stack, theme).child(title_text(title));
        let target = Selection::Stack(project.clone(), bottom.cloned());
        self.page_header(title, meta, target, None, None, theme, cx)
    }

    /// The box editing a commit's message, with how to save or leave it.
    fn reword_editor(
        &self,
        input: &Entity<TextInput>,
        theme: &Theme,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let focused = input.focus_handle(cx).is_focused(window);
        v_flex()
            .gap(dp(8.))
            .child(
                field(input, focused, theme)
                    .px(dp(10.))
                    .py(dp(8.))
                    .rounded(dp(6.))
                    .line_height(dp(19.))
                    .id("reword-box")
                    .max_h(dp(260.))
                    .overflow_y_scroll()
                    .track_scroll(&self.reword_scroll),
            )
            .child(
                h_flex()
                    .gap(dp(8.))
                    .justify_end()
                    .child(faint(crate::ui::keys_text("⌘↩ to save, esc to cancel"), theme).flex_1())
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
        ref_name: Option<&FullName>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((ws, branch)) =
            (store.workspace(project)).and_then(|ws| Some((ws, ws.find_branch(ref_name)?)))
        else {
            return not_found("Branch not found", theme, cx);
        };
        let files = self.render_diff_file_list(theme, cx);
        // Resolving the oldest conflict first, even in a branch below or one the lane rests on,
        // rebases the rest onto its result.
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        let oldest = (ref_name.filter(|_| conflicted > 0))
            .and_then(|ref_name| super::resolve::oldest_conflict(ws, ref_name));
        let resolve_oldest = oldest.map(|(holder, commit)| {
            let app = self.app.clone();
            let (project, id) = (project.clone(), commit.id);
            super::resolve::warn_banner(
                plural(conflicted, "conflicted commit"),
                match holder.ref_name.as_ref() == ref_name {
                    true if conflicted == 1 => "Resolve opens it to pick what it keeps.".to_owned(),
                    true => {
                        "Start with the oldest: the ones above it are rebuilt on it.".to_owned()
                    }
                    false => format!("The oldest is in {}, below this branch.", holder.name),
                },
                Some(
                    super::resolve::banner_action("resolve", "Resolve", theme)
                        .on_click(move |_, window, cx| {
                            let project = project.clone();
                            app.update_later(window, cx, move |app, window, cx| {
                                app.resolve_commit(project, id, window, cx)
                            });
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

    /// The files a stack's branches change together. Its header says what the stack is.
    pub(super) fn render_stack_side(
        &self,
        project: &ProjectId,
        bottom: Option<&FullName>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        if store
            .workspace(project)
            .and_then(|ws| ws.lane(bottom))
            .is_none()
        {
            return not_found("Stack not found", theme, cx);
        }
        self.render_diff_file_list(theme, cx)
    }
}

/// A crumb of the header: what it names, and where clicking it goes.
pub(super) type Crumb = (SharedString, Option<CrumbTo>);

/// Where clicking a crumb goes: what it names, selected, or picked on the page shown.
pub(super) enum CrumbTo {
    Select(Selection),
    /// On the Branches page, the branch over the commit of it picked.
    OtherBranch(FullName),
    /// On a project's page, the merge over the commit of it shown.
    Merge(CommitId),
}

impl From<Selection> for CrumbTo {
    fn from(selection: Selection) -> Self {
        Self::Select(selection)
    }
}

/// How tall a commit's header across the page is with its message closed, until it's drawn.
pub(super) const COMMIT_HEADER_HEIGHT: f32 = 85.;

/// The title row of a page's header: `icon`, then the title, which a caller adds, wrapping
/// rather than hiding its end.
pub(super) fn page_title(icon_name: Icon, theme: &Theme) -> Div {
    h_flex().min_w_0().items_start().gap(dp(8.)).child(
        div()
            .h(dp(22.))
            .flex()
            .items_center()
            .child(icon(icon_name).text_color(theme.text_1)),
    )
}

/// A page's title, as its header sets it.
pub(super) fn title_text(text: impl IntoElement) -> Div {
    div()
        .min_w_0()
        .line_height(dp(22.))
        .text_size(dp(16.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(text)
}

/// The line under a page's title saying what it is.
pub(super) fn meta_row(theme: &Theme) -> Div {
    h_flex()
        .h(dp(20.))
        .gap(dp(12.))
        .text_size(dp(13.))
        .text_color(theme.text_2)
}

/// One of a header's facts in its `meta_row`: its glyph at 0.7 of the line's ink, as
/// ui-react's meta glyphs are, then what's added after.
pub(super) fn meta_item(glyph: Icon, theme: &Theme) -> Div {
    h_flex().gap(dp(3.)).child(
        icon(glyph)
            .size(dp(14.))
            .text_color(theme.text_2.opacity(0.7)),
    )
}

/// A row of an overview, which opens `at` in the navigator, the navigator taking the keys, as a
/// crumb does.
pub(super) fn overview_row(
    id: impl Into<gpui::ElementId>,
    at: Selection,
    cx: &Context<Detail>,
) -> Stateful<Div> {
    let theme = Theme::get(cx);
    list_row(id, false, theme).on_click(cx.listener(move |this, _, window, cx| {
        let at = at.clone();
        (this.app).update_later(window, cx, move |app, window, cx| {
            app.select(at, window, cx)
        })
    }))
}

/// How many lanes an overview draws before saying how many more: the navigator lists them all.
const LANES_SHOWN: usize = 12;

/// A worktree on an overview, drawn as the Branches page draws a branch's lane.
pub(super) struct Lane {
    /// What a click on it opens.
    pub(super) at: Selection,
    pub(super) glyph: Icon,
    pub(super) name: String,
    /// A word on where it stands, in its colour.
    pub(super) tag: Option<(String, gpui::Hsla)>,
    /// Its branches, faint.
    pub(super) branches: String,
    /// When its commits were made, in seconds since the epoch.
    pub(super) commits: Vec<i64>,
    /// When it last changed.
    pub(super) changed: Option<i64>,
}

/// An overview's worktrees, `total` of them: the first of `lanes` each with a strip of its
/// commits a day, shaded against the busiest day of any, and when it last changed; then how
/// many more there are.
pub(super) fn worktree_lanes(
    lanes: impl Iterator<Item = Lane>,
    total: usize,
    theme: &Theme,
    cx: &Context<Detail>,
) -> Div {
    let today = upstream::day_of(crate::ui::now());
    let lanes: Vec<(Lane, Vec<u32>)> = (lanes.take(LANES_SHOWN))
        .map(|lane| {
            let days = daily(&lane.commits, today);
            (lane, days)
        })
        .collect();
    let most = (lanes.iter().flat_map(|(_, days)| days.iter().copied()))
        .max()
        .unwrap_or(1)
        .max(1);
    let rows = (lanes.into_iter().enumerate()).map(|(ix, (lane, days))| {
        overview_row(("lane", ix), lane.at, cx)
            .h_auto()
            .py(dp(4.))
            .child(icon(lane.glyph).text_color(theme.text_2))
            .child(
                h_flex()
                    .flex_1()
                    .flex_grow(2.)
                    .min_w_0()
                    .overflow_hidden()
                    .gap(dp(6.))
                    .child(div().min_w(dp(40.)).truncate().child(lane.name))
                    .children(lane.tag.map(|(said, color)| {
                        div()
                            .flex_none()
                            .text_size(dp(11.))
                            .text_color(color)
                            .child(said)
                    }))
                    .child(faint(lane.branches, theme).min_w_0().truncate()),
            )
            .child(day_strip(&days, most, today, theme))
            .child(age_column(lane.changed, theme))
    });
    let said = format!("commits a day, last {ACTIVITY_DAYS} days · last change");
    v_flex()
        .gap(dp(6.))
        .child(overview_heading("Worktrees", faint(said, theme), theme))
        .children(rows)
        .children(and_more(total.saturating_sub(LANES_SHOWN), theme))
}

/// Commits a day over the activity's days up to `today`, oldest first, from their `times` in
/// seconds; one dated ahead of this machine's clock counts as today.
fn daily(times: &[i64], today: i64) -> Vec<u32> {
    let first_day = today - (ACTIVITY_DAYS as i64 - 1);
    let mut days = vec![0; ACTIVITY_DAYS];
    for at in times {
        let day = upstream::day_of(*at).min(today);
        if let Some(n) = usize::try_from(day - first_day)
            .ok()
            .and_then(|ix| days.get_mut(ix))
        {
            *n += 1;
        }
    }
    days
}

/// A lane's strip, as the Branches page draws one: a cell for each of `days` up to `today`,
/// shaded by how many commits it had against the `most` any lane had, worded on hover.
/// The cells sit at their share of the strip rather than in a flex row, which costs the
/// page's layout half as much.
fn day_strip(days: &[u32], most: u32, today: i64, theme: &Theme) -> Div {
    let first_day = today - (days.len() as i64 - 1);
    let count = days.len() as f32;
    let cells = days.iter().enumerate().map(|(ix, &n)| {
        let day = first_day + ix as i64;
        let shade = (n as f32 / most.max(1) as f32).sqrt();
        div()
            .id(("lane-day", ix))
            .absolute()
            .top_0()
            .bottom_0()
            .left(gpui::relative(ix as f32 / count))
            .right(gpui::relative((count - 1. - ix as f32) / count))
            .mx(dp(1.))
            .rounded(dp(2.))
            .bg(match n {
                0 => theme.border_3.opacity(0.6),
                _ => theme.pop.opacity(0.3 + 0.7 * shade),
            })
            .tooltip(move |window, cx| {
                let day = crate::ui::local_weekday(noon(day));
                Tooltip::text(format!("{day}: {}", plural(n as usize, "commit")))(window, cx)
            })
    });
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .max_w(dp(360.))
        .h(dp(14.))
        .children(cells)
}

/// The block at the top of a side panel, saying what's selected.
pub(super) fn side_header(theme: &Theme) -> Div {
    v_flex()
        .flex_none()
        .p(dp(16.))
        .gap(dp(6.))
        .border_b_1()
        .border_color(theme.border_3)
}

/// The bar along the bottom of a checkout's uncommitted changes: what committing there does,
/// in `label`, and the buttons that do it.
fn footer(
    label: impl IntoElement,
    actions: impl IntoIterator<Item = impl IntoElement>,
    theme: &Theme,
) -> AnyElement {
    h_flex()
        .flex_none()
        .h(dp(super::FOOTER_HEIGHT))
        .px(dp(12.))
        .gap(dp(8.))
        .border_t_1()
        .border_color(theme.border_3)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(dp(12.))
                .text_color(theme.text_3)
                .child(label),
        )
        .children(actions)
        .into_any_element()
}

/// What measures the element it's drawn over into `cell`, as `measure` reads its bounds, in
/// pixels as designed, and redraws the window when that changed: after this frame, as a view told to draw while one
/// is drawing isn't drawn again.
pub(super) fn measured(
    cell: &Rc<Cell<f32>>,
    app: &AppHandle,
    measure: impl FnOnce(Bounds<Pixels>) -> f32 + 'static,
) -> impl IntoElement {
    let (cell, app) = (cell.clone(), app.clone());
    gpui::canvas(
        move |bounds, window, cx| {
            let drawn = measure(bounds) / Theme::get(cx).scale;
            if cell.replace(drawn) != drawn {
                app.update_later(window, cx, |_, _, cx| cx.notify());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// Says a checkout is stopped partway through Git's `op`, which committing there leaves
/// as it is.
pub(super) fn unfinished_banner(op: &str, theme: &Theme) -> Div {
    side_header(theme).child(super::resolve::warn_banner(
        format!("{op} isn't finished"),
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

/// A group's heading in a side list, sitting on the rows under it.
pub(super) fn list_heading(text: impl IntoElement, theme: &Theme) -> AnyElement {
    section_label(text, theme)
        .w_full()
        .h(dp(ROW_HEIGHT))
        .flex()
        .items_end()
        .px(dp(14.))
        .pb(dp(4.))
        .into_any_element()
}

/// The page for a selection that's gone, as `title` says.
pub(super) fn not_found(title: &'static str, theme: &Theme, cx: &App) -> AnyElement {
    empty_state(Some(Illustration::Papers), title, "", theme, cx)
}

/// What a side list shows in place of its rows, in the room they'd take.
pub(super) fn list_empty(
    drawing: Illustration,
    title: String,
    detail: impl Into<SharedString>,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .child(empty_state(Some(drawing), title, detail, theme, cx))
        .into_any_element()
}

/// What a side list shows when its filters keep none of its items: that none match the
/// filter's `query`, or with none typed, that the other filters hide every one. `one` and
/// `several` name its items, as "branch" and "branches".
pub(super) fn list_filtered_out(
    query: &str,
    (one, several): (&str, &str),
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let miss = match query.trim() {
        "" => format!("The filters hide every {one}"),
        query => format!("No {several} match \"{query}\""),
    };
    list_empty(Illustration::Papers, miss, "", theme, cx)
}

/// The item `delta` steps on from the one at `at` in `items`, or the first when none is;
/// none past either end, but that end for a step longer than the list, as gg and G take.
pub(super) fn step_from<T>(items: &[T], at: Option<usize>, delta: isize) -> Option<&T> {
    match delta.unsigned_abs() > items.len() {
        true if delta < 0 => items.first(),
        true => items.last(),
        false => items.get(at.map_or(Some(0), |at| at.checked_add_signed(delta))?),
    }
}

/// For `on_mouse_down_out` of a card on a list page that takes the focus with `focus`, as the
/// People card does: a click anywhere else gives the focus back to the page's list.
pub(super) fn back_to_list(
    focus: &FocusHandle,
    cx: &Context<Detail>,
) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let focus = focus.clone();
    cx.listener(move |this, _, window, cx| {
        if focus.is_focused(window) {
            window.focus(&this.side_focus, cx);
        }
    })
}

impl Detail {
    /// A list page's column, whose list takes the focus: ↑ and ↓ `step` through it, gg and G
    /// to either end, ⇧→ and ⇧← `fold` what's selected, on a page whose rows fold, and Return
    /// does `confirm` to it. Where rows don't fold, ⇧→ and ⇧← do nothing, not fold the diff.
    pub(super) fn list_column(
        &self,
        step: fn(&mut Self, isize, &mut Context<Self>),
        fold: Option<fn(&mut Self, bool, &mut Context<Self>)>,
        confirm: Option<fn(&mut Self, &mut Window, &mut Context<Self>)>,
        cx: &Context<Self>,
    ) -> Div {
        v_flex()
            .size_full()
            .track_focus(&self.side_focus)
            .key_context("List")
            .on_action(cx.listener(move |this, _: &SelectNext, _, cx| step(this, 1, cx)))
            .on_action(cx.listener(move |this, _: &SelectPrevious, _, cx| step(this, -1, cx)))
            .on_action(
                cx.listener(move |this, _: &SelectFirst, _, cx| step(this, isize::MIN / 2, cx)),
            )
            .on_action(
                cx.listener(move |this, _: &SelectLast, _, cx| step(this, isize::MAX / 2, cx)),
            )
            .on_action(cx.listener(move |this, _: &Expand, _, cx| {
                if let Some(fold) = fold {
                    fold(this, true, cx)
                }
            }))
            .on_action(cx.listener(move |this, _: &Collapse, _, cx| {
                if let Some(fold) = fold {
                    fold(this, false, cx)
                }
            }))
            .when_some(confirm, |el, confirm| {
                el.on_action(
                    cx.listener(move |this, _: &Confirm, window, cx| confirm(this, window, cx)),
                )
            })
    }

    /// A People card, under a list, with a focus of its own: a click in it takes the focus,
    /// and a click anywhere else gives it back to the list. Its `people` are everyone, most
    /// first, each with what picks them, their name, email and how many: it lists the busiest
    /// few and the one `picked`, then how many more, which a click lists too, scrolling within
    /// the card, until Show fewer. `counted` says what they count and `empty` stands in for
    /// none. A click on someone, ↑ and ↓ while the card has the focus, or Clear (with `None`)
    /// hands `pick` who to filter the list to; ⇧→ and ⇧← list everyone and fold them again.
    pub(super) fn people_card(
        &self,
        people: Vec<(String, String, String, usize)>,
        picked: Option<&str>,
        (counted, empty): (String, &'static str),
        pick: impl Fn(&mut Self, Option<String>, &mut Context<Self>) + Clone + 'static,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let solid = self.people_focus.is_focused(window);
        let most = people.first().map_or(1, |(.., n)| (*n).max(1));
        let keys: Vec<String> = people.iter().map(|(key, ..)| key.clone()).collect();
        // Clear shows while someone's picked, listed or not.
        let anyone_picked = picked.is_some();
        let picked = (keys.iter()).position(|key| Some(key.as_str()) == picked);
        let rows: Vec<_> = (people.into_iter().enumerate())
            .map(|(ix, (key, name, email, n))| {
                let pick = pick.clone();
                let on = (Some(ix) == picked, solid);
                person_row(("person", ix), (&name, &email), (n, most), on, theme)
                    .on_click(cx.listener(move |this, _, _, cx| pick(this, Some(key.clone()), cx)))
            })
            .collect();
        // The arrows step through everyone from the one picked, rather than the list over
        // the card.
        let step = {
            let pick = pick.clone();
            move |this: &mut Self, by: isize, cx: &mut Context<Self>| {
                let last = keys.len().saturating_sub(1);
                let to = picked.map_or(0, |ix| ix.saturating_add_signed(by).min(last));
                if let Some(key) = keys.get(to) {
                    // Folded, one past the busiest few shows right after them.
                    let row = match this.people_open {
                        true => to,
                        false => to.min(PEOPLE_SHOWN),
                    };
                    this.people_scroll.scroll_to_item(row);
                    pick(this, Some(key.clone()), cx);
                }
            }
        };
        let fold_to = |this: &mut Self, open: bool, cx: &mut Context<Self>| {
            this.people_open = open;
            cx.notify();
        };
        let everyone = rows.len();
        let open = self.people_open && everyone > PEOPLE_SHOWN;
        let rows: Vec<_> = (rows.into_iter().enumerate())
            .filter(|(ix, _)| open || *ix < PEOPLE_SHOWN || Some(*ix) == picked)
            .map(|(_, row)| row)
            .collect();
        let rest = everyone - rows.len();
        let open_height =
            (window.viewport_size().height * 0.3).clamp(theme.scaled(190.), theme.scaled(380.));
        let clear = anyone_picked.then(|| {
            clear_button("people-clear")
                .on_click(cx.listener(move |this, _, _, cx| pick(this, None, cx)))
        });
        // Its glyph under the pictures, its words under the names, as the navigator's are.
        let fold = |open: bool| {
            let (id, glyph) = match open {
                true => ("people-fewer", Icon::ChevronUp),
                false => ("people-more", Icon::Kebab),
            };
            fold_row(id, glyph, 18., 42., theme).on_click(cx.listener(move |this, _, _, cx| {
                this.people_open = !open;
                this.people_scroll.set_offset(Point::default());
                cx.notify();
            }))
        };
        // It gives way to the list over it in a short window, down to its heading, a row
        // and its fold, its rows scrolling within it; one or two people need no more room.
        v_flex()
            .when(everyone > 2, |el| el.min_h(dp(120.)))
            .m(dp(12.))
            .py(dp(8.))
            .gap(dp(2.))
            .border_1()
            .border_color(theme.border_3)
            .rounded(dp(8.))
            .track_focus(&self.people_focus)
            .on_action(cx.listener({
                let step = step.clone();
                move |this, _: &SelectNext, _, cx| step(this, 1, cx)
            }))
            .on_action(cx.listener(move |this, _: &SelectPrevious, _, cx| step(this, -1, cx)))
            .on_action(cx.listener(move |this, _: &Expand, _, cx| fold_to(this, true, cx)))
            .on_action(cx.listener(move |this, _: &Collapse, _, cx| fold_to(this, false, cx)))
            .on_mouse_down_out(back_to_list(&self.people_focus, cx))
            // As tall with Clear as without, so picking someone moves no row.
            .child(
                h_flex()
                    .flex_none()
                    .h(dp(22.))
                    .px(dp(14.))
                    .gap(dp(8.))
                    .child(section_label("People", theme).flex_grow(1.))
                    .child(faint(counted, theme))
                    .children(clear),
            )
            .when(everyone == 0, |el| {
                el.child(faint(empty, theme).px(dp(14.)).py(dp(4.)))
            })
            // Open, a third of the window tall or so, leaving the list over it room, the rest
            // a scroll away; it sits at the bottom of its column, so its fold stays under the
            // pointer as it opens and closes.
            .child(
                v_flex()
                    .relative()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("people-rows")
                            .min_h_0()
                            .gap(dp(2.))
                            .when(open, |el| el.max_h(open_height))
                            .overflow_y_scroll()
                            .track_scroll(&self.people_scroll)
                            .children(rows),
                    )
                    .children(scrollbar(
                        "people-scrollbar",
                        Scroll::Div(self.people_scroll.clone()),
                        theme,
                    )),
            )
            .when(rest > 0, |el| {
                el.child(fold(false).flex_none().child(format!("{rest} more")))
            })
            .when(open, |el| {
                el.child(fold(true).flex_none().child("Show fewer"))
            })
            .into_any_element()
    }
}

/// An overview's title, beside a list until something's opened: what it shows, and over what.
pub(super) fn overview_title(title: &'static str, scope: String, theme: &Theme) -> Div {
    h_flex()
        .px(dp(14.))
        .pb(dp(4.))
        .gap(dp(8.))
        // In a narrow pane the scope gives way first, then the title is cut short.
        .child(
            div()
                .flex_grow(1.)
                .min_w_0()
                .truncate()
                .text_size(dp(15.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(faint(scope, theme).flex_shrink(1000.).min_w_0().truncate())
}

/// An overview's headline numbers, a tile each: what it counts, the number, and more of what
/// it counts on hover.
pub(super) fn overview_tiles(
    tiles: impl IntoIterator<Item = (&'static str, String, &'static str)>,
    theme: &Theme,
) -> Div {
    let unkeyed = tiles
        .into_iter()
        .map(|(label, n, tip)| (label, n, tip, None));
    keyed_tiles(unkeyed, theme)
}

/// What an overview's things are split into, as a bar of each part's share over what each is
/// and how many: `parts` as their glyph, its colour, the bar's colour and what they are,
/// with how many. Parts with none are left out, and with none at all there's no bar.
pub(super) fn overview_split(
    parts: impl IntoIterator<Item = (Icon, gpui::Hsla, gpui::Hsla, String, usize)>,
    theme: &Theme,
) -> Option<AnyElement> {
    let split: Vec<_> = (parts.into_iter()).filter(|(.., n)| *n > 0).collect();
    if split.is_empty() {
        return None;
    }
    Some(
        v_flex()
            .px(dp(6.))
            .pt(dp(8.))
            .gap(dp(6.))
            .child(
                h_flex()
                    .h(dp(6.))
                    .gap(dp(2.))
                    .children(split.iter().map(|(.., fill, _, n)| {
                        div()
                            .h_full()
                            .flex_grow(*n as f32)
                            .flex_basis(Pixels::ZERO)
                            .rounded(dp(2.))
                            .bg(*fill)
                    })),
            )
            .child(
                h_flex()
                    .gap(dp(12.))
                    .flex_wrap()
                    .children(split.into_iter().map(|(glyph, color, _, said, n)| {
                        h_flex()
                            .gap(dp(4.))
                            .child(small_icon(glyph).text_color(color))
                            .child(faint(format!("{n} {said}"), theme))
                    })),
            )
            .into_any_element(),
    )
}

/// [`overview_tiles`] where some are keys to a chart under them: a chip of the colour they're
/// drawn in before their label.
pub(super) fn keyed_tiles(
    tiles: impl IntoIterator<Item = (&'static str, String, &'static str, Option<gpui::Hsla>)>,
    theme: &Theme,
) -> Div {
    // Two to a row where four would cut their labels short.
    h_flex()
        .flex_wrap()
        .gap(dp(8.))
        .px(dp(6.))
        .children(
            tiles
                .into_iter()
                .enumerate()
                .map(|(ix, (label, n, tip, key))| {
                    v_flex()
                        .id(("overview-tile", ix))
                        .flex_grow(1.)
                        .flex_basis(dp(120.))
                        .min_w_0()
                        .px(dp(12.))
                        .py(dp(10.))
                        .gap(dp(2.))
                        .border_1()
                        .border_color(theme.border_3)
                        .rounded(dp(8.))
                        .tooltip(Tooltip::text(tip))
                        .child(
                            h_flex()
                                .gap(dp(6.))
                                .children(key.map(|key| {
                                    div().flex_none().size(dp(7.)).rounded(dp(1.5)).bg(key)
                                }))
                                .child(faint(label, theme).min_w_0().truncate()),
                        )
                        .child(
                            div()
                                .text_size(dp(22.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .whitespace_nowrap()
                                .child(n),
                        )
                }),
        )
}

/// How wide an overview row's columns on the right are, so they line up from one section to
/// the next: a count and what it counts, then a short time or a bare number.
pub(super) const OVERVIEW_COUNT_WIDTH: f32 = 110.;
pub(super) const OVERVIEW_TIME_WIDTH: f32 = 40.;

/// One of an overview row's columns on the right: `text`, faint and flush right, `width` wide.
pub(super) fn overview_column(text: String, width: f32, theme: &Theme) -> Div {
    faint(text, theme).flex_none().w(dp(width)).text_right()
}

/// A row of an overview's counted list, as the files committed most or the areas landed in:
/// its `label`, a bar of its `n` against the `most` any has in `bar`'s box, then `n`. Picked,
/// it's a filter on, its selection `solid` while its list has the focus.
pub(super) fn share_row(
    id: impl Into<gpui::ElementId>,
    label: impl IntoElement,
    bar: Div,
    (n, most): (usize, usize),
    (picked, solid): (bool, bool),
    theme: &Theme,
) -> Stateful<Div> {
    let on = picked && solid;
    let (track, fill) = match on {
        true => (theme.text_inverted.opacity(0.25), theme.muted_on_solid()),
        false => (theme.border_3, theme.pop.opacity(0.7)),
    };
    let share = n as f32 / most.max(1) as f32;
    side_row(id, picked, solid, theme)
        .child(label)
        .child(
            bar.h(dp(6.)).rounded(dp(3.)).bg(track).child(
                div()
                    .h_full()
                    .rounded(dp(3.))
                    .w(gpui::relative(share))
                    .bg(fill),
            ),
        )
        .child(quiet(
            overview_column(n.to_string(), OVERVIEW_TIME_WIDTH, theme),
            on,
            theme,
        ))
}

/// The Clear that lets go of what's picked off an overview.
pub(super) fn clear_button(id: &'static str) -> Button {
    button(id, "Clear").small().style(ButtonStyle::Ghost)
}

/// `n` with its thousands set apart, as 1,434 reads more easily than 1434.
pub(super) fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// An overview's sections, side by side where the page has room for each to be its `basis`
/// wide, else stacked, so a wide window shows more of them at once.
pub(super) fn overview_sections(
    sections: impl IntoIterator<Item = (Option<AnyElement>, f32)>,
) -> Div {
    h_flex()
        .flex_wrap()
        .items_start()
        .gap_x(dp(12.))
        .gap_y(dp(6.))
        .children(sections.into_iter().filter_map(|(section, basis)| {
            Some(
                div()
                    .flex_grow(1.)
                    .flex_basis(dp(basis))
                    .min_w_0()
                    .child(section?),
            )
        }))
}

/// A section of an overview's heading: `title`, and what its numbers count. Each is as tall as
/// one with a Clear, so sections side by side start their rows level.
pub(super) fn overview_heading(
    title: &'static str,
    detail: impl IntoElement,
    theme: &Theme,
) -> Div {
    h_flex()
        .h(dp(28.))
        .mt(dp(14.))
        .px(dp(14.))
        .pb(dp(2.))
        .gap(dp(8.))
        .child(section_label(title, theme).flex_grow(1.))
        .child(detail)
}

/// Under a list cut short, how many it left out, when it left any.
pub(super) fn and_more(rest: usize, theme: &Theme) -> Option<Div> {
    (rest > 0).then(|| {
        faint(format!("and {rest} more"), theme)
            .px(dp(14.))
            .py(dp(4.))
    })
}

/// How many commits are conflicted, when any are.
fn conflicted_note(n: usize, theme: &Theme) -> Option<Div> {
    (n > 0).then(|| {
        div()
            .text_color(theme.warn)
            .child(format!("{n} conflicted"))
    })
}

/// How tall a side list stays however much else its column holds, as a short window's
/// would otherwise give it all to the cards under it.
pub(super) const LIST_MIN_HEIGHT: f32 = ROW_HEIGHT * 4.;

/// How many people a People card lists, the most active first, before N more.
const PEOPLE_SHOWN: usize = 5;

/// A row of a People card: who, and a bar of their `n` against the `most` anyone has. Picked,
/// it's a filter on, its selection `solid` while its card has the focus.
fn person_row(
    id: impl Into<gpui::ElementId>,
    (name, email): (&str, &str),
    (n, most): (usize, usize),
    (picked, solid): (bool, bool),
    theme: &Theme,
) -> Stateful<Div> {
    let on = picked && solid;
    let (track, bar) = match on {
        true => (theme.text_inverted.opacity(0.25), theme.muted_on_solid()),
        false => (theme.border_3, theme.text_3),
    };
    side_row(id, picked, solid, theme)
        .h_auto()
        .py(dp(5.))
        .child(author_avatar(name, email, 20., theme))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(dp(3.))
                .child(
                    h_flex()
                        .gap(dp(8.))
                        .child(div().flex_1().min_w_0().truncate().child(name.to_owned()))
                        .child(quiet(faint(n.to_string(), theme), on, theme)),
                )
                .child(
                    div().h(dp(4.)).rounded(dp(2.)).bg(track).child(
                        div()
                            .h_full()
                            .rounded(dp(2.))
                            .w(gpui::relative(n as f32 / most.max(1) as f32))
                            .bg(bar),
                    ),
                ),
        )
}
