//! A checkout's overview, its page's Now: what's going on in it and what just happened
//! there, as the project's page has what shipped and Branches what's cooking. It shows beside
//! the checkout's files and commit box until a file is picked, whose diff takes its place.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Div, ElementId, MouseButton, ScrollHandle, div, prelude::*, relative,
};

use super::Detail;
use super::diff_view::files_toggle;
use super::panes::{
    and_more, clear_button, meta_item, meta_row, overview_column, overview_heading, page_title,
    share_row, title_text,
};
use super::slots::SlotPick;
use crate::backend::{
    self, Branch, Checkout, Commit, CommitId, CommitState, FileChange, FullName, ReviewState,
    Workspace,
};
use crate::state::{Activity, ProjectId, QUARTER, SeenCommit, Selection, Store};
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::file_icons::file_icon;
use crate::ui::review::state_glyph;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::{
    ButtonStyle, Icon, ROW_HEIGHT, Tooltip, button, commit_title, dot, dp, faint, glow_once,
    h_flex, keep_focus_on_press, list_row, relative_time, short_path, shortcut, small_icon, v_flex,
};

mod activity;
mod inactive;

/// How many quarters of an hour the activity covers: the last 12 hours.
const BARS: usize = 48;
/// How wide the commits' times are, room for "59m" or "11mo" and no more, so they sit close to
/// the branch.
const COMMITS_TIME_WIDTH: f32 = 28.;
/// How many hot files are listed, the rest of a sweeping commit's files told as N more.
const HOT_SHOWN: usize = 50;

/// What the overview reads beyond the workspace, for the checkout it was read for.
#[derive(Default)]
pub(super) struct Now {
    /// The project and checkout it's read for.
    checkout: Option<(ProjectId, Checkout)>,
    /// What moved a linked worktree's `HEAD` lately other than its commits, newest first, and
    /// the branch and commit it was read at, as the reflog changes only when they do.
    events: Vec<(i64, String)>,
    events_at: Option<(Option<FullName>, Option<CommitId>)>,
    /// The commits of the checkout's branches when it was opened or last loaded, to tell one
    /// an agent just made from those already there.
    known: Option<HashSet<CommitId>>,
    /// The hot file picked, whose commits are all Commits lists.
    file: Option<Arc<str>>,
    /// The files each commit changes, read for those the overview can list as the checkout
    /// loads, and the commits being read. Commits don't change, so what's read stays, with
    /// another checkout picked too.
    paths: HashMap<CommitId, Vec<String>>,
    reading: HashSet<CommitId>,
    /// The quarters of an hour picked off the activity, by when they begin over `QUARTER`,
    /// which the rest of the overview is narrowed to; none for all it shows.
    span: SlotPick,
    /// Hot files' and the commits', each scrolling on its own.
    hot_scroll: ScrollHandle,
    commits_scroll: ScrollHandle,
}

fn checkout_commits<'a>(ws: &'a Workspace, checkout: &Checkout) -> Vec<&'a Commit> {
    (ws.branches_in(checkout))
        .flat_map(|branch| &branch.commits)
        .collect()
}

/// A commit the overview counts: on one of the checkout's branches, or taken by a branch
/// that left it since, as merged or unapplied, as last seen there.
#[derive(Clone, Copy)]
enum Made<'a> {
    Here(&'a Branch, &'a Commit),
    Gone(&'a SeenCommit),
}

impl<'a> Made<'a> {
    fn id(self) -> CommitId {
        match self {
            Made::Here(_, commit) => commit.id,
            Made::Gone(commit) => commit.id,
        }
    }

    fn time(self) -> i64 {
        match self {
            Made::Here(_, commit) => commit.time,
            Made::Gone(commit) => commit.time,
        }
    }

    fn state(self) -> CommitState {
        match self {
            Made::Here(_, commit) => commit.state,
            Made::Gone(commit) => commit.state,
        }
    }

    fn title(self) -> &'a str {
        match self {
            Made::Here(_, commit) => &commit.title,
            Made::Gone(commit) => &commit.title,
        }
    }
}

