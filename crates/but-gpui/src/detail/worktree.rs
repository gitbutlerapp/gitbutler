//! A checkout's overview, its page's Now: what's going on in it and what just happened
//! there, as the project's page has what shipped and Branches what's cooking. It shows beside
//! the checkout's files and commit box until a file is picked, whose diff takes its place.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Div, MouseButton, ScrollHandle, SharedString, Stateful, div,
    prelude::*, relative,
};

use super::Detail;
use super::panes::{
    OVERVIEW_TIME_WIDTH, grouped, meta_item, meta_row, overview_column, overview_heading,
    overview_tiles, overview_title, page_title, title_text,
};
use super::slots::SlotPick;
use crate::backend::{self, Branch, Commit, CommitId, Workspace};
use crate::state::{ProjectId, QUARTER, Selection, Store};
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::file_icons::file_icon;
use crate::ui::review::state_glyph;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::{
    ButtonStyle, Icon, ROW_HEIGHT, Tooltip, button, commit_title, count_badge, dp, faint,
    fold_glyph, fold_row, h_flex, icon, keep_focus_on_press, list_row, plural, relative_time,
    short_path, shortcut, side_row, small_icon, v_flex,
};

mod activity;

use activity::quarters_said;

/// How many quarters of an hour the activity covers: the last 12 hours.
const BARS: usize = 48;
/// How far a branch's files sit in under it: past its chevron, under its glyph.
const STEP_IN: f32 = 20.;
/// How many files the commits not seen yet changed are listed until "N more".
const UNSEEN_FILES_SHOWN: usize = 3;
/// How wide the commits' times are, room for "59m" or "11mo" and no more, so they sit close to
/// the branch.
const COMMITS_TIME_WIDTH: f32 = 28.;
/// A checkout written to this lately is being worked in, as the navigator's "active" says.
const WRITING_WITHIN: i64 = 120;

/// What the overview reads beyond the workspace, for the checkout it was read for.
#[derive(Default)]
pub(super) struct Now {
    /// The project, and the linked worktree or `None` for the main checkout, it's read for.
    checkout: Option<(ProjectId, Option<Arc<str>>)>,
    /// What moved a linked worktree's `HEAD` lately other than its commits, newest first, and
    /// the branch and commit it was read at, as the reflog changes only when they do.
    events: Vec<(i64, String)>,
    events_at: Option<(Option<String>, Option<CommitId>)>,
    /// The files each branch's commits not seen yet change, with their lines, by those
    /// commits: `None` while it's read.
    unseen_lines: HashMap<Arc<[CommitId]>, Option<FileLines>>,
    /// The commits of the checkout's branches when it last loaded, to tell one an agent just
    /// made from those already there.
    known: Option<HashSet<CommitId>>,
    /// The hot file picked, whose commits are all Commits lists.
    file: Option<Arc<str>>,
    /// The files each commit changes, read for those the overview can list as the checkout
    /// loads, and the commits being read. Commits don't change, so what's read stays.
    paths: HashMap<CommitId, Vec<String>>,
    reading: HashSet<CommitId>,
    /// The quarters of an hour picked off the activity, by when they begin over `QUARTER`,
    /// which the rest of the overview is narrowed to; none for all it shows.
    span: SlotPick,
    /// The sections listing all they have rather than their first few, by their fold's id.
    unfolded: HashSet<SharedString>,
    /// The branches whose commits not seen yet are folded to their heading, by name.
    folded: HashSet<SharedString>,
    /// New's rows, hot files' and the commits', each scrolling within its part of the page.
    new_scroll: ScrollHandle,
    hot_scroll: ScrollHandle,
    commits_scroll: ScrollHandle,
}

/// Files a range of commits changes, each with the lines it adds and removes.
type FileLines = Vec<(String, u32, u32)>;

/// The newest and oldest of a run of commits, one after the other in a lane.
type Run = (CommitId, CommitId);

/// The commits of a checkout's branches, as listed: linked worktree `worktree`'s, or the main
/// checkout's.
fn checkout_commits<'a>(ws: &'a Workspace, worktree: Option<&str>) -> Vec<&'a Commit> {
    let branches: Vec<&Branch> = match worktree {
        Some(name) => (ws.find_worktree(name)).map_or(Vec::new(), |w| w.branches.iter().collect()),
        None => ws.branches().collect(),
    };
    branches
        .into_iter()
        .flat_map(|branch| &branch.commits)
        .collect()
}

