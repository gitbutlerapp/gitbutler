//! A checkout's overview, its page's Now: what's going on in it and what just happened
//! there, as the project's page has what shipped and Branches what's cooking. It shows beside
//! the checkout's files and commit box until a file is picked, whose diff takes its place.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{AnyElement, App, Context, Div, MouseButton, Stateful, div, prelude::*, relative};

use super::Detail;
use super::panes::{
    OVERVIEW_COUNT_WIDTH, OVERVIEW_TIME_WIDTH, grouped, meta_item, meta_row, overview_column,
    overview_heading, overview_tiles, overview_title, page_title, title_text,
};
use super::slots::SlotPick;
use crate::backend::{self, Branch, Commit, CommitId, Workspace, upstream};
use crate::state::{ProjectId, QUARTER, Selection};
use crate::theme::Theme;
use crate::ui::person::byline_avatar;
use crate::ui::review::state_glyph;
use crate::ui::{
    ButtonStyle, Icon, ROW_HEIGHT, Tooltip, button, commit_title, dp, faint, fold_row, h_flex,
    keep_focus_on_press, list_row, plural, relative_time, short_path, side_row, small_icon, v_flex,
};

mod activity;

use activity::quarters_said;

/// How many quarters of an hour the activity covers: the last 12 hours.
const BARS: usize = 48;
/// How many of the files written most are listed until "N more" lists them all.
const HOT_SHOWN: usize = 3;
/// How many files the commits not seen yet changed are listed until "N more".
const UNSEEN_FILES_SHOWN: usize = 3;
/// How many of those who committed are listed until "N more".
const WHO_SHOWN: usize = 3;
/// How many of today's commits and moves are listed until "N more".
const TODAY_SHOWN: usize = 5;
/// How wide Today's times are, room for "59m" or "11mo" and no more, so they sit close to
/// the pictures.
const TODAY_TIME_WIDTH: f32 = 28.;
/// A checkout written to this lately is being worked in, as the navigator's "active" says.
const WRITING_WITHIN: i64 = 120;

/// What the overview reads beyond the workspace, for the checkout it was read for.
#[derive(Default)]
pub(super) struct Now {
    /// The project, and the linked worktree or `None` for the main checkout, it's read for.
    checkout: Option<(ProjectId, Option<Arc<str>>)>,
    /// What moved a linked worktree's `HEAD` today other than its commits, newest first, and
    /// the branch and commit it was read at, as the reflog changes only when they do.
    events: Vec<(i64, String)>,
    events_at: Option<(Option<String>, Option<CommitId>)>,
    /// The newest and oldest of the commits not seen yet, when they're one run, and the
    /// files they change, with their lines, once read.
    unseen_lines: Option<((CommitId, CommitId), FileLines)>,
    /// The commits of the checkout's branches when it last loaded, to tell one an agent just
    /// made from those already there.
    known: Option<HashSet<CommitId>>,
    /// The hot file picked, whose commits are all Today lists.
    file: Option<Arc<str>>,
    /// The files each commit changes, read for today's once a hot file is picked. Commits
    /// don't change, so what's read stays.
    paths: HashMap<CommitId, Vec<String>>,
    /// The quarters of an hour picked off the activity, by when they begin over `QUARTER`,
    /// which the rest of the overview is narrowed to; none for today.
    span: SlotPick,
    /// The sections listing all they have rather than their first few, by their fold's id.
    unfolded: HashSet<&'static str>,
}