/// What the overview of `checkout` in `ws` counts: its branches' commits, and those its
/// `activity` remembers of branches gone since, newest first.
fn made<'a>(
    ws: &'a Workspace,
    checkout: &Checkout,
    activity: Option<&'a Activity>,
) -> Vec<Made<'a>> {
    let branches: Vec<_> = ws.branches_in(checkout).collect();
    let gone = activity.map_or(Vec::new(), |a| a.gone(&branches));
    let mut made: Vec<Made> = (branches.iter())
        .flat_map(|&branch| branch.commits.iter().map(move |c| Made::Here(branch, c)))
        .chain(gone.into_iter().map(Made::Gone))
        .collect();
    made.sort_by_key(|commit| std::cmp::Reverse(commit.time()));
    made
}

/// A stretch of time said short: "<1m", "45m", "3h 05m", "2d".
fn span_said(seconds: i64) -> String {
    match seconds.max(0) / 60 {
        0 => "<1m".to_owned(),
        minutes @ ..60 => format!("{minutes}m"),
        minutes @ ..1440 => format!("{}h {:02}m", minutes / 60, minutes % 60),
        minutes => format!("{}d", minutes / 1440),
    }
}

impl Detail {
    /// Whether the selected checkout's page shows its overview beside its files rather than
    /// their diff: until a file is picked, as the project's page shows what shipped until a
    /// commit is, and always with no changes, an empty commit's box included. Not while
    /// committing changes, which picks them in the diff, nor editing a commit.
    pub fn showing_now(&self, cx: &App) -> bool {
        let Some((project, worktree)) = self.selected_checkout(cx) else {
            return false;
        };
        if self.editing(&project, cx).is_some() {
            return false;
        }
        let none = (self.store.read(cx).workspace(&project))
            .and_then(|ws| ws.uncommitted_in(&worktree))
            .is_none_or(<[_]>::is_empty);
        none || (self.now_open && !self.composing)
    }

    /// Go from a checkout's changes back to its overview, as Esc and its crumb do, unless
    /// committing. Says whether it did.
    pub fn close_changes(&mut self, cx: &mut Context<Self>) -> bool {
        let back = self.selected_checkout(cx).is_some() && !self.composing && !self.showing_now(cx);
        if back {
            self.now_open = true;
            // Lines picked in the diff go with it, rather than stay for keys to act on unseen.
            self.line_picks = None;
            cx.notify();
        }
        back
    }