/// Who a commit is by, as "Who worked here" counts them: the agent that wrote it, or its
/// author.
fn who(commit: &Commit) -> &str {
    commit
        .agent
        .as_ref()
        .map_or(&commit.author, |agent| &agent.name)
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
            .and_then(|ws| ws.uncommitted_in(worktree.as_deref()))
            .is_none_or(<[_]>::is_empty);
        none || (self.now_open && !self.composing)
    }

    /// Go from a checkout's changes back to its overview, as Esc and its crumb do, unless
    /// committing. Says whether it did.
    pub fn close_changes(&mut self, cx: &mut Context<Self>) -> bool {
        let back = self.selected_checkout(cx).is_some() && !self.composing && !self.showing_now(cx);
        if back {
            self.now_open = true;
            cx.notify();
        }
        back
    }

    /// The selected checkout's commits not seen yet, as the navigator lists them, by the
    /// branch they're on, the branch with the newest first.
    fn unseen_by_branch<'a>(
        &self,
        project: &ProjectId,
        worktree: Option<&str>,
        cx: &'a App,
    ) -> Vec<(&'a Branch, Vec<&'a Commit>)> {
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return Vec::new();
        };
        let mut groups: Vec<(&Branch, Vec<&Commit>)> = Vec::new();
        for commit in checkout_commits(ws, worktree) {
            let Some((branch, _)) = ws.find_commit(commit.id) else {
                continue;
            };
            if !store.is_unread(project, commit) {
                continue;
            }
            match groups.iter_mut().find(|(b, _)| std::ptr::eq(*b, branch)) {
                Some((_, commits)) => commits.push(commit),
                None => groups.push((branch, vec![commit])),
            }
        }
        groups.sort_by_key(|(_, commits)| {
            std::cmp::Reverse(commits.iter().map(|commit| commit.time).max())
        });
        groups
    }

    /// Read what the selected checkout's overview needs beyond the workspace: a linked
    /// worktree's moves lately, and the lines the commits not seen yet change.
    pub(super) fn load_now(&mut self, cx: &mut Context<Self>) {
        let Some(checkout) = self.selected_checkout(cx) else {
            return;
        };
        // With its changes gone, as committed, the page settles on the overview, so changes
        // showing up again don't bring a diff over it.
        let none = (self.store.read(cx).workspace(&checkout.0))
            .and_then(|ws| ws.uncommitted_in(checkout.1.as_deref()))
            .is_some_and(<[_]>::is_empty);
        if none && !self.composing {
            self.now_open = true;
        }
        if self.now.checkout.as_ref() != Some(&checkout) {
            // What's there already, so a commit made from now on stands out.
            let known = (self.store.read(cx).workspace(&checkout.0)).map(|ws| {
                (checkout_commits(ws, checkout.1.as_deref()).iter())
                    .map(|commit| commit.id)
                    .collect()
            });
            self.now = Now {
                checkout: Some(checkout.clone()),
                known,
                ..Now::default()
            };
        }
        let Some(path) = self.store.read(cx).path(&checkout.0) else {
            return;
        };
        let linked = (checkout.1.as_deref()).and_then(|name| {
            self.store
                .read(cx)
                .workspace(&checkout.0)?
                .find_worktree(name)
        });
        let head = linked.map(|w| {
            let tip = w.branches.first().and_then(|b| b.commits.first());
            (w.branch.clone(), tip.map(|commit| commit.id))
        });
        if let Some(name) = checkout.1.clone().filter(|_| head != self.now.events_at) {
            self.now.events_at = head;
            let since = window_start(crate::ui::now());
            let (read_from, read_for) = (path.clone(), checkout.clone());
            let events = backend::run(move || backend::worktrees::events(&read_from, &name, since));
            cx.spawn(async move |this, cx| {
                let events = events.await;
                this.update(cx, |this, cx| {
                    if this.now.checkout.as_ref() != Some(&read_for) {
                        return;
                    }
                    match events {
                        Ok(events) => this.now.events = events,
                        Err(err) => tracing::warn!("couldn't read the worktree's moves: {err:#}"),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.read_commit_paths(&checkout, &path, cx);
        // Each branch's, as one range when they're one run, else each commit's own added up,
        // as a range would count the commits between them too.
        let sets: Vec<(Arc<[CommitId]>, Option<Run>)> =
            match self.store.read(cx).workspace(&checkout.0) {
                Some(ws) => (self.unseen_by_branch(&checkout.0, checkout.1.as_deref(), cx))
                    .iter()
                    .map(|(_, commits)| {
                        let ids = commits.iter().map(|commit| commit.id).collect();
                        (ids, unseen_run(ws, commits))
                    })
                    .collect(),
                None => Vec::new(),
            };
        (self.now.unseen_lines).retain(|ids, _| sets.iter().any(|(set, _)| set == ids));
        for (ids, run) in sets {
            if self.now.unseen_lines.contains_key(&ids) {
                continue;
            }
            self.now.unseen_lines.insert(ids.clone(), None);
            let (path, read_for, key) = (path.clone(), checkout.clone(), ids.clone());
            let lines = backend::run(move || match run {
                Some((newest, oldest)) => backend::range_lines(&path, oldest, newest),
                None => backend::commits_lines(&path, &ids),
            });
            cx.spawn(async move |this, cx| {
                let lines = lines.await;
                this.update(cx, |this, cx| {
                    if this.now.checkout.as_ref() != Some(&read_for) {
                        return;
                    }
                    // Read again on the next load when this failed.
                    match lines {
                        Ok(lines) => drop(this.now.unseen_lines.insert(key, Some(lines))),
                        Err(err) => {
                            this.now.unseen_lines.remove(&key);
                            tracing::warn!("couldn't count the unseen commits' lines: {err:#}")
                        }
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Follow the file being read in a checkout's changes into the commit an agent just made
    /// of it, as the workspace loads again, with the file still under the cursor. Not a commit
    /// of the app's own, which is seen, nor while committing or with the overview picked.
    pub(super) fn follow_into_commit(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some((_, worktree)) = self.selected_checkout(cx).filter(|(p, _)| p == project) else {
            return;
        };
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return;
        };
        let commits = checkout_commits(ws, worktree.as_deref());
        let ids = commits.iter().map(|commit| commit.id).collect();
        let Some(known) = self.now.known.replace(ids) else {
            return;
        };
        let reading = (self.diff.as_ref())
            .filter(|diff| diff.key.project == *project)
            .is_some_and(|diff| diff.key.checkout() == Some(worktree.as_deref()));
        let Some(path) = self.cursor_path().filter(|_| reading) else {
            return;
        };
        // The tab picked, as with the changes gone the overview shows in any case.
        if self.composing || self.now_open {
            return;
        }
        let still = (ws.uncommitted_in(worktree.as_deref()))
            .is_some_and(|files| files.iter().any(|file| file.path == path));
        let made = (commits.iter())
            .filter(|commit| !known.contains(&commit.id) && store.is_unread(project, **commit))
            .max_by_key(|commit| commit.time);
        let Some(made) = made.filter(|_| !still) else {
            return;
        };
        let (id, by) = (made.id, who(made).to_owned());
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
        worktree: Option<&Arc<str>>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return div().into_any_element();
        };
        let ws = entry.workspace.as_ref();
        let (title, branch, folder, target) = match worktree {
            Some(name) => {
                let linked = ws.and_then(|ws| ws.find_worktree(name));
                (
                    name.to_string(),
                    (linked.and_then(|w| w.branch.as_deref()))
                        .map_or("No branch checked out".to_owned(), |b| {
                            backend::short_ref(b).to_owned()
                        }),
                    linked.map(|w| short_path(&w.path)),
                    Selection::Worktree(project.clone(), name.clone()),
                )
            }
            None => (
                "Main worktree".to_owned(),
                match ws.map_or(0, |ws| ws.branches().count()) {
                    1 => "1 branch applied".to_owned(),
                    n => format!("{n} branches applied"),
                },
                Some(short_path(&entry.project.path)),
                Selection::Uncommitted(project.clone()),
            ),
        };
        let meta = meta_row(theme)
            .child(meta_item(Icon::Branch, theme).child(branch))
            .children(folder.map(|folder| {
                meta_item(Icon::Folder, theme)
                    .min_w_0()
                    .child(div().min_w_0().truncate().child(folder))
            }));
        let main = v_flex()
            .gap(dp(10.))
            .child(page_title(Icon::Folder, theme).child(title_text(title)))
            .child(meta);
        self.page_header(main, target, None, None, theme, cx)
    }

    /// The overview of the checkout of `project`, linked worktree `worktree` or the main one,
    /// in two parts, laid out as the project's and Branches' are: New, what you haven't
    /// looked at (its uncommitted files, whether they're being written, and the commits not
    /// seen yet), then Activity, what happened over the last 12 hours or the quarters picked
    /// off it (how busy it's been, the files committed most and the commits, side by side, so
    /// a file picked filters the commits beside it).
    pub(super) fn render_now(
        &self,
        project: &ProjectId,
        worktree: Option<&Arc<str>>,
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
        let name = worktree.map(|w| &**w);
        let now = crate::ui::now();
        let mut commits = checkout_commits(ws, name);
        commits.sort_by_key(|commit| std::cmp::Reverse(commit.time));
        // Everything under the activity is of the 12 hours it covers, or the quarters picked.
        let span = &self.now.span;
        let range = quarters_said(&span.slots, now);
        let start = window_start(now);
        let inside = |at: i64| at >= start && span.keeps(at.div_euclid(QUARTER));
        let scoped: Vec<&Commit> = (commits.iter().copied())
            .filter(|commit| inside(commit.time))
            .collect();
        let activity = store.activity.get(&(project.clone(), worktree.cloned()));
        let busy: HashSet<i64> = (activity.into_iter())
            .flat_map(|a| a.quarters.iter().map(|(at, _)| *at))
            .chain(scoped.iter().map(|commit| commit.time))
            .filter(|at| inside(*at))
            .map(|at| at.div_euclid(QUARTER))
            .collect();
        // Once every commit's files are read, so the count doesn't climb as they come in.
        let changed: Option<HashSet<&String>> = (scoped.iter())
            .map(|commit| self.now.paths.get(&commit.id))
            .collect::<Option<Vec<_>>>()
            .map(|paths| paths.into_iter().flatten().collect());
        let tiles = [
            (
                "Commits",
                grouped(scoped.len()),
                "Commits made here in the time the activity shows or the quarters picked",
            ),
            (
                "Files changed",
                changed.map_or("–".to_owned(), |files| grouped(files.len())),
                "Files those commits changed",
            ),
            (
                "Busy for",
                match busy.len() {
                    0 => "–".to_owned(),
                    n => span_said(n as i64 * QUARTER),
                },
                "Quarters of an hour a file was written or a commit made in",
            ),
        ];
        // Side by side where both fit, as the tiles wrap.
        let columns = [
            (self.now_hot_files(&scoped, theme, cx), 2., 240.),
            (self.now_commits(project, &scoped, now, theme, cx), 3., 340.),
        ];
        // The page holds still: New scrolls within the most of it it takes, and hot files and
        // the commits within what's left under the activity. Over it, the bar a file's diff
        // has, its toggle for the file list where the diff's is, so opening a file moves it not.
        v_flex()
            .id("now")
            .flex_1()
            .h_full()
            .min_w_0()
            .child(self.now_bar(ws.uncommitted_in(name).unwrap_or_default(), theme, cx))
            .child(
                v_flex()
                    .w_full()
                    .max_w(dp(820.))
                    .flex_1()
                    .min_h_0()
                    .px(dp(14.))
                    .py(dp(16.))
                    .gap(dp(6.))
                    .child(overview_title(
                        "New",
                        "since you last looked".to_owned(),
                        theme,
                    ))
                    .child(self.now_new(project, worktree, &commits, theme, cx))
                    .child(
                        overview_title(
                            "Activity",
                            range.clone().unwrap_or("last 12 hours".to_owned()),
                            theme,
                        )
                        .mt(dp(22.)),
                    )
                    .child(self.now_activity(activity, &commits, now, theme, cx))
                    .child(overview_tiles(tiles, theme).mt(dp(6.)))
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
    fn now_bar(&self, files: &[backend::FileChange], theme: &Theme, cx: &Context<Self>) -> Div {
        let (added, removed) = (files.iter().filter_map(|file| file.lines))
            .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr));
        let hidden = self.files_hidden;
        let (glyph, tip) = match hidden {
            true => (Icon::SidebarNarrow, "Show the files"),
            false => (Icon::FilesSidebar, "Hide the files"),
        };
        h_flex()
            .flex_none()
            .h(dp(40.))
            .px(dp(12.))
            .bg(theme.bg_1)
            .child(
                h_flex()
                    .id("now-files")
                    .flex_none()
                    .h(dp(28.))
                    .px(dp(6.))
                    .gap(dp(6.))
                    .rounded(dp(4.))
                    .hover(|el| el.bg(theme.hover()))
                    .child(icon(glyph).text_color(theme.text_2))
                    .when(hidden && !files.is_empty(), |el| {
                        el.child(count_badge(files.len(), theme))
                            .child(diff::stats(added, removed, theme))
                    })
                    .tooltip(Tooltip::keyed(tip, shortcut(&super::ToggleFiles, cx)))
                    .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_files(window, cx))),
            )
    }

    /// What you haven't looked at, as rows: first the checkout's uncommitted files, with
    /// whether they're being written or how long it's been quiet, then each branch with
    /// commits not opened yet, its files under it. All caught up when there's neither.
    fn now_new(
        &self,
        project: &ProjectId,
        worktree: Option<&Arc<str>>,
        commits: &[&Commit],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let name = worktree.map(|w| &**w);
        let groups = self.unseen_by_branch(project, name, cx);
        let parts = (groups.iter().enumerate()).map(|(ix, (branch, commits))| {
            self.unseen_part(ix, project, branch, commits, theme, cx)
        });
        let store = self.store.read(cx);
        let files = (store.workspace(project))
            .and_then(|ws| ws.uncommitted_in(name))
            .unwrap_or_default();
        let activity = store.activity.get(&(project.clone(), worktree.cloned()));
        let written = (files.iter())
            .filter_map(|file| file.modified.map(|ms| ms / 1000))
            .max();
        let last_write = activity.and_then(|a| a.last).max(written);
        // Written to lately, and since its last commit: once committed, it's quiet again.
        let now = crate::ui::now();
        let newest = commits.first().map(|commit| commit.time);
        let writing = last_write.is_some_and(|at| now - at <= WRITING_WITHIN && newest < Some(at));
        let quiet = match (writing, commits.first()) {
            (true, _) => "being written now".to_owned(),
            (false, Some(last)) => match now - last_write.max(Some(last.time)).unwrap_or(last.time)
            {
                ..60 => "quiet".to_owned(),
                quiet => format!("quiet for {}", span_said(quiet)),
            },
            (false, None) => "nothing committed yet".to_owned(),
        };
        let (glyph, said) = match (files.len(), groups.is_empty()) {
            (0, true) => (
                small_icon(Icon::Tick)
                    .text_color(theme.text_3)
                    .into_any_element(),
                "All caught up".to_owned(),
            ),
            (n, _) => (
                // Lit while an agent writes.
                div()
                    .size(dp(7.))
                    .rounded_full()
                    .bg(match writing {
                        true => theme.pop,
                        false => theme.text_3,
                    })
                    .into_any_element(),
                match n {
                    0 => "Nothing uncommitted".to_owned(),
                    n => plural(n, "uncommitted file"),
                },
            ),
        };
        let status = h_flex()
            .h(dp(ROW_HEIGHT))
            .px(dp(14.))
            .gap(dp(8.))
            .child(
                div()
                    .flex_none()
                    .w(dp(14.))
                    .flex()
                    .justify_center()
                    .child(glyph),
            )
            .child(div().flex_none().child(said))
            .child(faint(quiet, theme).flex_1().min_w_0().truncate())
            .when(!files.is_empty(), |el| {
                // The first of them, as ↓ from the list does.
                el.child(
                    button("now-open-changes", "Open")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.move_file(1, cx))),
                )
            });
        // At most 40% of the page, and in a short one it gives way first, down to its first
        // three rows (where it has them), so the lists under the activity keep room.
        let kept = (1 + groups.len()).min(3) as f32;
        v_flex()
            .relative()
            .flex_shrink(1.)
            .min_h(dp(ROW_HEIGHT * kept))
            .max_h(relative(0.4))
            .child(
                v_flex()
                    .id("now-new")
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.now.new_scroll)
                    .child(status)
                    .children(parts),
            )
            .children(scrollbar(
                "now-new-scrollbar",
                Scroll::Div(self.now.new_scroll.clone()),
                theme,
            ))
    }

    /// A branch's `commits` not opened yet: the branch and who made them, with Open, which
    /// shows them together, or Mark as read, for those not worth a look; then their files as
    /// the file list draws them, with the lines each changes, a click on one opening them at
    /// it.
    fn unseen_part(
        &self,
        ix: usize,
        project: &ProjectId,
        branch: &Branch,
        commits: &[&Commit],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let mut by: Vec<&str> = Vec::new();
        for commit in commits {
            if !by.contains(&who(commit)) {
                by.push(who(commit));
            }
        }
        let ids: Arc<[CommitId]> = commits.iter().map(|commit| commit.id).collect();
        let lines = self.now.unseen_lines.get(&ids).and_then(Option::as_ref);
        let fold: SharedString = format!("now-unseen-more-{}", branch.name).into();
        let name: SharedString = branch.name.clone().into();
        let open = !self.now.folded.contains(&name);
        let file_rows = (lines.into_iter().flatten())
            .take(self.now_shown(&fold, UNSEEN_FILES_SHOWN))
            .enumerate()
            .map(|(row, (path, added, removed))| {
                // Opens them on this file.
                let (project, ids, file) = (project.clone(), ids.clone(), path.clone());
                // A step in, under the branch's glyph.
                list_row(("now-unseen-file", row), false, theme)
                    .pl(dp(STEP_IN + 8.))
                    .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_unseen(&project, &ids, Some(file.clone()), cx)
                    }))
                    .child(file_label(path, false, theme, cx))
                    .child(diff::stats(*added, *removed, theme))
            });
        let hidden = lines.map_or(0, |lines| lines.len().saturating_sub(UNSEEN_FILES_SHOWN));
        let ((glyph, color), label, tip) = branch_said(self.store.read(cx), project, branch, theme);
        let (to_open, read) = (
            (project.clone(), ids.clone()),
            (project.clone(), ids.clone()),
        );
        v_flex()
            .id(("now-unseen", ix))
            .child(
                // Its actions go under it where the line has no room for them.
                h_flex()
                    .flex_wrap()
                    .min_h(dp(ROW_HEIGHT))
                    .px(dp(14.))
                    .py(dp(2.))
                    .gap_x(dp(8.))
                    .gap_y(dp(4.))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(dp(160.))
                            .gap(dp(8.))
                            .child(
                                // A click folds its files away, or unfolds them.
                                h_flex()
                                    .id("branch")
                                    .min_w_0()
                                    .gap(dp(6.))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.now.folded.remove(&name) {
                                            this.now.folded.insert(name.clone());
                                        }
                                        cx.notify();
                                    }))
                                    .child(fold_glyph(Some(open), theme.text_3))
                                    .child(small_icon(glyph).flex_none().text_color(color))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(label),
                                    )
                                    .tooltip(Tooltip::text(tip)),
                            )
                            .child(faint(by.join(", "), theme).flex_1().min_w_0().truncate()),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap(dp(4.))
                            .child(
                                button("open", "Open")
                                    .small()
                                    .style(ButtonStyle::Ghost)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let (project, ids) = &to_open;
                                        this.open_unseen(project, ids, None, cx)
                                    })),
                            )
                            .child(
                                button("read", "Mark as read")
                                    .small()
                                    .style(ButtonStyle::Ghost)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let (project, ids) = read.clone();
                                        this.store.update(cx, |store, cx| {
                                            store.mark_read(&project, ids.iter().copied(), true, cx)
                                        })
                                    })),
                            ),
                    ),
            )
            .when(open, |el| {
                el.children(file_rows).children(self.now_fold(
                    fold,
                    hidden,
                    14. + STEP_IN,
                    theme,
                    cx,
                ))
            })
    }

    /// Open commits `ids` together, as Review does, on `file` once their diff shows.
    fn open_unseen(
        &mut self,
        project: &ProjectId,
        ids: &[CommitId],
        file: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.pending_file = file;
        self.store
            .update(cx, |store, cx| store.review(project, ids, cx))
    }

    /// The files most of `commits` changed, once read. Clicking one lists only the commits
    /// that changed it under Commits, as an area filters what shipped; Clear lets go of it.
    fn now_hot_files(&self, commits: &[&Commit], theme: &Theme, cx: &Context<Self>) -> Option<Div> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for paths in commits.iter().filter_map(|c| self.now.paths.get(&c.id)) {
            for path in paths {
                *counts.entry(path).or_default() += 1;
            }
        }
        let mut hot: Vec<(Arc<str>, usize)> = (counts.into_iter())
            .map(|(path, n)| (path.into(), n))
            .collect();
        hot.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let picked = self.now.file.as_ref();
        // The one picked stays, with its Clear, once no commit listed changed it.
        if let Some(picked) = picked.filter(|p| !hot.iter().any(|(path, _)| path == *p)) {
            hot.push((picked.clone(), 0));
        }
        let most = hot.first()?.1.max(1);
        let clear = picked.map(|_| {
            button("now-hot-clear", "Clear")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.pick_hot_file(None, cx)))
        });
        let rows = (hot.iter().enumerate()).map(|(ix, (path, n))| {
            let on = picked == Some(path);
            let (track, fill) = match on {
                true => (theme.text_inverted.opacity(0.25), theme.muted_on_solid()),
                false => (theme.border_3, theme.pop.opacity(0.7)),
            };
            let path = path.clone();
            side_row(("now-hot", ix), on, true, theme)
                .child(file_label(&path, on, theme, cx))
                .child(bar_on(*n as f32 / most as f32, track, fill))
                .child(
                    div()
                        .flex_none()
                        .w(dp(OVERVIEW_TIME_WIDTH))
                        .text_right()
                        .text_size(dp(11.))
                        .text_color(match on {
                            true => theme.muted_on_solid(),
                            false => theme.text_3,
                        })
                        .child(n.to_string()),
                )
                .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.pick_hot_file(Some(path.clone()), cx)),
                )
        });
        Some(
            v_flex()
                .child(overview_heading("Hot files", div().children(clear), theme).h(dp(28.)))
                .child(scrolling(
                    ("now-hot-rows", "now-hot-scrollbar"),
                    &self.now.hot_scroll,
                    rows,
                    theme,
                )),
        )
    }

    /// List only the commits that changed `file` under Today, or all with `None`.
    fn pick_hot_file(&mut self, file: Option<Arc<str>>, cx: &mut Context<Self>) {
        self.now.file = file.filter(|file| self.now.file.as_ref() != Some(file));
        self.now.commits_scroll.set_offset(Default::default());
        cx.notify();
    }

    /// Read which files `checkout`'s commits change, those of the hours the activity shows,
    /// unless read already or being read, for Hot files and its filter.
    fn read_commit_paths(
        &mut self,
        checkout: &(ProjectId, Option<Arc<str>>),
        path: &Path,
        cx: &mut Context<Self>,
    ) {
        let Some(ws) = self.store.read(cx).workspace(&checkout.0) else {
            return;
        };
        let now = crate::ui::now();
        let since = window_start(now);
        let known =
            |id: &CommitId| self.now.paths.contains_key(id) || self.now.reading.contains(id);
        let unread: Vec<CommitId> = (checkout_commits(ws, checkout.1.as_deref()).into_iter())
            .filter(|commit| commit.time >= since && !known(&commit.id))
            .map(|commit| commit.id)
            .collect();
        if unread.is_empty() {
            return;
        }
        self.now.reading.extend(unread.iter().copied());
        let (path, read_for) = (path.to_owned(), checkout.clone());
        let paths = backend::run(move || backend::commit_paths(&path, &unread));
        cx.spawn(async move |this, cx| {
            let paths = paths.await;
            this.update(cx, |this, cx| {
                if this.now.checkout.as_ref() != Some(&read_for) {
                    return;
                }
                // Read again on the next load when this failed.
                this.now.reading.clear();
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

    /// `commits`, and, in a linked worktree, what else moved its `HEAD` then, newest first,
    /// with a hot file picked only those that changed it. A commit opens on a click.
    fn now_commits(
        &self,
        project: &ProjectId,
        commits: &[&Commit],
        now: i64,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        enum Item<'a> {
            Commit(&'a Commit),
            Moved(&'a str),
        }
        // With a hot file picked, the commits that changed it, once that's read; nothing
        // else that moved `HEAD` changed a file.
        let file = self.now.file.as_ref();
        let changed = |commit: &&&Commit| {
            file.is_none_or(|file| {
                (self.now.paths.get(&commit.id))
                    .is_some_and(|paths| paths.iter().any(|path| **path == **file))
            })
        };
        let listed: Vec<&Commit> = commits.iter().filter(changed).copied().collect();
        let (span, start) = (&self.now.span, window_start(now));
        let events = (self.now.events.iter())
            .filter(|(at, _)| file.is_none() && *at >= start && span.keeps(at.div_euclid(QUARTER)));
        let mut items: Vec<(i64, Item)> = (listed.iter().map(|c| (c.time, Item::Commit(c))))
            .chain(events.map(|(at, said)| (*at, Item::Moved(said))))
            .collect();
        if items.is_empty() && file.is_none() {
            return None;
        }
        items.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        let store = self.store.read(cx);
        let ws = store.workspace(project);
        let rows = (items.into_iter().enumerate()).map(|(ix, (at, item))| {
            let row = list_row(("now-today", ix), false, theme);
            match item {
                Item::Commit(commit) => {
                    let at_commit = Selection::Commit(project.clone(), commit.id);
                    // The pull request that takes it there, else the branch it's on, its
                    // beginning faint beside the glyph and all of it on hover.
                    let branch =
                        (ws.and_then(|ws| ws.find_commit(commit.id))).map(|(branch, _)| {
                            let ((glyph, color), label, tip) =
                                branch_said(store, project, branch, theme);
                            h_flex()
                                .id(("now-today-branch", ix))
                                .flex_none()
                                .max_w(relative(0.22))
                                .gap(dp(4.))
                                .child(small_icon(glyph).flex_none().text_color(color))
                                .child(faint(label, theme).min_w_0().truncate())
                                .tooltip(Tooltip::text(tip))
                        });
                    row.child(small_icon(Icon::Commit).text_color(theme.blue))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(commit_title(&commit.title)),
                        )
                        .children(branch)
                        .child(overview_column(
                            relative_time(at),
                            COMMITS_TIME_WIDTH,
                            theme,
                        ))
                        .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let at = at_commit.clone();
                            this.store
                                .update(cx, |store, cx| store.select(Some(at), cx))
                        }))
                }
                Item::Moved(said) => row
                    .text_color(theme.text_2)
                    .child(small_icon(Icon::Branch).text_color(theme.text_3))
                    .child(div().flex_1().min_w_0().truncate().child(said.to_owned()))
                    .child(overview_column(
                        relative_time(at),
                        COMMITS_TIME_WIDTH,
                        theme,
                    )),
            }
        });
        let reading = file.is_some() && commits.iter().any(|c| !self.now.paths.contains_key(&c.id));
        Some(
            v_flex()
                // As tall as Hot files' beside it, which has room for Clear.
                .child(overview_heading("Commits", div(), theme).h(dp(28.)))
                .when(listed.is_empty() && file.is_some(), |el| {
                    let said = match reading {
                        true => "Reading which commits changed it…",
                        false => "No commit then changed it",
                    };
                    el.child(faint(said, theme).px(dp(14.)).py(dp(4.)))
                })
                .child(scrolling(
                    ("now-commits-rows", "now-commits-scrollbar"),
                    &self.now.commits_scroll,
                    rows,
                    theme,
                )),
        )
    }

    /// How many rows the section `fold` folds lists: its first `first`, or all once unfolded.
    fn now_shown(&self, fold: &str, first: usize) -> usize {
        match self.now.unfolded.contains(fold) {
            true => usize::MAX,
            false => first,
        }
    }

    /// "N more" under a section listing its first few, `hidden` left out, or "Show fewer"
    /// once it lists them all, as the navigator folds a long list: its glyph `at` the rows'
    /// glyphs, its words under their titles.
    fn now_fold(
        &self,
        fold: impl Into<SharedString>,
        hidden: usize,
        at: f32,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Stateful<Div>> {
        let fold = fold.into();
        let (glyph, said) = match (hidden, self.now.unfolded.contains(&fold)) {
            (0, _) => return None,
            (_, true) => (Icon::ChevronUp, "Show fewer".to_owned()),
            (n, false) => (Icon::Kebab, format!("{n} more")),
        };
        let row = fold_row(fold.clone(), glyph, at, at + 20., theme).child(said);
        Some(row.on_click(cx.listener(move |this, _, _, cx| {
            if !this.now.unfolded.remove(&fold) {
                this.now.unfolded.insert(fold.clone());
            }
            cx.notify();
        })))
    }
}

/// When the first of the quarters of an hour the activity shows, up to the one `now` is in,
/// begins: what the overview counts when none is picked.
fn window_start(now: i64) -> i64 {
    now - now.rem_euclid(QUARTER) - QUARTER * (BARS as i64 - 1)
}

/// The newest and oldest of `commits` when they're one run of a lane, as one diff shows them.
fn unseen_run(ws: &Workspace, commits: &[&Commit]) -> Option<Run> {
    let ids: Vec<CommitId> = commits.iter().map(|commit| commit.id).collect();
    match ids[..] {
        [id] => Some((id, id)),
        _ => ws.run_of(&ids),
    }
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

/// A file's icon and name, then its folder fainter and cut in the middle, as the file list
/// draws one, but quieter, as an overview lists files beside what they belong to; `on` a
/// solid selection, as the row's text is.
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
            div()
                .min_w_0()
                .truncate()
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
                    .text_ellipsis_middle()
                    .text_color(muted)
                    .child(dir.to_owned()),
            )
        })
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

/// A bar of `share` of the most, filled with `fill` on `track`, short, leaving the row's
/// name the room.
fn bar_on(share: f32, track: gpui::Hsla, fill: gpui::Hsla) -> Div {
    div()
        .flex_none()
        .w(dp(72.))
        .h(dp(6.))
        .rounded(dp(3.))
        .bg(track)
        .child(div().h_full().rounded(dp(3.)).w(relative(share)).bg(fill))
}