/// Files a range of commits changes, each with the lines it adds and removes.
type FileLines = Vec<(String, u32, u32)>;

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

    /// The selected checkout's commits not seen yet, as the navigator lists them.
    fn unseen<'a>(
        &self,
        project: &ProjectId,
        worktree: Option<&str>,
        cx: &'a App,
    ) -> Vec<&'a Commit> {
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return Vec::new();
        };
        (checkout_commits(ws, worktree).into_iter())
            .filter(|commit| store.is_unread(project, *commit))
            .collect()
    }

    /// Read what the selected checkout's overview needs beyond the workspace: a linked
    /// worktree's moves today, and the lines the commits not seen yet change.
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
            let since = upstream::day_start(upstream::day_of(crate::ui::now()));
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
        let ids: Vec<CommitId> = (self.unseen(&checkout.0, checkout.1.as_deref(), cx).iter())
            .map(|commit| commit.id)
            .collect();
        let run = (self.store.read(cx).workspace(&checkout.0)).and_then(|ws| match ids.len() {
            1 => Some((ids[0], ids[0])),
            _ => ws.run_of(&ids),
        });
        let Some(run) =
            run.filter(|run| self.now.unseen_lines.as_ref().is_none_or(|(r, _)| r != run))
        else {
            return;
        };
        let (newest, oldest) = run;
        let lines = backend::run(move || backend::range_lines(&path, oldest, newest));
        cx.spawn(async move |this, cx| {
            let lines = lines.await;
            this.update(cx, |this, cx| {
                if this.now.checkout.as_ref() != Some(&checkout) {
                    return;
                }
                match lines {
                    Ok(lines) => this.now.unseen_lines = Some((run, lines)),
                    Err(err) => tracing::warn!("couldn't count the unseen commits' lines: {err:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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

    /// The overview of the checkout of `project`, linked worktree `worktree` or the main one:
    /// whether it's being worked in, its changes and the commits not seen yet, how busy it's
    /// been, the files written most, who committed and what happened today.
    pub(super) fn render_now(
        &self,
        project: &ProjectId,
        worktree: Option<&Arc<str>>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let Some(ws) = store.workspace(project) else {
            return div().flex_1().into_any_element();
        };
        let name = worktree.map(|w| &**w);
        let now = crate::ui::now();
        let today = upstream::day_start(upstream::day_of(now));
        let mut commits = checkout_commits(ws, name);
        commits.sort_by_key(|commit| std::cmp::Reverse(commit.time));
        // Everything below the activity is of the quarters picked off it, or of today.
        let span = &self.now.span;
        let range = quarters_said(&span.slots, now);
        let inside = |at: i64| match range {
            Some(_) => span.keeps(at.div_euclid(QUARTER)),
            None => at >= today,
        };
        let scoped: Vec<&Commit> = (commits.iter().copied())
            .filter(|commit| inside(commit.time))
            .collect();
        let files = ws.uncommitted_in(name).unwrap_or_default();
        let activity = store.activity.get(&(project.clone(), worktree.cloned()));
        let written = files
            .iter()
            .filter_map(|file| file.modified.map(|ms| ms / 1000))
            .max();
        let last_write = activity.and_then(|a| a.last).max(written);
        let busy: HashSet<i64> = (activity.into_iter())
            .flat_map(|a| a.quarters.iter().map(|(at, _)| *at))
            .chain(scoped.iter().map(|commit| commit.time))
            .filter(|at| inside(*at))
            .map(|at| at.div_euclid(QUARTER))
            .collect();
        let people: HashSet<&str> = scoped.iter().map(|commit| who(commit)).collect();
        let (hot, hot_said) = match &range {
            Some(range) => (
                activity.map(|a| a.hot_files(inside)),
                format!("writes · {range}"),
            ),
            None => (
                activity.map(|a| a.hot_files(|at| at > now - 3600)),
                "writes · last hour".to_owned(),
            ),
        };
        let tiles = [
            match range {
                Some(_) => ("Commits", grouped(scoped.len()), "Commits made here then"),
                None => (
                    "Commits today",
                    grouped(scoped.len()),
                    "Commits made here since midnight",
                ),
            },
            (
                "Uncommitted",
                match files.len() {
                    0 => "None".to_owned(),
                    n => plural(n, "file"),
                },
                "Files changed here and not committed yet",
            ),
            (
                "Busy for",
                match busy.len() {
                    0 => "–".to_owned(),
                    n => span_said(n as i64 * QUARTER),
                },
                "Quarters of an hour a file was written or a commit made in",
            ),
            (
                "People",
                grouped(people.len()),
                "Who committed here, people and agents",
            ),
        ];
        v_flex()
            .id("now")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(dp(820.))
                    .px(dp(14.))
                    .py(dp(16.))
                    .gap(dp(6.))
                    .child(overview_title("Now", String::new(), theme))
                    .child(self.now_status(&commits, last_write, now, theme))
                    .children(self.now_unseen(project, name, theme, cx))
                    .child(overview_tiles(tiles, theme))
                    .child(self.now_activity(activity, &commits, now, theme, cx))
                    .child({
                        // Side by side where both fit, as the tiles wrap, so Today shows
                        // without scrolling in an ordinary window.
                        let hot = self.now_hot_files(hot.unwrap_or_default(), hot_said, theme, cx);
                        let who = self.now_people(&scoped, range.as_deref(), theme, cx);
                        h_flex().flex_wrap().items_start().children(
                            [hot, who].into_iter().flatten().map(|section| {
                                section.flex_grow(1.).flex_basis(dp(300.)).min_w_0()
                            }),
                        )
                    })
                    .children(self.now_today(project, &scoped, range.as_deref(), theme, cx)),
            )
            .into_any_element()
    }

    /// Whether the checkout is being written to, and since when it's been quiet.
    fn now_status(
        &self,
        commits: &[&Commit],
        last_write: Option<i64>,
        now: i64,
        theme: &Theme,
    ) -> Div {
        // Written to lately, and since its last commit: once committed, it's quiet again.
        let newest = commits.first().map(|commit| commit.time);
        let writing = last_write.filter(|at| now - at <= WRITING_WITHIN && newest < Some(*at));
        let (said, live) = match (writing, commits.first()) {
            (Some(at), _) => (
                format!("Writing files · last change {}", relative_time(at)),
                true,
            ),
            (None, Some(last)) => {
                let quiet = now - last_write.max(Some(last.time)).unwrap_or(last.time);
                let since = match quiet {
                    ..60 => String::new(),
                    _ => format!(" for {}", span_said(quiet)),
                };
                let title = commit_title(&last.title);
                (format!("Quiet{since} · last commit \"{title}\""), false)
            }
            (None, None) => ("Nothing committed here yet".to_owned(), false),
        };
        h_flex()
            .mx(dp(6.))
            .mb(dp(4.))
            .px(dp(12.))
            .py(dp(8.))
            .gap(dp(8.))
            .rounded(dp(8.))
            .border_1()
            .border_color(theme.border_3)
            .child(div().size(dp(7.)).rounded_full().bg(match live {
                true => theme.pop,
                false => theme.text_3,
            }))
            .child(div().flex_1().min_w_0().truncate().child(said))
    }

    /// The commits made here that haven't been looked at, by whom, the files they change,
    /// and Review, which opens them together, or Mark as read.
    fn now_unseen(
        &self,
        project: &ProjectId,
        worktree: Option<&str>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let unseen = self.unseen(project, worktree, cx);
        let newest = unseen.first()?;
        let n = unseen.len();
        let mut by: Vec<&str> = Vec::new();
        for commit in &unseen {
            if !by.contains(&who(commit)) {
                by.push(who(commit));
            }
        }
        let ids: Arc<[CommitId]> = unseen.iter().map(|commit| commit.id).collect();
        let run = (self.now.unseen_lines.as_ref())
            .filter(|((top, _), _)| *top == newest.id)
            .map(|(_, lines)| lines);
        let (added, removed) = (run.into_iter().flatten())
            .fold((0, 0), |(a, r), (_, added, removed)| {
                (a + added, r + removed)
            });
        let most = (run.into_iter().flatten())
            .map(|(_, added, removed)| added + removed)
            .max()
            .unwrap_or(1)
            .max(1);
        let files = run.map_or(0, Vec::len);
        let file_rows = (run
            .into_iter()
            .flatten()
            .take(self.now_shown("now-unseen-more", UNSEEN_FILES_SHOWN))
            .enumerate())
        .map(|(ix, (path, added, removed))| {
            // Opens them on this file.
            let (project, ids, file) = (project.clone(), ids.clone(), path.clone());
            list_row(("now-unseen-file", ix), false, theme)
                .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.review_unseen(&project, &ids, Some(file.clone()), cx)
                }))
                .child(div().flex_1().min_w_0().truncate().child(path.clone()))
                .child(bar(
                    (added + removed) as f32 / most as f32,
                    theme.pop.opacity(0.7),
                    theme,
                ))
                .child(overview_column(
                    format!("+{added} −{removed}"),
                    OVERVIEW_TIME_WIDTH + 40.,
                    theme,
                ))
        });
        let (review, read) = (
            (project.clone(), ids.clone()),
            (project.clone(), ids.clone()),
        );
        let label = match n {
            1 => "Review it".to_owned(),
            n => format!("Review the {n} commits"),
        };
        Some(
            v_flex()
                .mx(dp(6.))
                .mb(dp(4.))
                .py(dp(4.))
                .rounded(dp(8.))
                .border_1()
                .border_color(theme.pop.opacity(0.6))
                .child(
                    h_flex()
                        .px(dp(8.))
                        .py(dp(6.))
                        .gap(dp(8.))
                        .child(byline_avatar(
                            &newest.author,
                            &newest.email,
                            newest.agent.as_ref(),
                            18.,
                            theme,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(format!("{} you haven't seen", plural(n, "commit"))),
                        )
                        .child(faint(format!("by {}", by.join(", ")), theme))
                        .when(run.is_some(), |el| {
                            el.child(
                                div()
                                    .text_color(theme.diff_added)
                                    .child(format!("+{added}")),
                            )
                            .child(
                                div()
                                    .text_color(theme.diff_removed)
                                    .child(format!("−{removed}")),
                            )
                        }),
                )
                .children(file_rows)
                .children(self.now_fold(
                    "now-unseen-more",
                    files.saturating_sub(UNSEEN_FILES_SHOWN),
                    theme,
                    cx,
                ))
                .child(
                    h_flex()
                        .px(dp(8.))
                        .pt(dp(6.))
                        .pb(dp(4.))
                        .gap(dp(8.))
                        .child(
                            button("now-review", label)
                                .small()
                                .style(ButtonStyle::Pop)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let (project, ids) = &review;
                                    this.review_unseen(project, ids, None, cx)
                                })),
                        )
                        .child(
                            button("now-read", "Mark as read")
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
    }

    /// Open commits `ids` together, as Review does, on `file` once their diff shows.
    fn review_unseen(
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

    /// The files written most, `hot`, when `said` says. Clicking one lists only the commits
    /// that changed it under Today, as an area filters what shipped; Clear lets go of it.
    fn now_hot_files(
        &self,
        mut hot: Vec<(Arc<str>, usize)>,
        said: String,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let picked = self.now.file.as_ref();
        // The one picked stays, with its Clear, once it's no longer written.
        if let Some(picked) = picked.filter(|p| !hot.iter().any(|(path, _)| path == *p)) {
            hot.push((picked.clone(), 0));
        }
        let most = hot.first()?.1.max(1);
        let detail = match picked {
            Some(_) => button("now-hot-clear", "Clear")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.pick_hot_file(None, cx)))
                .into_any_element(),
            None => faint(said, theme).into_any_element(),
        };
        let shown = self.now_shown("now-hot-more", HOT_SHOWN);
        let listed = |ix: usize, path: &Arc<str>| ix < shown || picked == Some(path);
        let hidden = (hot.iter().enumerate())
            .filter(|(ix, (path, _))| *ix >= HOT_SHOWN && picked != Some(path))
            .count();
        let rows =
            (hot.iter().enumerate())
                .filter(|(ix, (path, _))| listed(*ix, path))
                .map(|(ix, (path, n))| {
                    let on = picked == Some(path);
                    let (track, fill) = match on {
                        true => (theme.text_inverted.opacity(0.25), theme.muted_on_solid()),
                        false => (theme.border_3, theme.pop.opacity(0.7)),
                    };
                    let path = path.clone();
                    side_row(("now-hot", ix), on, true, theme)
                        .child(div().flex_1().min_w_0().truncate().child(path.to_string()))
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
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.pick_hot_file(Some(path.clone()), cx)
                        }))
                });
        Some(
            v_flex()
                .child(overview_heading("Hot files", detail, theme).h(dp(28.)))
                .children(rows)
                .children(self.now_fold("now-hot-more", hidden, theme, cx)),
        )
    }

    /// List only the commits that changed `file` under Today, or all with `None`, reading
    /// which files today's commits change first, unless known.
    fn pick_hot_file(&mut self, file: Option<Arc<str>>, cx: &mut Context<Self>) {
        self.now.file = file.filter(|file| self.now.file.as_ref() != Some(file));
        cx.notify();
        let Some((project, worktree)) = self
            .now
            .checkout
            .clone()
            .filter(|_| self.now.file.is_some())
        else {
            return;
        };
        let store = self.store.read(cx);
        let (Some(ws), Some(path)) = (store.workspace(&project), store.path(&project)) else {
            return;
        };
        // Today's commits, and those of the quarters the activity can pick.
        let now = crate::ui::now();
        let since = upstream::day_start(upstream::day_of(now)).min(now - QUARTER * BARS as i64);
        let unread: Vec<CommitId> = (checkout_commits(ws, worktree.as_deref()).into_iter())
            .filter(|commit| commit.time >= since && !self.now.paths.contains_key(&commit.id))
            .map(|commit| commit.id)
            .collect();
        if unread.is_empty() {
            return;
        }
        let read_for = (project, worktree);
        let paths = backend::run(move || backend::commit_paths(&path, &unread));
        cx.spawn(async move |this, cx| {
            let paths = paths.await;
            this.update(cx, |this, cx| {
                if this.now.checkout.as_ref() != Some(&read_for) {
                    return;
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

    /// Who made `commits`, most first, people and agents: today's, or those of the quarters
    /// picked, `range`.
    fn now_people(
        &self,
        commits: &[&Commit],
        range: Option<&str>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let mut counts: HashMap<&str, (usize, &Commit)> = HashMap::new();
        for commit in commits {
            counts.entry(who(commit)).or_insert((0, commit)).0 += 1;
        }
        let mut people: Vec<_> = counts.into_iter().collect();
        people.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(b.0)));
        let most = people.first()?.1.0.max(1);
        let hidden = people.len().saturating_sub(WHO_SHOWN);
        let shown = self.now_shown("now-who-more", WHO_SHOWN);
        Some(
            v_flex()
                .child(
                    overview_heading(
                        "Who worked here",
                        faint(
                            range.map_or("commits today".to_owned(), |r| format!("commits · {r}")),
                            theme,
                        ),
                        theme,
                    )
                    // As tall as Hot files' beside it, which has room for Clear.
                    .h(dp(28.)),
                )
                .children(people.into_iter().take(shown).map(|(name, (n, commit))| {
                    still_row(theme)
                        .child(byline_avatar(
                            &commit.author,
                            &commit.email,
                            commit.agent.as_ref(),
                            16.,
                            theme,
                        ))
                        .child(div().flex_1().min_w_0().truncate().child(name.to_owned()))
                        .child(bar(n as f32 / most as f32, theme.pop.opacity(0.7), theme))
                        .child(overview_column(n.to_string(), OVERVIEW_TIME_WIDTH, theme))
                }))
                .children(self.now_fold("now-who-more", hidden, theme, cx)),
        )
    }

    /// Today's commits, or those of the quarters picked, `range`, and, in a linked worktree,
    /// what else moved its `HEAD` then, newest first. A commit opens on a click.
    fn now_today(
        &self,
        project: &ProjectId,
        todays: &[&Commit],
        range: Option<&str>,
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
        let commits: Vec<&Commit> = todays.iter().filter(changed).copied().collect();
        let span = &self.now.span;
        let events = (self.now.events.iter())
            .filter(|(at, _)| file.is_none() && span.keeps(at.div_euclid(QUARTER)));
        let mut items: Vec<(i64, Item)> = (commits.iter().map(|c| (c.time, Item::Commit(c))))
            .chain(events.map(|(at, said)| (*at, Item::Moved(said))))
            .collect();
        if items.is_empty() && file.is_none() {
            return None;
        }
        items.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        let hidden = items.len().saturating_sub(TODAY_SHOWN);
        let shown = self.now_shown("now-today-more", TODAY_SHOWN);
        let store = self.store.read(cx);
        let ws = store.workspace(project);
        let rows = (items.into_iter().take(shown).enumerate()).map(|(ix, (at, item))| {
            let row = list_row(("now-today", ix), false, theme);
            match item {
                Item::Commit(commit) => {
                    let at_commit = Selection::Commit(project.clone(), commit.id);
                    // The pull request that takes it there, else the branch it's on, its
                    // beginning faint beside the glyph and all of it on hover.
                    let branch =
                        (ws.and_then(|ws| ws.find_commit(commit.id))).map(|(branch, _)| {
                            let name = store.name_of(project, branch).to_owned();
                            let ((glyph, color), label, tip) = match &branch.review {
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
                            };
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
                        .child(byline_avatar(
                            &commit.author,
                            &commit.email,
                            commit.agent.as_ref(),
                            16.,
                            theme,
                        ))
                        .child(overview_column(relative_time(at), TODAY_TIME_WIDTH, theme))
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
                    .child(overview_column(relative_time(at), TODAY_TIME_WIDTH, theme)),
            }
        });
        let reading = file.is_some() && todays.iter().any(|c| !self.now.paths.contains_key(&c.id));
        let detail = match file {
            Some(file) => {
                let name = file.rsplit('/').next().unwrap_or(file);
                format!("{} · changed {name}", plural(commits.len(), "commit"))
            }
            None => plural(todays.len(), "commit"),
        };
        let (title, detail) = match range {
            Some(range) => ("Commits", format!("{detail} · {range}")),
            None => ("Today", detail),
        };
        Some(
            v_flex()
                .child(overview_heading(title, faint(detail, theme), theme))
                .children(rows)
                .when(commits.is_empty() && file.is_some(), |el| {
                    let said = match (reading, range) {
                        (true, _) => "Reading which commits changed it…",
                        (false, Some(_)) => "No commit then changed it",
                        (false, None) => "No commit today changed it",
                    };
                    el.child(faint(said, theme).px(dp(14.)).py(dp(4.)))
                })
                .children(self.now_fold("now-today-more", hidden, theme, cx)),
        )
    }

    /// How many rows the section `fold` folds lists: its first `first`, or all once unfolded.
    fn now_shown(&self, fold: &'static str, first: usize) -> usize {
        match self.now.unfolded.contains(fold) {
            true => usize::MAX,
            false => first,
        }
    }

    /// "N more" under a section listing its first few, `hidden` left out, or "Show fewer"
    /// once it lists them all, as the navigator folds a long list: its glyph under the rows'
    /// glyphs, its words under their titles.
    fn now_fold(
        &self,
        fold: &'static str,
        hidden: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Stateful<Div>> {
        let (glyph, said) = match (hidden, self.now.unfolded.contains(fold)) {
            (0, _) => return None,
            (_, true) => (Icon::ChevronUp, "Show fewer".to_owned()),
            (n, false) => (Icon::Kebab, format!("{n} more")),
        };
        let row = fold_row(fold, glyph, 14., 34., theme).child(said);
        Some(row.on_click(cx.listener(move |this, _, _, cx| {
            if !this.now.unfolded.remove(fold) {
                this.now.unfolded.insert(fold);
            }
            cx.notify();
        })))
    }
}

/// A row of an overview that opens nothing, laid out as a list's but not lit under the
/// pointer.
fn still_row(theme: &Theme) -> Div {
    h_flex()
        .flex_none()
        .h(dp(ROW_HEIGHT))
        .mx(dp(6.))
        .px(dp(8.))
        .gap(dp(8.))
        .text_color(theme.text_1)
}

/// A bar of `share` of the most, for an overview row, as the project page's areas have.
fn bar(share: f32, color: gpui::Hsla, theme: &Theme) -> Div {
    bar_on(share, theme.border_3, color)
}

/// A bar of `share` of the most, filled with `fill` on `track`.
fn bar_on(share: f32, track: gpui::Hsla, fill: gpui::Hsla) -> Div {
    div()
        .flex_1()
        .max_w(dp(OVERVIEW_COUNT_WIDTH * 2.))
        .h(dp(6.))
        .rounded(dp(3.))
        .bg(track)
        .child(div().h_full().rounded(dp(3.)).w(relative(share)).bg(fill))
}