    /// Read what the selected checkout's overview needs beyond the workspace: a linked
    /// worktree's moves lately, and which files its commits change. `opened` as it's picked.
    pub(super) fn load_now(&mut self, opened: bool, cx: &mut Context<Self>) {
        let Some(checkout) = self.selected_checkout(cx) else {
            return;
        };
        // With its changes gone, as committed, the page settles on the overview, so changes
        // showing up again don't bring a diff over it.
        let none = (self.store.read(cx).workspace(&checkout.0))
            .and_then(|ws| ws.uncommitted_in(&checkout.1))
            .is_some_and(<[_]>::is_empty);
        if none && !self.composing {
            self.now_open = true;
        }
        if self.now.checkout.as_ref() != Some(&checkout) {
            self.now = Now {
                checkout: Some(checkout.clone()),
                paths: std::mem::take(&mut self.now.paths),
                ..Now::default()
            };
        }
        // What's there already, so a commit made from now on stands out, and not one made
        // while another page was picked.
        if opened {
            self.now.known = (self.store.read(cx).workspace(&checkout.0)).map(|ws| {
                (checkout_commits(ws, &checkout.1).iter())
                    .map(|commit| commit.id)
                    .collect()
            });
        }
        let Some(path) = self.store.read(cx).path(&checkout.0) else {
            return;
        };
        let linked = (checkout.1.linked()).and_then(|name| {
            let ws = self.store.read(cx).workspace(&checkout.0)?;
            Some((ws.find_worktree(name)?, ws.worktree_lane(name)))
        });
        let head = linked.map(|(w, lane)| {
            let top = lane.and_then(|lane| lane.branches.first());
            let tip = top.and_then(|b| b.commits.first());
            (w.branch.clone(), tip.map(|commit| commit.id))
        });
        if let Checkout::Linked(name) = checkout.1.clone()
            && head != self.now.events_at
        {
            self.now.events_at = head.clone();
            let since = window_start(crate::ui::now());
            let (read_from, read_for) = (path.clone(), checkout.clone());
            let events =
                backend::run(move || backend::worktrees::events(&read_from, name.as_ref(), since));
            cx.spawn(async move |this, cx| {
                let events = events.await;
                this.update(cx, |this, cx| {
                    // Not over what a read for where `HEAD` moved on to found.
                    if this.now.checkout.as_ref() != Some(&read_for) || this.now.events_at != head {
                        return;
                    }
                    match events {
                        Ok(events) => this.now.events = events,
                        // Read again on the next load.
                        Err(err) => {
                            this.now.events_at = None;
                            tracing::warn!("couldn't read the worktree's moves: {err:#}")
                        }
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.read_commit_paths(&checkout, &path, cx);
    }

    /// Follow the file being read in a checkout's changes into the commit an agent just made
    /// of it, as the workspace loads again, with the file still under the cursor. Only when
    /// that's the one commit made, as of several it can't tell which took the file; not a
    /// commit of the app's own, which is seen, nor while committing or with the overview picked.
    pub(super) fn follow_into_commit(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some((_, worktree)) = self.selected_checkout(cx).filter(|(p, _)| p == project) else {
            return;
        };
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return;
        };
        let commits = checkout_commits(ws, &worktree);
        let ids = commits.iter().map(|commit| commit.id).collect();
        let Some(known) = self.now.known.replace(ids) else {
            return;
        };
        let reading = (self.diff.as_ref())
            .filter(|diff| diff.key.project == *project)
            .is_some_and(|diff| diff.key.checkout().as_ref() == Some(&worktree));
        let Some(path) = self.cursor_path().filter(|_| reading) else {
            return;
        };
        // The tab picked, as with the changes gone the overview shows in any case.
        if self.composing || self.now_open {
            return;
        }
        let still = (ws.uncommitted_in(&worktree))
            .is_some_and(|files| files.iter().any(|file| file.path == path));
        let mut made = (commits.iter())
            .filter(|commit| !known.contains(&commit.id) && store.is_unread(project, **commit));
        let (Some(made), None, false) = (made.next(), made.next(), still) else {
            return;
        };
        // The agent that wrote it, or its author.
        let by = (made.agent.as_ref()).map_or(&made.author, |agent| &agent.name);
        let (id, by) = (made.id, by.to_owned());
        self.pending_file = Some(path);
        let at = Selection::Commit(project.clone(), id);
        self.store.update(cx, |store, cx| {
            store.select(Some(at), cx);
            store.snack(format!("{by} committed what you were reading"), cx);
        });
    }

    /// A checkout's header: its name, the branch it has checked out or how many are applied,
    /// and its folder.
    pub(super) fn render_checkout_header(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return div().into_any_element();
        };
        let ws = entry.workspace.as_ref();
        let (title, branch, folder, target) = match checkout {
            Checkout::Linked(name) => {
                let linked = ws.and_then(|ws| ws.find_worktree(name.as_ref()));
                (
                    name.to_string(),
                    (linked.and_then(|w| w.branch.as_ref()))
                        .map_or("No branch checked out".to_owned(), |b| {
                            backend::short_ref(b).into_owned()
                        }),
                    linked.map(|w| short_path(&w.path)),
                    Selection::Worktree(project.clone(), name.clone()),
                )
            }
            Checkout::Main => (
                "Main worktree".to_owned(),
                match ws {
                    // Checked out on its own, as in Git, the branch is named.
                    Some(ws) if !ws.managed => (ws.branches_in(&Checkout::Main).next())
                        .map(|b| b.name.clone())
                        .or(ws
                            .target_branch
                            .as_ref()
                            .map(|t| backend::short_ref(t).into_owned()))
                        .unwrap_or_else(|| "No branch checked out".to_owned()),
                    ws => match ws.map_or(0, |ws| ws.branches_in(&Checkout::Main).count()) {
                        1 => "1 branch applied".to_owned(),
                        n => format!("{n} branches applied"),
                    },
                },
                Some(short_path(&entry.project.path)),
                Selection::Uncommitted(project.clone()),
            ),
        };
        let rests_on = ws.zip(checkout.linked()).and_then(|(ws, name)| {
            let (branch, from) = ws.beneath(ws.worktree_lane(name)?).next()?.top()?;
            let at = from.map_or(String::new(), |id| format!(" at {}", id.short()));
            Some(format!("On {}{at}", store.name_of(project, branch)))
        });
        let meta = meta_row(theme)
            .child(meta_item(Icon::Branch, theme).child(branch))
            .children(rests_on.map(|on| meta_item(Icon::Stack, theme).child(on)))
            .children(folder.map(|folder| {
                meta_item(Icon::Folder, theme)
                    .min_w_0()
                    // Cut in the middle, keeping the folder's own name, as the project's page does.
                    .child(
                        (div().min_w_0().overflow_hidden().whitespace_nowrap())
                            .text_ellipsis_middle()
                            .child(folder),
                    )
            }));
        let title = page_title(Icon::Folder, theme).child(title_text(title));
        self.page_header(title, meta, target, None, None, theme, cx)
    }

    /// The overview of `project`'s `checkout`, laid out as the project's and Branches' are: where
    /// it is (whether it's being written, its uncommitted files and unread commits), then what
    /// happened over the last 12 hours or the quarters picked off its activity (how busy it's been,
    /// the files committed most and the commits, side by side, so a file picked filters the commits
    /// beside it).
    pub(super) fn render_now(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            // A target several local branches track is picked here, as on the project's page.
            return match (store.project(project)).and_then(|entry| entry.target_choice.as_ref()) {
                Some(choice) => self.target_chooser(project, choice, theme, cx),
                None => div().flex_1().into_any_element(),
            };
        };
        let now = crate::ui::now();
        // Everything under the activity is of the 12 hours it covers, or the quarters picked,
        // the commits of branches that left since too.
        let span = &self.now.span;
        let start = window_start(now);
        let inside = |at: i64| at >= start && span.keeps(at.div_euclid(QUARTER));
        let activity = store.activity.get(&(project.clone(), checkout.clone()));
        let all = made(ws, checkout, activity);
        let scoped: Vec<Made> = (all.iter().copied())
            .filter(|commit| inside(commit.time()))
            .collect();
        let events = (self.now.events.iter())
            .filter(|(at, _)| inside(*at))
            .collect::<Vec<_>>();
        let files = ws.uncommitted_in(checkout).unwrap_or_default();
        // Of all its commits, not only those listed, so marking them read leaves it caught up.
        let unread: Vec<CommitId> = (all.iter())
            .filter_map(|c| match c {
                Made::Here(_, commit) if store.is_unread(project, *commit) => Some(commit.id),
                _ => None,
            })
            .collect();
        let status = self.now_status(project, checkout, files, &all, unread.len(), theme, cx);
        // Side by side where both fit, as the tiles wrap.
        let columns = [
            (self.now_hot_files(&scoped, theme, cx), 2., 240.),
            (
                self.now_commits(project, &scoped, &events, unread, theme, cx),
                3.,
                340.,
            ),
        ];
        // The page holds still: hot files and the commits scroll within what's left under the
        // activity. Over it, the bar a file's diff has, its toggle for the file list where the
        // diff's is, so opening a file moves it not.
        v_flex()
            .id("now")
            .flex_1()
            .h_full()
            .min_w_0()
            .child(self.now_bar(files, theme, cx))
            .child(
                v_flex()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .px(dp(14.))
                    .py(dp(16.))
                    .gap(dp(6.))
                    .child(self.now_activity((activity, now), &all, &scoped, status, theme, cx))
                    // Stacked, each half of what's left, where they don't fit side by side.
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h(dp(ROW_HEIGHT * 6.))
                            .flex_wrap()
                            .items_stretch()
                            .content_stretch()
                            .children(columns.into_iter().filter_map(|(column, grow, basis)| {
                                Some(column?.flex_grow(grow).flex_basis(dp(basis)).min_w_0())
                            })),
                    ),
            )
            .into_any_element()
    }

    /// The bar over the overview, as over a file's diff: the file list's toggle, counting the
    /// `files` and their lines while the list is hidden, which shows the overview alone.
    fn now_bar(&self, files: &[FileChange], theme: &Theme, cx: &Context<Self>) -> Div {
        let (added, removed) = (files.iter().filter_map(|file| file.lines))
            .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr));
        let hidden = self.files_hidden;
        let (glyph, tip) = match hidden {
            true => (Icon::SidebarNarrow, "Show the files"),
            false => (Icon::FilesSidebar, "Hide the files"),
        };
        let counts = (hidden && !files.is_empty()).then_some((files.len(), added, removed));
        h_flex()
            .flex_none()
            .h(dp(40.))
            .px(dp(12.))
            .bg(theme.bg_1)
            .child(
                files_toggle("now-files", glyph, counts, theme)
                    .tooltip(Tooltip::keyed(tip, shortcut(&super::ToggleFiles, cx)))
                    .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_files(window, cx))),
            )
    }

    /// Where `checkout` is, in a line: whether files are being written or how long it's
    /// been quiet, how many `files` are uncommitted and how many of its `commits` are `unread`,
    /// or All caught up.
    #[expect(clippy::too_many_arguments)]
    fn now_status(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        files: &[FileChange],
        commits: &[Made],
        unread: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let store = self.store.read(cx);
        let last_write = store.last_write(project, checkout);
        let now = crate::ui::now();
        let newest = commits.first().map(|commit| commit.time());
        let writing = store.writing(project, checkout);
        let state = match (writing, newest) {
            (true, _) => "Being written now".to_owned(),
            (false, Some(last)) => match now - last_write.max(Some(last)).unwrap_or(last) {
                ..60 => "Quiet".to_owned(),
                quiet => format!("Quiet for {}", span_said(quiet)),
            },
            (false, None) => "Nothing committed yet".to_owned(),
        };
        // One line, short enough for a narrow page, its last words cut short before they'd
        // widen it.
        meta_row(theme)
            .min_w_0()
            .overflow_hidden()
            .px(dp(14.))
            .child(
                h_flex()
                    .flex_none()
                    .gap(dp(6.))
                    // Lit while an agent writes.
                    .child(div().size(dp(7.)).rounded_full().bg(match writing {
                        true => theme.pop,
                        false => theme.text_3,
                    }))
                    .child(state),
            )
            .when(!files.is_empty(), |el| {
                el.child(
                    meta_item(Icon::File, theme)
                        .flex_none()
                        .child(format!("{} uncommitted", files.len())),
                )
            })
            .when(unread > 0, |el| {
                let said = div().min_w_0().truncate().child(format!("{unread} unread"));
                el.child(meta_item(Icon::Commit, theme).min_w_0().child(said))
            })
            .when(files.is_empty() && unread == 0, |el| {
                let said = div().min_w_0().truncate().child("All caught up");
                el.child(meta_item(Icon::Tick, theme).min_w_0().child(said))
            })
    }

    /// The files most of `commits` changed, once read. Clicking one lists only the commits
    /// that changed it under Commits, as an area filters what shipped; Clear lets go of it.
    fn now_hot_files(&self, commits: &[Made], theme: &Theme, cx: &Context<Self>) -> Option<Div> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for paths in commits.iter().filter_map(|c| self.now.paths.get(&c.id())) {
            for path in paths {
                *counts.entry(path).or_default() += 1;
            }
        }
        let mut hot: Vec<(Arc<str>, usize)> = (counts.into_iter())
            .map(|(path, n)| (path.into(), n))
            .collect();
        hot.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let picked = self.now.file.as_ref();
        // A sweeping commit's thousand files would be a thousand rows drawn every frame.
        let rest = hot.len().saturating_sub(HOT_SHOWN);
        hot.truncate(HOT_SHOWN);
        // The one picked stays, with its Clear, once no commit listed changed it.
        if let Some(picked) = picked.filter(|p| !hot.iter().any(|(path, _)| path == *p)) {
            hot.push((picked.clone(), 0));
        }
        let most = hot.first()?.1;
        let clear = picked.map(|_| {
            clear_button("now-hot-clear")
                .on_click(cx.listener(|this, _, _, cx| this.pick_hot_file(None, cx)))
        });
        let rows = (hot.iter().enumerate()).map(|(ix, (path, n))| {
            let on = picked == Some(path);
            let label = file_label(path, on, theme, cx);
            let path = path.clone();
            let bar = div().flex_none().w(dp(48.));
            share_row(("now-hot", ix), label, bar, (*n, most), (on, true), theme)
                // In full, as a long name can leave its folder no room.
                .tooltip(Tooltip::text(path.to_string()))
                .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.pick_hot_file(Some(path.clone()), cx)),
                )
                .into_any_element()
        });
        let more = and_more(rest, theme).map(IntoElement::into_any_element);
        Some(
            v_flex()
                .child(overview_heading("Hot files", div().children(clear), theme))
                .child(scrolling(
                    ("now-hot-rows", "now-hot-scrollbar"),
                    &self.now.hot_scroll,
                    rows.chain(more),
                    theme,
                )),
        )
    }

    /// On the page a click on a commit of a branch that left a checkout picked, open what took
    /// it: on the project's page the commit that landed it, on the Branches page its branch.
    /// It waits for the page to read its list, as it's picked before it has, or reads it
    /// again: once that's `read`, it opens or not, the history read further back first, and
    /// another page picked lets go of it.
    pub(super) fn open_waiting(&mut self, read: bool, cx: &mut Context<Self>) {
        let Some((page, seen)) = self.opening.take() else {
            return;
        };
        if self.selection(cx).as_ref() != Some(&page) {
            return;
        }
        let done = match &page {
            // A landing past the pages read is looked for further back, as far as the commit.
            Selection::Project(project) => {
                self.open_landing(project, seen.pull, seen.id, cx)
                    || (read && !self.read_back_to(project, seen.time, cx))
            }
            Selection::Branches(project) => {
                let seen_branch = (seen.branch.as_deref()).and_then(|b| FullName::try_from(b).ok());
                let listed = (seen_branch.as_ref())
                    .and_then(|branch| self.listed_branch(project, branch))
                    .is_some();
                if listed {
                    self.select_other_branch(seen_branch, cx);
                }
                listed || read
            }
            _ => true,
        };
        if !done {
            self.opening = Some((page, seen));
        }
    }

    /// List only the commits that changed `file` under Commits, or all with `None`.
    fn pick_hot_file(&mut self, file: Option<Arc<str>>, cx: &mut Context<Self>) {
        self.now.file = file.filter(|file| self.now.file.as_ref() != Some(file));
        self.now.commits_scroll.set_offset(Default::default());
        cx.notify();
    }

    /// Read which files `checkout`'s commits change, those of the hours the activity shows,
    /// unless read already or being read, for Hot files and its filter.
    fn read_commit_paths(
        &mut self,
        checkout: &(ProjectId, Checkout),
        path: &Path,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(&checkout.0) else {
            return;
        };
        let now = crate::ui::now();
        let since = window_start(now);
        let known =
            |id: &CommitId| self.now.paths.contains_key(id) || self.now.reading.contains(id);
        let made = made(ws, &checkout.1, store.activity.get(checkout));
        let unread: Vec<CommitId> = (made.into_iter())
            .filter(|commit| commit.time() >= since && !known(&commit.id()))
            .map(Made::id)
            .collect();
        if unread.is_empty() {
            return;
        }
        self.now.reading.extend(unread.iter().copied());
        let (path, asked) = (path.to_owned(), unread.clone());
        let paths = backend::run(move || backend::commit_paths(&path, &unread));
        cx.spawn(async move |this, cx| {
            let paths = paths.await;
            this.update(cx, |this, cx| {
                // Kept with another checkout picked meanwhile, as commits don't change; read
                // again on the next load when this failed.
                for id in &asked {
                    this.now.reading.remove(id);
                }
                match paths {
                    Ok(paths) => this.now.paths.extend(paths),
                    Err(err) => tracing::warn!("couldn't read which files commits change: {err:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `commits`, and, in a linked worktree, the `events` that moved its `HEAD` then, newest
    /// first, with a hot file picked only the commits that changed it. A commit opens on a
    /// click, and one whose branch left the checkout opens the page that has it now.
    fn now_commits(
        &self,
        project: &ProjectId,
        commits: &[Made],
        events: &[&(i64, String)],
        unread: Vec<CommitId>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        enum Item<'a> {
            Commit(Made<'a>),
            Moved(&'a str),
        }
        // With a hot file picked, the commits that changed it, once that's read; nothing
        // else that moved `HEAD` changed a file.
        let file = self.now.file.as_ref();
        let changed = |commit: &&Made| {
            file.is_none_or(|file| {
                (self.now.paths.get(&commit.id()))
                    .is_some_and(|paths| paths.iter().any(|path| **path == **file))
            })
        };
        let listed: Vec<Made> = commits.iter().filter(changed).copied().collect();
        let events = events.iter().filter(|_| file.is_none());
        let mut items: Vec<(i64, Item)> = (listed.iter().map(|c| (c.time(), Item::Commit(*c))))
            .chain(events.map(|(at, said)| (*at, Item::Moved(said))))
            .collect();
        // Kept with none listed while any is unread, for its Mark all as read.
        let none = items.is_empty();
        if none && file.is_none() && unread.is_empty() {
            return None;
        }
        items.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        let store = self.store.read(cx);
        let rows = (items.into_iter().enumerate()).map(|(ix, (at, item))| {
            let row = list_row(("now-commit", ix), false, theme);
            match item {
                Item::Commit(made) => {
                    // On one of the checkout's branches, it opens, read or not as the
                    // navigator shows it: quieter once read, in semibold with a dot while not,
                    // and marked conflicted as there. Gone with its branch, it shows as last
                    // seen, and opens what took it: the commit that landed it on the project's
                    // page, else its branch on the Branches page.
                    let ((glyph, color), said, unread, target) = match made {
                        Made::Here(branch, commit) => (
                            match commit.conflicted {
                                true => (Icon::Warning, theme.warn),
                                false => (Icon::Commit, theme.blue),
                            },
                            branch_said(store, project, branch, theme),
                            store.is_unread(project, commit),
                            Selection::Commit(project.clone(), commit.id),
                        ),
                        Made::Gone(seen) => (
                            (Icon::Commit, theme.text_3),
                            left_said(seen, theme),
                            false,
                            match seen.state {
                                CommitState::Integrated => Selection::Project(project.clone()),
                                CommitState::Local | CommitState::Pushed => {
                                    Selection::Branches(project.clone())
                                }
                            },
                        ),
                    };
                    let opening = match made {
                        Made::Gone(seen) => Some((target.clone(), seen.clone())),
                        Made::Here(..) => None,
                    };
                    // Under the row's place in the list, so its own id goes by the commit.
                    let glow = matches!(made, Made::Here(_, commit) if store.glows(commit.id))
                        .then(|| ElementId::Name(format!("glow-{}", made.id().0).into()));
                    // The pull request that takes it there, else the branch it's on, its
                    // beginning faint beside the glyph and all of it on hover.
                    let ((branch_glyph, branch_color), label, tip) = said;
                    let branch = h_flex()
                        .id(("now-commit-branch", ix))
                        .flex_none()
                        .max_w(relative(0.22))
                        .gap(dp(4.))
                        .child(
                            small_icon(branch_glyph)
                                .flex_none()
                                .text_color(branch_color),
                        )
                        .child(faint(label, theme).min_w_0().truncate())
                        .tooltip(Tooltip::text(tip));
                    row.when_some(glow, |el, glow| el.relative().child(glow_once(glow, theme)))
                        .child(small_icon(glyph).text_color(color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .when(!unread, |el| el.text_color(theme.text_2))
                                .when(unread, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                                .child(commit_title(made.title())),
                        )
                        .when(unread, |el| el.child(dot(theme.pop)))
                        .child(branch)
                        .child(overview_column(
                            relative_time(at),
                            COMMITS_TIME_WIDTH,
                            theme,
                        ))
                        .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.opening = opening.clone();
                            let target = target.clone();
                            this.store
                                .update(cx, |store, cx| store.select(Some(target), cx))
                        }))
                        .into_any_element()
                }
                Item::Moved(said) => row
                    .text_color(theme.text_2)
                    .child(small_icon(Icon::Branch).text_color(theme.text_3))
                    .child(div().flex_1().min_w_0().truncate().child(said.to_owned()))
                    .child(overview_column(
                        relative_time(at),
                        COMMITS_TIME_WIDTH,
                        theme,
                    ))
                    .into_any_element(),
            }
        });
        let reading = file.is_some()
            && commits
                .iter()
                .any(|c| !self.now.paths.contains_key(&c.id()));
        let said = match file {
            Some(_) if listed.is_empty() => Some(match reading {
                true => "Reading which commits changed it…",
                false => "No commit then changed it",
            }),
            None if none => Some("No commits then"),
            _ => None,
        };
        // Where Hot files has its Clear, while any is unread, and quieter than the unread
        // titles under it.
        let mark_read = (!unread.is_empty()).then(|| {
            let project = project.clone();
            button("now-mark-read", "Mark all as read")
                .small()
                .style(ButtonStyle::Ghost)
                .tint(theme.text_2)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let ids = unread.clone();
                    (this.store).update(cx, |store, cx| store.mark_read(&project, ids, true, cx))
                }))
        });
        Some(
            v_flex()
                .child(overview_heading(
                    "Commits",
                    div().children(mark_read),
                    theme,
                ))
                .children(said.map(|said| faint(said, theme).px(dp(14.)).py(dp(4.))))
                .child(scrolling(
                    ("now-commits-rows", "now-commits-scrollbar"),
                    &self.now.commits_scroll,
                    rows,
                    theme,
                )),
        )
    }
}

/// When the first of the quarters of an hour the activity shows, up to the one `now` is in,
/// begins: what the overview counts when none is picked.
fn window_start(now: i64) -> i64 {
    now - now.rem_euclid(QUARTER) - QUARTER * (BARS as i64 - 1)
}

/// `rows` in what's left of their part's height, scrolling within it with a bar of their own;
/// `ids` the rows' and the bar's.
fn scrolling(
    ids: (&'static str, &'static str),
    scroll: &ScrollHandle,
    rows: impl IntoIterator<Item = impl IntoElement>,
    theme: &Theme,
) -> Div {
    // Laid over its part rather than in it, so however many rows there are, they never make
    // the part taller than the page leaves it.
    v_flex()
        .relative()
        .flex_1()
        .min_h_0()
        .child(
            v_flex()
                .id(ids.0)
                .absolute()
                .size_full()
                .overflow_y_scroll()
                .track_scroll(scroll)
                .children(rows),
        )
        .children(scrollbar(ids.1, Scroll::Div(scroll.clone()), theme))
}

/// A file's icon and name, then its folder fainter and cut from the start, as its nearest
/// folders tell apart the `page.tsx` files a web app has in every route, but quieter, as an
/// overview lists files beside what they belong to; `on` a solid selection, as the row's text is.
fn file_label(path: &str, on: bool, theme: &Theme, cx: &App) -> Div {
    let (name, dir) = diff::split_path(path);
    let muted = match on {
        true => theme.muted_on_solid(),
        false => theme.text_3,
    };
    h_flex()
        .flex_1()
        .min_w_0()
        .gap(dp(8.))
        .child(file_icon(path, cx).flex_none())
        // The folder gives way first, then the name, in a narrow column.
        .child(
            (div().min_w_0().overflow_hidden().whitespace_nowrap())
                .text_ellipsis_middle()
                .when(!on, |el| el.text_color(theme.text_2))
                .child(name.to_owned()),
        )
        .when(!dir.is_empty(), |el| {
            el.child(
                div()
                    .flex_shrink(100.)
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_start()
                    .text_color(muted)
                    .child(dir.to_owned()),
            )
        })
}

/// What names the branch `commit` left the checkout on: its name, with the glyph of a merged
/// pull request for one that landed, and where it is now for a hover.
fn left_said(commit: &SeenCommit, theme: &Theme) -> ((Icon, gpui::Hsla), String, String) {
    let name = (commit.branch.as_deref())
        .map_or("Unnamed branch", backend::short_text)
        .to_owned();
    match commit.state {
        CommitState::Integrated => (
            state_glyph(ReviewState::Merged, theme),
            name.clone(),
            format!("{name} · landed, on the project's page"),
        ),
        CommitState::Local | CommitState::Pushed => (
            (Icon::Branch, theme.text_3),
            name.clone(),
            format!("{name} · left this worktree, on the Branches page"),
        ),
    }
}

/// What names `branch`: the pull request that takes it to the target, else its name, with
/// the glyph that says which, and both in full for a hover.
fn branch_said(
    store: &Store,
    project: &ProjectId,
    branch: &Branch,
    theme: &Theme,
) -> ((Icon, gpui::Hsla), String, String) {
    let name = store.name_of(project, branch).to_owned();
    match &branch.review {
        Some(review) if review.title.is_empty() => (
            state_glyph(review.state, theme),
            name.clone(),
            format!("{name} · #{}", review.number),
        ),
        Some(review) => (
            state_glyph(review.state, theme),
            review.title.clone(),
            format!("{name} · #{} {}", review.number, review.title),
        ),
        None => ((Icon::Branch, theme.text_3), name.clone(), name),
    }
}
