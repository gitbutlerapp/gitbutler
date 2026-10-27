//! A project's page: where it is, when it last fetched, how busy the target has been and
//! who's been committing to it, and its history, the commits the workspace doesn't have yet
//! first, each with the pull request that landed it. Selecting a commit shows its diff.

use std::cell::Cell;
use std::collections::{BTreeSet, HashSet};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, MouseButton, MouseDownEvent, Pixels, Point, SharedString, div, prelude::*,
};

mod shipped;

use super::Detail;
use super::panes::{
    Crumb, CrumbTo, LIST_MIN_HEIGHT, clear_button, list_heading, measured, meta_item, meta_row,
    not_found, side_header, side_list, step_from, title_text, unfinished_banner,
};
use super::pull_request::TABS_HEIGHT;
use super::slots::{SlotGesture, SlotPick, ends_drag, picks_slot};
use crate::backend::upstream::{self, ACTIVITY_DAYS, Overview, Person, TargetCommit};
use crate::backend::{self, Checkout, CommitId, DiffSource, Forge};
use crate::overlays::MenuItem;
use crate::state::{Cut, Pane, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::emoji::ProjectIcon;
use crate::ui::markdown;
use crate::ui::person::{byline, byline_avatar};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, age_column, button, dp, error_summary, faint, fixed, fold_button,
    guide, h_flex, icon, list_row, muted, plural, quiet, resize_handle, resize_pane, section_label,
    shortcut, side_row, small_icon, v_flex,
};

/// Where the middle of a merge's chevron sits in its row: the row's padding and half the
/// chevron's step.
const FOLD_MIDDLE: f32 = 8. + 7.;

/// What the page opens on: what landed lately, even when nothing did, once the target's
/// summed up, else the newest commit.
fn opening(summary: Option<&Overview>, commits: &[TargetCommit]) -> Option<Picked> {
    (summary.is_some().then_some(Picked::Shipped))
        .or_else(|| commits.first().map(|c| Picked::Commit(c.id)))
}

/// What a target commit is called: the pull request it landed, by its title, as Lite names
/// it, or else the commit's own title.
fn label(commit: &TargetCommit) -> SharedString {
    match (commit.review.as_ref()).filter(|review| !review.title.is_empty()) {
        Some(review) => review.title.clone().into(),
        None => crate::ui::commit_title(&commit.title),
    }
}

/// How many target commits a page holds.
const PAGE: u32 = 50;

/// A reload that changes nothing the list shows still refreshes it this often, for the
/// fetch time.
const STALE_AFTER: Duration = Duration::from_secs(60);

/// How wide the diff has to stay for the files beside a list page's list to show there
/// rather than behind the button over the diff.
const MIN_DIFF: f32 = 560.;

/// What's picked on the page: what landed lately, or a commit of the target.
#[derive(Clone, Copy, PartialEq)]
enum Picked {
    Shipped,
    Commit(CommitId),
}

pub(super) struct Upstream {
    commits: Vec<TargetCommit>,
    more: bool,
    selected: Option<Picked>,
    /// The commit of the selected merge shown in place of everything it brought in.
    merged_pick: Option<CommitId>,
    /// What landed lately and the activity, read with the newest page; `None` until read, or
    /// when there's no target to read them from.
    summary: Option<Overview>,
    /// When the project last fetched successfully, and why the last fetch failed; `None`
    /// when it has no remote to fetch from.
    fetch: Option<(Option<i64>, Option<String>)>,
    /// The forge the project is on, when no account for it is connected.
    unconnected_forge: Option<Forge>,
    /// What the list was loaded for, and when.
    seen: (u64, Instant),
    /// Why reading the history failed, shown in place of the list.
    failed: Option<String>,
    /// Reading the next page, which starts once its row scrolls into view, or on Show older.
    paging: Cell<Paging>,
    /// A commit picked off what landed lately, not listed yet: the pages are read until it is.
    pending: Option<CommitId>,
    view: View,
}

impl Upstream {
    /// The activity's days picked, while any are.
    fn picked_days(&self) -> Option<&BTreeSet<i64>> {
        Some(&self.view.days.slots).filter(|days| !days.is_empty())
    }

    /// The person picked, if any.
    fn picked_person(&self) -> Option<&Person> {
        let email = self.view.person.as_ref()?;
        let summary = self.summary.as_ref()?;
        summary.people.iter().find(|person| person.email == *email)
    }
}

/// What the history is filtered to, by the activity's days, whose commits by email and the
/// part of the repository by what landed there, and the merges whose commits show under them.
/// A reload keeps it.
#[derive(Default)]
struct View {
    days: SlotPick,
    person: Option<String>,
    area: Option<String>,
    open_merges: HashSet<CommitId>,
}

#[derive(Clone, Copy, PartialEq)]
enum Paging {
    Idle,
    Loading,
    /// Reading it failed, so it waits for a click rather than trying again every frame.
    Failed,
}

enum Row {
    Heading(String),
    Commit(usize),
    /// A merge's commit, by the merge's index and its own among those it merged.
    Merged(usize, usize),
    /// What there is to say under a merge instead of its commits.
    Note(&'static str),
    More,
    /// Past the activity's days, while filtering: older ones, read on a click.
    Older,
}

impl Detail {
    /// Load the newest target commits, or with `more` the next page; unless `force`, only
    /// when the target moved or the list is a minute old.
    pub(super) fn load_upstream(
        &mut self,
        project: &ProjectId,
        more: bool,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        if self.editing(project, cx).is_some() {
            return;
        }
        let seen = self.upstream_fingerprint(project, cx);
        let current = self.upstream_for(project);
        if !force
            && !more
            && current.is_some_and(|u| {
                u.failed.is_none() && u.seen.0 == seen && u.seen.1.elapsed() < STALE_AFTER
            })
        {
            return;
        }
        // Older ones say they're reading, wherever the read started.
        if let Some(upstream) = current.filter(|_| more) {
            upstream.paging.set(Paging::Loading);
        }
        let after = current
            .filter(|_| more)
            .and_then(|u| u.commits.last())
            .map(|commit| commit.id);
        // Reading back for a commit picked off Shipped or for a filter, a few pages at once
        // rather than a frame's wait between each, and for words, which can be years back on
        // a click, many more.
        let words = !self.target_filter.read(cx).text().trim().is_empty();
        let reading_back = words
            || current.is_some_and(|u| {
                u.pending.is_some() || u.view.area.is_some() || u.view.person.is_some()
            });
        let limit = match more {
            true if words => PAGE * 100,
            true if reading_back => PAGE * 8,
            true => PAGE,
            false => current.map_or(PAGE, |u| (u.commits.len() as u32).max(PAGE)),
        };
        let project = project.clone();
        let workspace = self.store.read(cx).workspace(&project);
        let forge = workspace.and_then(|ws| ws.forge);
        // What landed lately, only with the newest page: paging further back keeps what was
        // read.
        let newest = after.is_none();
        let now = crate::ui::now();
        // The branches the header counts are the Branches page's, read on their own rather
        // than waited for; that page then opens with them.
        if newest && workspace.is_some_and(|ws| ws.target.is_some()) {
            self.load_branches(&project, false, cx);
        }
        self._upstream_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                // What landed lately takes longest, so it's read beside the page rather than
                // after it.
                std::thread::scope(|scope| {
                    let summary = newest.then(|| scope.spawn(|| upstream::overview(&path, now)));
                    let (mut commits, mut more) = upstream::target_commits(&path, after, limit)?;
                    // The newest page stops where the workspace forked off, often a commit or
                    // two in, so the rest of a page is read with it rather than a wait after.
                    let short = PAGE.saturating_sub(commits.len() as u32);
                    if let Some(last) = commits.last().map(|c| c.id).filter(|_| more && short > 0) {
                        let (older, older_more) =
                            upstream::target_commits(&path, Some(last), short)?;
                        commits.extend(older);
                        more = older_more;
                    }
                    let page = (commits, more);
                    let connected = forge.is_none_or(|forge| {
                        (backend::accounts::accounts().iter()).any(|account| account.forge == forge)
                    });
                    let fetch = upstream::fetch_status(&path)?;
                    let summary = summary
                        .and_then(|s| joined(s.join(), "what landed"))
                        .flatten();
                    Ok((page, fetch, forge.filter(|_| !connected), summary))
                })
            })
            .await;
            this.update(cx, |this, cx| {
                let ((commits, mut has_more), fetch, unconnected_forge, summary) = match result {
                    Ok(loaded) => loaded,
                    // Paging further back keeps what's listed.
                    Err(err) if after.is_some() => {
                        if let Some(upstream) = this.upstream.get_mut(&project) {
                            upstream.paging.set(Paging::Failed);
                            // A commit waited for isn't, rather than with every reload.
                            upstream.pending = None;
                            cx.notify();
                        }
                        this.store.update(cx, |store, cx| {
                            store.failed("Couldn't read more of the history", &err, cx)
                        });
                        return;
                    }
                    Err(err) => {
                        // When it last fetched is still known, though the history can't be read,
                        // and what it's filtered to stays. On a first read, only whether it has a
                        // remote may be known.
                        let remote = (this.store.read(cx).workspace(&project))
                            .is_none_or(|ws| !ws.remoteless);
                        let (fetch, view) = (this.upstream.remove(&project))
                            .map_or((remote.then_some((None, None)), View::default()), |u| {
                                (u.fetch, u.view)
                            });
                        this.upstream.insert(
                            project.clone(),
                            Upstream {
                                commits: Vec::new(),
                                more: false,
                                selected: None,
                                merged_pick: None,
                                summary: None,
                                fetch,
                                unconnected_forge: None,
                                seen: (seen, Instant::now()),
                                failed: Some(backend::read_failure(&err)),
                                paging: Cell::new(Paging::Idle),
                                pending: None,
                                view,
                            },
                        );
                        this.refresh_diff(false, cx);
                        return cx.notify();
                    }
                };
                let mut previous = this.upstream.remove(&project);
                let view =
                    (previous.as_mut().map(|u| std::mem::take(&mut u.view))).unwrap_or_default();
                let pending = previous.as_ref().and_then(|u| u.pending);
                // Read before its project's target was known, as an unwatched one's first page
                // is, the page fell back to its newest commit: it opens properly now.
                let first_summed = previous.as_ref().is_some_and(|u| u.summary.is_none());
                // A page further back says nothing of whether the newest are current, as when
                // it cut short the reload after a fetch.
                let now = (seen, Instant::now());
                let (mut list, was, pick, seen, summary, older) = match previous {
                    Some(previous) if after.is_some() => (
                        previous.commits,
                        previous.selected,
                        previous.merged_pick,
                        previous.seen,
                        previous.summary,
                        Vec::new(),
                    ),
                    // The newest page stops where the workspace forked off, so the older pages
                    // read before stay, from where it stops, and one picked from them with it.
                    Some(previous) => {
                        let mut read = previous.commits;
                        let at = (commits.last())
                            .and_then(|last| read.iter().position(|c| c.id == last.id));
                        let older = match at {
                            Some(at) => {
                                has_more = previous.more;
                                read.split_off(at + 1)
                            }
                            None => Vec::new(),
                        };
                        let (selected, pick) = (previous.selected, previous.merged_pick);
                        (Vec::new(), selected, pick, now, summary, older)
                    }
                    None => (Vec::new(), None, None, now, summary, Vec::new()),
                };
                list.extend(commits);
                list.extend(older);
                let selected = was
                    .filter(|_| !(first_summed && summary.is_some()))
                    .filter(|picked| match picked {
                        Picked::Shipped => summary.is_some(),
                        Picked::Commit(id) => list.iter().any(|c| c.id == *id),
                    })
                    .or_else(|| opening(summary.as_ref(), &list));
                // A commit picked before it was listed, once it is, while it can be: what landed
                // lately goes back no further than the summary's first day.
                let found = pending.filter(|id| list.iter().any(|c| c.id == *id));
                let within = (summary.as_ref().zip(list.last()))
                    .is_some_and(|(s, oldest)| oldest.time >= upstream::day_start(s.first_day));
                let pending = pending.filter(|_| found.is_none() && has_more && within);
                this.upstream.insert(
                    project.clone(),
                    Upstream {
                        commits: list,
                        more: has_more,
                        selected,
                        // A merge's commit stays picked while the merge stays selected.
                        merged_pick: pick.filter(|_| selected == was),
                        summary,
                        fetch,
                        unconnected_forge,
                        seen,
                        failed: None,
                        paging: Cell::new(Paging::Idle),
                        pending,
                        view,
                    },
                );
                this.refresh_diff(false, cx);
                this.load_selected_merge(&project, cx);
                // Left since, the page keeps the pick for later rather than taking over what's
                // shown now, as a composer opened meanwhile.
                let shown = matches!(&this.selection(cx),
                    Some(Selection::Project(p)) if *p == project);
                match found {
                    Some(id) if shown => {
                        this.pick_on_page(&project, Picked::Commit(id), None, cx);
                        this.scroll_to_picked(&project, cx);
                    }
                    Some(id) => {
                        if let Some(upstream) = this.upstream.get_mut(&project) {
                            upstream.selected = Some(Picked::Commit(id));
                        }
                    }
                    None => {}
                }
                if shown {
                    this.open_waiting(true, cx);
                }
                // A page further back that cut short a reload of the newest reads them again.
                if pending.is_some() {
                    this.load_upstream(&project, true, false, cx);
                } else if shown && seen.0 != this.upstream_fingerprint(&project, cx) {
                    this.load_upstream(&project, false, false, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    fn upstream_fingerprint(&self, project: &ProjectId, cx: &Context<Self>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let store = self.store.read(cx);
        // A fetch that brought nothing in still changes when the page says it fetched, and
        // one that failed, what it says about that.
        (store.fetched_at(project), store.fetch_failed_at(project)).hash(&mut hasher);
        if let Some(workspace) = store.workspace(project) {
            workspace
                .target
                .as_ref()
                .map(|t| (&t.name, t.behind))
                .hash(&mut hasher);
            for branch in workspace.branches_in(&Checkout::Main) {
                branch.commits.last().map(|c| c.id.0).hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn upstream_for(&self, project: &ProjectId) -> Option<&Upstream> {
        self.upstream.get(project)
    }

    pub(super) fn selected_target_commit(&self, project: &ProjectId) -> Option<&TargetCommit> {
        let upstream = self.upstream_for(project)?;
        let Picked::Commit(id) = upstream.selected? else {
            return None;
        };
        upstream.commits.iter().find(|c| c.id == id)
    }

    pub(super) fn upstream_source(
        &self,
        project: &ProjectId,
        cx: &gpui::App,
    ) -> Option<DiffSource> {
        match self.upstream_for(project)?.selected? {
            // Or the commit of a merge picked to show in its place.
            Picked::Commit(id) => Some(DiffSource::Commit(
                self.merged_commit(project, cx).map_or(id, |c| c.id),
            )),
            Picked::Shipped => None,
        }
    }

    /// Whether the project's page shows a commit of its target, with its message over its
    /// diff, as it does unless it's editing a commit.
    pub(super) fn showing_target_commit(&self, project: &ProjectId, cx: &gpui::App) -> bool {
        self.editing(project, cx).is_none() && self.selected_target_commit(project).is_some()
    }

    /// The merge selected on the project's page, and the commits it brought in, once read.
    fn selected_merge<'a>(
        &'a self,
        project: &ProjectId,
        cx: &'a gpui::App,
    ) -> Option<(&'a TargetCommit, Option<&'a [TargetCommit]>)> {
        let commit = self.selected_target_commit(project).filter(|c| c.merge)?;
        let merged = (self.store.read(cx).merged.get(&commit.id))
            .and_then(|m| m.as_ref())
            .map(|(commits, _)| commits.as_slice());
        Some((commit, merged))
    }

    /// The commit of a merge picked to show in its place.
    fn merged_commit<'a>(
        &'a self,
        project: &ProjectId,
        cx: &'a gpui::App,
    ) -> Option<&'a TargetCommit> {
        let (_, merged) = self.selected_merge(project, cx)?;
        let pick = self.upstream_for(project)?.merged_pick?;
        merged?.iter().find(|c| c.id == pick)
    }

    /// The target commit whose message shows over the diff: the commit of a merge picked, or
    /// else the one selected.
    pub(super) fn shown_target_commit<'a>(
        &'a self,
        project: &ProjectId,
        cx: &'a gpui::App,
    ) -> Option<&'a TargetCommit> {
        (self.merged_commit(project, cx)).or_else(|| self.selected_target_commit(project))
    }

    /// Whether the window leaves the diff out, as a pull request and what landed take its
    /// place.
    pub fn diff_hidden(&self, cx: &gpui::App) -> bool {
        self.showing_review(cx)
            || self.showing_shipped(cx)
            || self.showing_branches_overview(cx)
            || self.showing_now(cx)
            || self.choosing_target(cx)
            || self.overview_only(cx)
    }

    /// Whether the project's page asks which of the local branches tracking its target is it.
    pub fn choosing_target(&self, cx: &gpui::App) -> bool {
        let store = self.store.read(cx);
        match self.selection(cx) {
            Some(Selection::Project(project)) => {
                (store.project(&project)).is_some_and(|entry| entry.target_choice.is_some())
            }
            _ => false,
        }
    }

    /// Whether a list page shows a commit's or a branch's files between its list and the diff.
    pub(super) fn has_files_beside(&self, cx: &gpui::App) -> bool {
        let shows = match &self.selection(cx) {
            Some(Selection::Project(project)) => self.showing_target_commit(project, cx),
            Some(Selection::Branches(project)) => self.selected_other_branch(project).is_some(),
            _ => false,
        };
        shows && !self.showing_review(cx)
    }

    /// How wide the files beside a list page's list are drawn: as dragged, narrowed to leave
    /// the diff its room, or 0 when even their narrowest doesn't.
    pub(super) fn beside_files_width(&self, cx: &gpui::App) -> f32 {
        let room = self.beside_width.get() - MIN_DIFF;
        let width = self.store.read(cx).width(Pane::BesideFiles).min(room);
        match width >= Pane::BesideFiles.limits().0 {
            true => width,
            false => 0.,
        }
    }

    /// How far right of a list page's list the diff starts: past the files, when they're shown
    /// and there's room for them; 0 when they're left to the button over the diff.
    pub fn beside_offset(&self, cx: &gpui::App) -> f32 {
        match self.has_files_beside(cx) && !self.files_hidden {
            true => self.beside_files_width(cx),
            false => 0.,
        }
    }

    /// Read the selected commit's merged commits, if it's a merge.
    fn load_selected_merge(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let merge = (self.selected_target_commit(project))
            .filter(|c| c.merge)
            .map(|c| c.id);
        if let Some(id) = merge {
            self.store
                .update(cx, |store, cx| store.load_merged(project, id, cx));
        }
    }

    /// Show merge `id`'s commits under it, or fold them away, by hand: it stays as it's left.
    /// Folding them over the one picked shows the merge.
    fn toggle_merge(&mut self, project: &ProjectId, id: CommitId, cx: &mut Context<Self>) {
        let Some(upstream) = self.upstream.get_mut(project) else {
            return;
        };
        if upstream.view.open_merges.insert(id) {
            self.store
                .update(cx, |store, cx| store.load_merged(project, id, cx));
        } else {
            upstream.view.open_merges.remove(&id);
            if upstream.selected == Some(Picked::Commit(id)) && upstream.merged_pick.is_some() {
                self.pick_on_page(project, Picked::Commit(id), None, cx);
            }
        }
        cx.notify();
    }

    /// Beside a list page's list, what's selected in it: its `header`, a commit's message or a
    /// branch's, if any; tabs when it has a pull request; and under them the pull request, or
    /// the files when there's room, the window drawing the diff over the rest.
    pub(super) fn list_beside(
        &self,
        project: &ProjectId,
        header: Option<AnyElement>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let below = match self.showing_review(cx) {
            true => self.render_review_tab(project, theme, cx),
            false => self.render_files_beside(theme, cx),
        };
        let tabs = self.selected_review(cx).map(|review| {
            h_flex()
                .flex_none()
                .h(dp(TABS_HEIGHT))
                .px(dp(16.))
                .items_end()
                .border_b_1()
                .border_color(theme.border_3)
                .child(self.review_tabs(review, theme, cx))
        });
        // How wide it is, for whether the files fit beside the diff.
        let width = measured(&self.beside_width, &self.app, |bounds| {
            bounds.size.width.into()
        });
        v_flex()
            .relative()
            .flex_1()
            .h_full()
            .min_w_0()
            .children(header)
            .children(tabs)
            .child(below)
            .child(width)
            .into_any_element()
    }

    /// Under a list page's tabs: the files, when there's room for them beside the diff, which
    /// the window draws over the rest.
    fn render_files_beside(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let offset = self.beside_offset(cx);
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .on_drag_move(resize_pane(Pane::BesideFiles, self.store.clone()))
            .when(offset > 0., |el| {
                el.child(
                    v_flex()
                        .w(dp(offset))
                        .flex_none()
                        .relative()
                        .h_full()
                        .border_r_1()
                        .border_color(theme.border_3)
                        // Its own focus, so a file picked shows solid and the arrows step
                        // through the files, as in a checkout's file list.
                        .track_focus(&self.files_focus)
                        .key_context("Files")
                        .child(self.render_diff_file_list(theme, cx))
                        .child(resize_handle(Pane::BesideFiles, self.store.clone(), theme)),
                )
            })
            .into_any_element()
    }

    /// A target commit's message: its title and who wrote it and when, with a link that shows
    /// the rest of it, when there is any.
    pub(super) fn render_target_message(
        &self,
        commit: &TargetCommit,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        // Folded, it's as tall whatever the commit, so picking another moves nothing below:
        // one line of title, and one of who wrote it, which holds the link.
        const LINE: f32 = 20.;
        // A merge names the pull request by its title, which the body often repeats.
        let names_review =
            |line: &&str| (commit.review.as_ref()).is_some_and(|r| line.trim() == r.title);
        let own = (commit.body.lines())
            .filter(|line| !names_review(line))
            .any(|line| !line.trim().is_empty());
        // A merge with no more to say than the pull request it landed says what that does, in
        // its description's words.
        let described;
        let text = match (own, &commit.review) {
            (false, Some(review)) if !review.body.trim().is_empty() => {
                let end = review.body.floor_char_boundary(4096);
                let body = review.body.get(..end).unwrap_or(&review.body);
                described = (markdown::parse(body).iter())
                    .map(|block| block.text.as_str())
                    .filter(|text| !text.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                &described
            }
            _ => &commit.body,
        };
        let shown = (text.lines())
            .filter(|line| !names_review(line))
            .collect::<Vec<_>>()
            .join("\n");
        let shown = shown.trim().to_owned();
        let title = label(commit).to_string();
        let open = self.message_open;
        let toggle = (!shown.is_empty()).then(|| {
            let label = match open {
                true => "Hide message",
                false => "Show message",
            };
            button("target-message-more", label)
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.message_open = !this.message_open;
                    cx.notify();
                }))
        });
        // And the rule along its bottom.
        let height = measured(&self.target_message_height, &self.app, |bounds| {
            f32::from(bounds.size.height) + 1.
        });
        v_flex()
            .relative()
            .flex_none()
            .gap(dp(6.))
            .px(dp(16.))
            .py(dp(12.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .id("target-message-title")
                    .text_size(dp(15.))
                    .line_height(dp(22.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .when(!open, |el| {
                        el.truncate().tooltip(Tooltip::text(title.clone()))
                    })
                    .child(SharedString::from(title)),
            )
            // As a commit's page header says who wrote it and when, with its ID to copy.
            .child(
                meta_row(theme)
                    .child(byline(
                        &commit.author,
                        &commit.email,
                        commit.agent.as_ref(),
                        commit.time,
                        theme,
                    ))
                    .child(self.sha_chip(commit.id, theme, cx))
                    .children(toggle),
            )
            .when(open && !shown.is_empty(), |el| {
                el.child(
                    div()
                        .id("target-message-body")
                        .text_size(dp(13.))
                        .line_height(dp(LINE))
                        .text_color(theme.text_2)
                        .whitespace_normal()
                        // A long one scrolls rather than push the diff out of the window.
                        .max_h(dp(LINE * 12.))
                        .overflow_y_scroll()
                        .child(SharedString::from(shown)),
                )
            })
            .child(height)
            .into_any_element()
    }

    /// Whether the project's page shows what landed lately, in place of a diff.
    pub fn showing_shipped(&self, cx: &gpui::App) -> bool {
        match &self.selection(cx) {
            Some(Selection::Project(project)) => {
                self.editing(project, cx).is_none()
                    && (self.upstream_for(project))
                        .is_some_and(|u| u.selected == Some(Picked::Shipped))
            }
            _ => false,
        }
    }

    /// Esc on the project's page with a commit shown: back to what landed
    /// lately, whether it did.
    pub fn close_opened_target(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(Selection::Project(project)) = self.selection(cx) else {
            return false;
        };
        let opened = self.editing(&project, cx).is_none()
            && (self.upstream_for(&project)).is_some_and(|u| {
                u.summary.is_some() && !matches!(u.selected, None | Some(Picked::Shipped))
            });
        if opened {
            self.pick_on_page(&project, Picked::Shipped, None, cx);
        }
        opened
    }

    fn upstream_rows(&self, upstream: &Upstream, target: &str, cx: &gpui::App) -> Vec<Row> {
        // The days picked and which is today, while there's a summary to count them in.
        let picked = (upstream.picked_days().zip(upstream.summary.as_ref()))
            .map(|(days, s)| (days, s.first_day + s.daily.len() as i64 - 1));
        // A person picked who's no longer among the summary's matches nothing, rather than
        // letting go of the filter unseen.
        let who = upstream.view.person.as_ref();
        let person = upstream.picked_person();
        // What landed in the area picked, of what the summary has; with no summary to say, the
        // area doesn't filter.
        let area = (upstream.view.area.as_ref()).filter(|_| upstream.summary.is_some());
        let in_area: HashSet<CommitId> = (area.zip(upstream.summary.as_ref()))
            .map(|(area, summary)| {
                (summary.landings.iter())
                    .filter(|l| l.areas.iter().any(|a| a == area))
                    .map(|l| l.id)
                    .collect()
            })
            .unwrap_or_default();
        let query = self.target_filter.read(cx).text().trim().to_lowercase();
        let says = |text: &str| text.to_lowercase().contains(&query);
        // By its words, who it's by, its pull request's title or number, or its ID.
        let found = |commit: &TargetCommit| {
            query.is_empty()
                || says(&commit.title)
                || says(&commit.body)
                || says(&commit.author)
                || (commit.review.as_ref()).is_some_and(|review| {
                    says(&review.title)
                        || query.trim_start_matches('#') == review.number.to_string()
                })
                || commit.id.0.to_string().starts_with(&query)
        };
        let filtered = !query.is_empty();
        let shown = |commit: &TargetCommit| {
            picked.is_none_or(|(d, today)| d.contains(&upstream::day_of(commit.time).min(today)))
                && who.is_none_or(|_| person.is_some_and(|p| p.landed.contains(&commit.id)))
                && area.is_none_or(|_| in_area.contains(&commit.id))
                && found(commit)
        };
        let mut rows = Vec::new();
        // The navigator lists the new ones on their own, so here they're only marked.
        for (ix, commit) in (upstream.commits.iter().enumerate()).filter(|(_, c)| shown(c)) {
            if rows.is_empty() {
                // The area picked says so here too, as what picks it hides behind a commit.
                rows.push(Row::Heading(match area {
                    Some(area) => format!("On {target} · {area}"),
                    None => format!("On {target}"),
                }));
            }
            rows.push(Row::Commit(ix));
            // An open merge lists the commits it brought in under it.
            if !(commit.merge && upstream.view.open_merges.contains(&commit.id)) {
                continue;
            }
            match (self.store.read(cx).merged.get(&commit.id)).and_then(|m| m.as_ref()) {
                None => rows.push(Row::Note("Loading…")),
                Some((merged, _)) if merged.is_empty() => {
                    rows.push(Row::Note("This clone doesn't have its commits"))
                }
                Some((merged, more)) => {
                    rows.extend((0..merged.len()).map(|m| Row::Merged(ix, m)));
                    if *more {
                        rows.push(Row::Note("Older ones aren't listed"));
                    }
                }
            }
        }
        // Read further back only while the picked days may have more, or the activity's
        // days, which all of a person's are on; a filter by words reads that far on its own,
        // and older ones a click at a time, rather than the whole history.
        // Without a summary to say where they start, a filter reads on a click from the first.
        let bounded = who.is_some() || area.is_some() || filtered;
        let first = (picked.and_then(|(days, _)| days.first()).copied())
            .or_else(|| {
                (upstream.summary.as_ref())
                    .filter(|_| bounded)
                    .map(|s| s.first_day)
            })
            .map(upstream::day_start);
        let past = (who.is_some() && person.is_none())
            || first.map_or(bounded, |start| {
                (upstream.commits.last()).is_some_and(|oldest| oldest.time < start)
            });
        if upstream.more && !past {
            rows.push(Row::More);
        } else if rows.is_empty() && picked.is_some() {
            rows.push(Row::Heading(format!("No commits on {target} then")));
        } else if rows.is_empty() && bounded {
            rows.push(Row::Heading(format!("No commits on {target} match")));
        }
        if upstream.more && past && filtered && picked.is_none() && who.is_none() && area.is_none()
        {
            rows.push(Row::Older);
        }
        rows
    }

    /// What a project whose target an older GitButler stored as a remote branch asks, `choice`:
    /// which of the local branches tracking it, a row each, none named as the remote's branch,
    /// as the engine picks that one by itself. With none tracking it, the row is the one named as
    /// it, which picking makes track it. Choose another… offers every local branch. The branches
    /// not picked stay as they are.
    pub(super) fn target_chooser(
        &self,
        project: &ProjectId,
        choice: &backend::TargetChoice,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let pick = |name: &backend::FullName| {
            let (project, full) = (project.clone(), name.clone());
            cx.listener(
                move |this: &mut Self, _: &gpui::ClickEvent, window: &mut gpui::Window, cx| {
                    let (project, full) = (project.clone(), full.clone());
                    this.app.update_later(window, cx, move |app, _, cx| {
                        app.set_target(project, full, cx)
                    });
                },
            )
        };
        let remote = choice.remote.shorten();
        // Each row picks the branch it names, but the one to track the remote branch picks the
        // remote branch, which makes it track it.
        let picks: Vec<(&backend::FullName, &backend::FullName)> = match &choice.to_track {
            Some((local, _)) => vec![(local, &choice.remote)],
            None => choice.branches.iter().map(|name| (name, name)).collect(),
        };
        let asked = "Which branch is the target?";
        let untracked = format!("GitButler now targets a local branch, and none tracks {remote}.");
        let said = match &choice.to_track {
            Some((local, new)) => {
                let does = if *new {
                    "creates it to track"
                } else {
                    "has it track"
                };
                let local = backend::short_ref(local);
                format!(
                    "{untracked} Picking {local} {does} {remote}, so updates keep coming from there."
                )
            }
            None if choice.branches.is_empty() => {
                format!("{untracked} Pick the one your branches build on.")
            }
            None => {
                format!("Several branches track {remote}. Pick the one your branches build on.")
            }
        };
        let another = {
            let project = project.clone();
            let label = if picks.is_empty() {
                "Choose a branch…"
            } else {
                "Choose another…"
            };
            button("target-another", label)
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(move |this, _, window, cx| {
                    let project = project.clone();
                    this.app.update_later(window, cx, move |app, window, cx| {
                        app.pick_target(project, window, cx)
                    });
                }))
        };
        let has_rows = !picks.is_empty();
        let rows = (picks.into_iter().enumerate()).map(|(ix, (name, picked))| {
            list_row(("target-choice", ix), false, theme)
                .cursor_pointer()
                .child(small_icon(Icon::Branch).text_color(theme.text_3))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(backend::short_ref(name).into_owned()),
                )
                .on_click(pick(picked))
        });
        let answers = v_flex()
            .items_center()
            .gap(dp(8.))
            .when(has_rows, |answers| {
                answers.child(
                    v_flex()
                        .w(dp(260.))
                        .py(dp(4.))
                        .rounded(dp(8.))
                        .border_1()
                        .border_color(theme.border_3)
                        .text_left()
                        .children(rows),
                )
            })
            .child(another);
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .px(dp(32.))
            .pb(dp(60.))
            .child(
                v_flex()
                    .max_w(dp(380.))
                    .items_center()
                    .gap(dp(16.))
                    .text_center()
                    .child(
                        v_flex()
                            .items_center()
                            .gap(dp(6.))
                            .child(
                                div()
                                    .text_size(dp(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(asked),
                            )
                            .child(muted(said, theme)),
                    )
                    .child(answers),
            )
            .into_any_element()
    }

    /// `title` and `detail` over a button that picks `project`'s target branch.
    fn pick_target_state(
        &self,
        project: &ProjectId,
        title: String,
        detail: impl Into<SharedString>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let project = project.clone();
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(dp(4.))
            .px(dp(16.))
            .text_center()
            .child(
                div()
                    .text_size(dp(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(muted(detail.into(), theme).mb(dp(8.)))
            .child(
                crate::ui::button("pick-target", "Choose a target branch…").on_click(cx.listener(
                    move |this, _, window, cx| {
                        let project = project.clone();
                        this.app.update_later(window, cx, move |app, window, cx| {
                            app.pick_target(project, window, cx)
                        });
                    },
                )),
            )
            .into_any_element()
    }

    /// Pick the activity's days as `gesture` does, which filters the history to them.
    fn day_gesture(&mut self, project: &ProjectId, gesture: SlotGesture, cx: &mut Context<Self>) {
        if let Some(upstream) = self.upstream.get_mut(project)
            && upstream.view.days.apply(gesture)
        {
            self.show_newest_kept(cx);
            cx.notify();
        }
    }

    fn clear_days(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        if let Some(upstream) = self.upstream.get_mut(project) {
            upstream.view.days = SlotPick::default();
        }
        cx.notify();
    }

    /// Filter the history to what landed `email`'s commits, or not at all.
    fn filter_landed(
        &mut self,
        project: &ProjectId,
        email: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(upstream) = self.upstream.get_mut(project) {
            upstream.view.person = email;
        }
        self.show_newest_kept(cx);
        cx.notify();
    }

    /// Filter the history to what landed in `area`, or let go of the area.
    fn filter_area(&mut self, project: &ProjectId, area: Option<String>, cx: &mut Context<Self>) {
        if let Some(upstream) = self.upstream.get_mut(project) {
            upstream.view.area = area;
        }
        self.show_newest_kept(cx);
        cx.notify();
    }

    /// Back to the top of the history, its newest commit kept, as a filter changes.
    pub(super) fn show_newest_kept(&mut self, _cx: &mut Context<Self>) {
        self.upstream_scroll
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
    }

    /// Whose commits landed on the target, the most landings first, counted on the days
    /// picked or all the activity's, and only those that changed the area picked; the one picked
    /// stays, however few they have then. Clicking one filters the history to what landed theirs.
    fn render_people(
        &self,
        project: &ProjectId,
        upstream: &Upstream,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let summary = upstream.summary.as_ref()?;
        let (days, who) = (upstream.picked_days(), upstream.view.person.as_ref());
        if summary.people.is_empty() && who.is_none() {
            return None;
        }
        // What landed in the area picked on those days, while one is.
        let in_area: Option<HashSet<CommitId>> = (upstream.view.area.as_ref()).map(|area| {
            (summary.landings.iter())
                .filter(|l| l.areas.contains(area) && days.is_none_or(|d| d.contains(&l.day)))
                .map(|l| l.id)
                .collect()
        });
        let on_days = |person: &Person| -> usize {
            if let Some(in_area) = &in_area {
                return person.landed.intersection(in_area).count();
            }
            (person.daily.iter().enumerate())
                .filter(|(ix, _)| {
                    days.is_none_or(|d| d.contains(&(summary.first_day + *ix as i64)))
                })
                .map(|(_, n)| *n as usize)
                .sum()
        };
        let mut people: Vec<_> = (summary.people.iter())
            .map(|person| (person, on_days(person)))
            .filter(|(person, n)| *n > 0 || who == Some(&person.email))
            .map(|(p, n)| (p.email.clone(), p.name.clone(), p.email.clone(), n))
            .collect();
        people.sort_by_key(|(.., n)| std::cmp::Reverse(*n));
        // Landings, as a merge bringing in several of someone's commits counts once.
        let counted = match days {
            None => format!("landings · {ACTIVITY_DAYS} days"),
            Some(_) => "landings · days picked".to_owned(),
        };
        let project = project.clone();
        Some(self.people_card(
            people,
            who.map(String::as_str),
            (counted, "No commits landed then"),
            move |this, email, cx| this.filter_landed(&project, email, cx),
            window,
            cx,
        ))
    }

    /// The page's crumbs after the project's: the target and the commit of it selected.
    pub(super) fn upstream_crumbs(
        &self,
        project: &ProjectId,
        crumbs: &mut Vec<Crumb>,
        cx: &Context<Self>,
    ) {
        let Some(upstream) = self.upstream_for(project) else {
            return;
        };
        // Over what landed lately, the target names the page, last.
        let target = (self.store.read(cx).workspace(project)).and_then(|ws| ws.target_name());
        if upstream.selected == Some(Picked::Shipped) {
            return crumbs.push((target.unwrap_or("Target".into()).into_owned().into(), None));
        }
        let Some(picked) = (self.selected_target_commit(project)).map(|c| c.id.short()) else {
            return;
        };
        // The target goes back to how the page opens, as the project's crumb does.
        let page = Selection::Project(project.clone()).into();
        crumbs.push((
            target.unwrap_or("Target".into()).into_owned().into(),
            Some(page),
        ));
        let merged = self.merged_commit(project, cx);
        // A merge's commit shown in its place goes back to the merge.
        let merge = match upstream.selected {
            Some(Picked::Commit(id)) if merged.is_some() => Some(CrumbTo::Merge(id)),
            _ => None,
        };
        crumbs.push((picked.into(), merge));
        if let Some(commit) = merged {
            crumbs.push((commit.id.short().into(), None));
        }
    }

    /// Opening the project's page, wherever from: it shows what landed lately, rather than
    /// what was picked on it last, as the Branches page shows its overview.
    pub(super) fn open_on_shipped(&mut self, project: &ProjectId) {
        let Some(upstream) = self.upstream.get_mut(project) else {
            return;
        };
        // A read cut short when the page was left isn't still going.
        upstream.paging.set(Paging::Idle);
        upstream.pending = None;
        if upstream.summary.is_some() {
            upstream.selected = Some(Picked::Shipped);
            upstream.merged_pick = None;
        }
    }

    /// Back to what the page opens on, as when it's picked again.
    pub(super) fn pick_opening(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(upstream) = self.upstream_for(project) else {
            return;
        };
        if let Some(picked) = opening(upstream.summary.as_ref(), &upstream.commits) {
            self.pick_on_page(project, picked, None, cx)
        }
    }

    /// Pick commit `id` on the project page shown, as the crumb of a merge does over a commit
    /// of it.
    pub(super) fn pick_target_commit(&mut self, id: CommitId, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.pick_on_page(&project, Picked::Commit(id), None, cx)
        }
    }

    /// Open the commit on `project`'s history that landed a branch: the one that landed its
    /// pull request `pull`, else its commit `id` itself, as when it landed as it was. Says
    /// whether the history lists either.
    pub(super) fn open_landing(
        &mut self,
        project: &ProjectId,
        pull: Option<u64>,
        id: CommitId,
        cx: &mut Context<Self>,
    ) -> bool {
        let commits = self.upstream_for(project).map_or(&[][..], |u| &u.commits);
        let landed = (commits.iter())
            .find(|c| c.id == id || (c.review.as_ref()).is_some_and(|r| Some(r.number) == pull))
            .map(|c| c.id);
        if let Some(landed) = landed {
            self.pick_landed(project, landed, cx);
        }
        landed.is_some()
    }

    /// Read the next page of `project`'s history, while its oldest read is no older than
    /// `since`. Says whether it does.
    pub(super) fn read_back_to(
        &mut self,
        project: &ProjectId,
        since: i64,
        cx: &mut Context<Self>,
    ) -> bool {
        let older = (self.upstream_for(project))
            .is_some_and(|u| u.more && u.commits.last().is_some_and(|c| c.time >= since));
        if older {
            self.load_upstream(project, true, false, cx);
        }
        older
    }

    /// Open commit `id` of what landed lately, reading older pages of the history until
    /// it's listed.
    fn pick_landed(&mut self, project: &ProjectId, id: CommitId, cx: &mut Context<Self>) {
        let Some(upstream) = self.upstream.get_mut(project) else {
            return;
        };
        if upstream.commits.iter().any(|c| c.id == id) {
            self.pick_on_page(project, Picked::Commit(id), None, cx);
            return self.scroll_to_picked(project, cx);
        }
        if !upstream.more {
            return;
        }
        upstream.pending = Some(id);
        self.load_upstream(project, true, false, cx);
    }

    /// Bring the commit picked into view in the history, as it was picked from elsewhere.
    fn scroll_to_picked(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(upstream) = self.upstream_for(project) else {
            return;
        };
        let Some(Picked::Commit(id)) = upstream.selected else {
            return;
        };
        let picked = |ix: &usize| upstream.commits.get(*ix).is_some_and(|c| c.id == id);
        let row = (self.upstream_rows(upstream, "", cx).iter())
            .position(|row| matches!(row, Row::Commit(ix) if picked(ix)));
        if let Some(row) = row {
            self.upstream_scroll
                .scroll_to_item(row, gpui::ScrollStrategy::Center);
        }
    }

    /// Pick `picked` on `project`'s page, and with `merged` a commit of that merge to show in
    /// its place.
    fn pick_on_page(
        &mut self,
        project: &ProjectId,
        picked: Picked,
        merged: Option<CommitId>,
        cx: &mut Context<Self>,
    ) {
        // A commit picked again, as by a second click, is shown already, and doesn't change.
        let again = |u: &Upstream| u.selected == Some(picked) && u.merged_pick == merged;
        if matches!(picked, Picked::Commit(_)) && self.upstream.get(project).is_some_and(again) {
            return;
        }
        if let Some(upstream) = self.upstream.get_mut(project) {
            upstream.selected = Some(picked);
            upstream.merged_pick = merged;
            // Picked now, a commit still waited for no longer is.
            upstream.pending = None;
        }
        self.message_open = false;
        self.load_selected_merge(project, cx);
        self.show_picked(cx);
    }

    /// Select the target's commit `merge`, or the one of its commits `merged`, and offer what
    /// applies to it there: its merged commits shown or hidden, its pull request opened and
    /// its ID copied, as for a branch's. Only one the workspace doesn't have yet is
    /// cherry-picked into a branch, as the others are already under every branch, and not a
    /// merge.
    fn target_commit_menu(
        &mut self,
        project: &ProjectId,
        merge: CommitId,
        merged: Option<CommitId>,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.pick_on_page(project, Picked::Commit(merge), merged, cx);
        let project = project.clone();
        let items = move |this: &mut Self, _: &mut Context<Self>| {
            this.target_commit_items(&project, merge, merged)
        };
        self.open_menu(at, items, cx);
    }

    /// What right-clicking the target's commit `merge`, or `merged` among its commits, offers.
    fn target_commit_items(
        &self,
        project: &ProjectId,
        merge: CommitId,
        merged: Option<CommitId>,
    ) -> Vec<MenuItem> {
        let Some(upstream) = self.upstream_for(project) else {
            return Vec::new();
        };
        let Some(commit) = upstream.commits.iter().find(|c| c.id == merge) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        if merged.is_none() {
            let shown = project.clone();
            if commit.merge {
                let label = match upstream.view.open_merges.contains(&merge) {
                    true => "Hide commits",
                    false => "Show commits",
                };
                items.push(MenuItem::new(label, move |app, _, cx| {
                    (app.detail).update(cx, |detail, cx| detail.toggle_merge(&shown, merge, cx))
                }));
            }
            if commit.incoming && !commit.merge {
                let picked = project.clone();
                items.push(MenuItem::new("Cherry-pick", move |app, _, cx| {
                    app.cut(Cut::CherryPick(picked.clone(), [merge].into()), cx)
                }));
            }
            if let Some(url) = (commit.review.as_ref()).and_then(|r| r.url.clone()) {
                items.push(MenuItem::new("Open pull request", move |_, _, cx| {
                    cx.open_url(&url)
                }));
            }
        }
        let id = merged.unwrap_or(merge);
        let copy = MenuItem::new("Copy commit ID", move |app, _, cx| {
            app.copy_text(id.0.to_string(), cx)
        });
        items.push(match items.is_empty() {
            true => copy,
            false => copy.separated(),
        });
        items
    }

    /// ⇧→ in the list: open the picked merge, or step into its commits when it's open, as
    /// on the Branches page. ⇧← folds it, or steps from one of its commits back up to it.
    fn fold_upstream(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(project) = self.selected_project(cx) else {
            return;
        };
        let Some(upstream) = self.upstream_for(&project) else {
            return;
        };
        let Some(Picked::Commit(id)) = upstream.selected else {
            return;
        };
        let merge = (upstream.commits.iter()).any(|commit| commit.id == id && commit.merge);
        match (
            open,
            upstream.merged_pick.is_some(),
            upstream.view.open_merges.contains(&id),
        ) {
            (true, false, false) if merge => self.toggle_merge(&project, id, cx),
            (true, false, true) => self.step_upstream(1, cx),
            (false, true, _) => self.pick_on_page(&project, Picked::Commit(id), None, cx),
            (false, false, true) => self.toggle_merge(&project, id, cx),
            _ => {}
        }
    }

    /// Select the target commit `delta` rows from the selected one, keeping it in view.
    fn step_upstream(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(project) = self.selected_project(cx) else {
            return;
        };
        let Some(upstream) = self.upstream_for(&project) else {
            return;
        };
        let merged = |ix: usize, m: usize| {
            let merge = upstream.commits.get(ix)?.id;
            let merged = self.store.read(cx).merged.get(&merge)?.as_ref()?;
            Some((Picked::Commit(merge), Some(merged.0.get(m)?.id)))
        };
        // A merge's commit by the merge and it.
        let picks: Vec<_> = (self.upstream_rows(upstream, "", cx).iter().enumerate())
            .filter_map(|(row, r)| match r {
                Row::Commit(ix) => {
                    Some((row, (Picked::Commit(upstream.commits.get(*ix)?.id), None)))
                }
                Row::Merged(ix, m) => Some((row, merged(*ix, *m)?)),
                _ => None,
            })
            .collect();
        // From what landed lately, which sits over the list, only down.
        if upstream.selected == Some(Picked::Shipped) && delta < 0 {
            return;
        }
        let current = (upstream.selected, upstream.merged_pick);
        let at = (picks.iter()).position(|(_, (picked, pick))| (Some(*picked), *pick) == current);
        let Some(&(row, (picked, pick))) = step_from(&picks, at, delta) else {
            return;
        };
        self.pick_on_page(&project, picked, pick, cx);
        (self.upstream_scroll).scroll_to_item(row, gpui::ScrollStrategy::Nearest);
    }

    /// How busy the target has been: a bar for each day, the last week's stronger, counting
    /// only what landed the person picked's commits and changed the area picked, as the history
    /// lists them. Clicking a day, shift-clicking another or dragging across them filters the
    /// history to them.
    fn render_activity(
        &self,
        project: &ProjectId,
        upstream: &Upstream,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let summary = upstream.summary.as_ref()?;
        // A person picked who's no longer among those who landed anything keeps nothing.
        let (who, area) = (upstream.view.person.as_ref(), upstream.view.area.as_ref());
        let theirs = upstream.picked_person().map(|p| &p.landed);
        let mut daily = vec![0; summary.daily.len()];
        for landing in (summary.landings.iter())
            .filter(|l| who.is_none() || theirs.is_some_and(|theirs| theirs.contains(&l.id)))
            .filter(|l| area.is_none_or(|area| l.areas.contains(area)))
        {
            let at = usize::try_from(landing.day - summary.first_day).ok();
            if let Some(n) = at.and_then(|at| daily.get_mut(at)) {
                *n += 1;
            }
        }
        let (project, cleared) = (project.clone(), project.clone());
        Some(activity_strip(
            &daily,
            summary.first_day,
            &upstream.view.days.slots,
            theme,
            cx,
            move |this, gesture, cx| this.day_gesture(&project, gesture, cx),
            move |this, cx| this.clear_days(&cleared, cx),
        ))
    }

    /// A project's header: its icon, which opens the picker, and its name, over where it is
    /// and when it last fetched.
    pub(super) fn render_project_header(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return div().into_any_element();
        };
        let mark = ProjectIcon::parse(store.project_icon(project)).render(theme.scaled(16.), theme);
        let fetch = self.upstream_for(project).map(|u| u.fetch.clone());
        let picked = project.clone();
        // As wide as the other pages' icons, so the titles line up.
        let trigger = div()
            .id("project-icon")
            .flex_none()
            .w(dp(16.))
            .h(dp(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(dp(4.))
            .hover(|el| el.bg(theme.hover()))
            .text_size(dp(16.))
            .child(mark)
            // No tooltip: it would sit over the picker's search field.
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    // Just under the button, which the click is half its height above.
                    let at =
                        event.position() + gpui::point(Pixels::ZERO, Theme::get(cx).scaled(14.));
                    let project = picked.clone();
                    this.app.update_later(window, cx, move |app, window, cx| {
                        app.open_icon_picker(project, at, window, cx)
                    });
                }),
            );
        let meta = meta_row(theme)
            .child(
                div()
                    .min_w_0()
                    // Cut in the middle, keeping the folder's own name at its end.
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .child(crate::ui::short_path(&entry.project.path)),
            )
            .when(!entry.folder_gone, |el| {
                let said = match &fetch {
                    Some(None) => "No remote to fetch from".into(),
                    Some(Some((Some(at), _))) => format!("Fetched {}", crate::ui::ago(*at)),
                    _ => "Not fetched yet".into(),
                };
                let busy = store.fetching(project) && !matches!(fetch, Some(None));
                // Fetching says so over what it said, which keeps its room, so nothing beside
                // it moves; it's shorter than any of them.
                let item = meta_item(Icon::Refresh, theme).flex_none().child(
                    div()
                        .relative()
                        .child(div().when(busy, |el| el.invisible()).child(said))
                        .when(busy, |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .whitespace_nowrap()
                                    .child("Fetching…"),
                            )
                        }),
                );
                // When it last fetched, which fetches again on a click, as ⌘R does.
                el.child(match fetch {
                    Some(None) => item.into_any_element(),
                    _ => {
                        let hover = theme.text_1;
                        div()
                            .id("fetch-now")
                            .flex_none()
                            .hover(move |el| el.text_color(hover))
                            .tooltip(Tooltip::keyed(
                                "Fetch, and check pull requests and CI again",
                                shortcut(&crate::app::Fetch, cx),
                            ))
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(crate::app::Fetch), cx)
                            })
                            .child(item)
                            .into_any_element()
                    }
                })
            });
        // What's going on in it, in counts, after where it is on the same line, which gives
        // way first.
        let workspace = entry.workspace.as_ref();
        let applied = workspace.map_or(0, |ws| ws.branches_in(&Checkout::Main).count());
        let unapplied = self.unapplied(project);
        let worktrees = workspace.map_or(0, |ws| ws.worktrees.len());
        let meta = meta
            .child(
                meta_item(Icon::Branch, theme)
                    .flex_none()
                    .child(format!("{applied} applied")),
            )
            .when(worktrees > 0, |el| {
                el.child(
                    meta_item(Icon::Folder, theme)
                        .flex_none()
                        .child(plural(worktrees, "linked worktree")),
                )
            })
            .children(self.mesh_meta(project, theme, cx))
            .children(workspace.and_then(|ws| {
                let target = ws.target.as_ref()?;
                let remote = target.untracked.as_ref()?;
                Some(
                    meta_item(Icon::Warning, theme)
                        .flex_none()
                        .id("untracked-target")
                        .text_color(theme.warn)
                        .child(format!("{} doesn't track {remote}", target.name))
                        .tooltip(Tooltip::text(format!(
                            "What's pushed to {remote} doesn't show here, nor can branches be \
                             pushed. Track it with `git branch -u {remote} {}`",
                            target.name
                        ))),
                )
            }))
            // Last, as it's read after the rest, so nothing moves when it shows.
            .children(unapplied.map(|n| {
                meta_item(Icon::Branch, theme)
                    .flex_none()
                    .id("unapplied-count")
                    .child(format!("{n} not applied"))
                    .tooltip(Tooltip::text(
                        "Branches in progress that aren't applied: with commits of their own, \
                         and no merged or closed pull request",
                    ))
            }));
        let title = h_flex()
            .items_start()
            .gap(dp(8.))
            .child(trigger)
            .child(title_text(entry.project.name.clone()));
        let target = Selection::Project(project.clone());
        self.page_header(title, meta, target, None, None, theme, cx)
    }

    pub(super) fn render_upstream_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return not_found("Project not found", theme, cx);
        };
        let target = entry
            .workspace
            .as_ref()
            .and_then(|ws| ws.target.as_ref())
            .map(|t| t.name.clone());
        let upstream = self.upstream_for(project);
        let missing = entry.folder_gone;
        let fetch_error = upstream.and_then(|u| u.fetch.as_ref()?.1.clone());
        let fetched = upstream.and_then(|u| u.fetch.as_ref()?.0);
        let fetching = store.fetching(project);
        // Said in or outside the workspace, as the uncommitted changes say it.
        let unfinished = (entry.workspace.as_ref())
            .and_then(|ws| ws.git_operation)
            .map(|op| unfinished_banner(op, theme));
        // A box of what needs doing, with the button that does it.
        let callout = |ground: gpui::Hsla| {
            v_flex()
                .items_start()
                .mt(dp(4.))
                .gap(dp(8.))
                .p(dp(8.))
                .rounded(dp(6.))
                .bg(ground)
        };
        // Where it is and when it fetched head its page; here, what needs doing, if anything.
        let needs_doing: Vec<AnyElement> = [
            // As a conflict is, a condition with its fix rather than an alarm: what Git said
            // shows on hover, the column being narrow.
            fetch_error.map(|err| {
                let last = fetched.map(|at| format!(" It last fetched {}.", crate::ui::ago(at)));
                let again = super::resolve::banner_action("fetch-again", "Try again", theme)
                    .disabled_because(fetching.then_some("Fetching…"))
                    .dispatch(&crate::app::Fetch);
                super::resolve::warn_banner(
                    "Couldn't fetch",
                    format!("{}{}", git_said(&err), last.unwrap_or_default()),
                    Some(again.into_any_element()),
                    theme,
                )
                .into_any_element()
            }),
            upstream.and_then(|u| u.unconnected_forge).map(|forge| {
                callout(theme.hover())
                    .child(div().text_color(theme.text_1).child(format!(
                        "Connect {} to see pull requests and CI here.",
                        forge.name()
                    )))
                    .child(
                        crate::ui::button("connect-forge", "Connect…")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.app.update_later(window, cx, move |app, window, cx| {
                                    app.open_accounts(window, cx)
                                });
                            })),
                    )
                    .into_any_element()
            }),
            missing.then(|| {
                let remove = project.clone();
                callout(theme.danger.opacity(0.08))
                    .child(
                        div()
                            .text_color(theme.danger)
                            .child("The project's folder isn't there any more."),
                    )
                    .child(
                        crate::ui::button("forget", "Remove")
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let project = remove.clone();
                                this.store
                                    .update(cx, |store, cx| store.remove_project(&project, cx));
                            })),
                    )
                    .into_any_element()
            }),
            // Which branch is the target is asked beside, so it isn't said here too.
            (entry.error.clone())
                .filter(|_| !missing && entry.target_choice.is_none())
                .map(|err| {
                    h_flex()
                        .gap(dp(8.))
                        .text_color(theme.danger)
                        .child(icon(Icon::Warning).text_color(theme.danger))
                        .child(div().flex_1().min_w_0().child(err))
                        .into_any_element()
                }),
        ]
        .into_iter()
        .flatten()
        .collect();
        let list = match upstream {
            // Its banner says so; reading its history fails with no more to say.
            _ if missing => div().flex_1().into_any_element(),
            None => crate::ui::loading_state("Loading…", theme, cx),
            // Picking the target again is the way out of what it most often is, as a target that
            // more than one local branch tracks.
            // Its target is being asked for beside, the history waiting on it.
            Some(Upstream {
                failed: Some(_), ..
            }) if entry.target_choice.is_some() => crate::ui::empty_state(
                None,
                "",
                "The target's history shows here once it's picked",
                theme,
                cx,
            ),
            Some(Upstream {
                failed: Some(err), ..
            }) => self.pick_target_state(
                project,
                "Couldn't read the target's history".to_owned(),
                error_summary(err),
                theme,
                cx,
            ),
            // The page lists what it read, whether or not the project is expanded and loaded.
            Some(upstream) if upstream.commits.is_empty() => {
                // A target the remote renamed or deleted is said so, not as if never picked.
                let lost = (entry.workspace.as_ref()).and_then(|ws| ws.lost_target.clone());
                let unpushed = (entry.workspace.as_ref()).and_then(|ws| ws.target_branch.clone());
                let untargeted = (entry.workspace.as_ref()).is_some() && unpushed.is_none();
                let (title, detail) = match lost {
                    Some(name) => (
                        format!("{name} is gone"),
                        "The remote no longer has it. Pick the branch your work goes into now; \
                         one only the remote has needs a local branch first, as \
                         `git switch -c main --track origin/main` makes",
                    ),
                    // A loaded project without a target needs one picked first, remote or not.
                    None if untargeted => (
                        "No target branch".to_owned(),
                        "Pick the branch your work goes into, like main",
                    ),
                    // The history is the target's remote branch's, so a target picked here
                    // shows none either.
                    None if upstream.fetch.is_none() => (
                        "No remote".to_owned(),
                        "The target's history shows here once the repository has a remote",
                    ),
                    // Picked, but with no branch on the remote yet, as before a first push.
                    None if let Some(name) = unpushed => (
                        format!("{} isn't on the remote yet", backend::short_ref(&name)),
                        "Push it once, with `git push -u`, and its history shows here",
                    ),
                    None => (
                        "No target branch".to_owned(),
                        "Pick the branch your work goes into, like main",
                    ),
                };
                self.pick_target_state(project, title, detail, theme, cx)
            }
            Some(upstream) => {
                let target = target.unwrap_or_else(|| "the target".into());
                let rows = self.upstream_rows(upstream, &target, cx);
                let shown = project.clone();
                let list = side_list(
                    "target-commits",
                    rows.len(),
                    &self.upstream_scroll,
                    "target-scrollbar",
                    cx,
                    move |this, ix, theme, window, cx| {
                        let upstream = this.upstream_for(&shown)?;
                        let solid = this.side_focus.is_focused(window);
                        // A commit, or one that merge `under` brought in. A click picks it and
                        // a right-click opens its menu.
                        let commit_row = |commit: &TargetCommit, under: Option<&TargetCommit>| {
                            let id = commit.id;
                            let (merge, merged) = under.map_or((id, None), |m| (m.id, Some(id)));
                            // One a merge brought in is as new to the workspace as the merge.
                            let incoming = under.map_or(commit.incoming, |m| m.incoming);
                            let selected = upstream.selected == Some(Picked::Commit(merge))
                                && upstream.merged_pick == merged;
                            let on = selected && solid;
                            // Landed, as everything here has, in purple, as the app marks what
                            // landed; amber while the workspace doesn't have it yet.
                            let glyph = match (on, incoming) {
                                (true, _) => theme.text_inverted,
                                (false, true) => theme.warn,
                                (false, false) => theme.purple,
                            };
                            // One that landed a pull request by the request, as Lite names it,
                            // its number, merge commit and ID on hover.
                            let tip = (commit.review.as_ref()).map(|review| {
                                let short = id.short();
                                format!("#{} · {} · {short}", review.number, commit.title)
                            });
                            let mark = match commit.review {
                                Some(_) => Icon::BranchMerge,
                                None => Icon::CommitFill,
                            };
                            // A merge's chevron folds the commits it brought in, by hand; the
                            // others keep its room, so the titles line up.
                            let fold = match commit.merge && under.is_none() {
                                false => div().flex_none().w(dp(14.)).into_any_element(),
                                true => {
                                    let project = shown.clone();
                                    let open = upstream.view.open_merges.contains(&id);
                                    let ink = match on {
                                        true => theme.text_inverted,
                                        false => theme.text_3,
                                    };
                                    fold_button(
                                        ("target-commit-fold", ix),
                                        Some(open),
                                        ink,
                                        cx,
                                        move |this, _, cx| this.toggle_merge(&project, id, cx),
                                    )
                                    .into_any_element()
                                }
                            };
                            let (project, menu_project) = (shown.clone(), shown.clone());
                            side_row(("target-commit", ix), selected, solid, theme)
                                // One a merge brought in, as a branch's commits are under it: a
                                // line down from the merge's chevron.
                                .when(under.is_some(), |el| el.child(guide(FOLD_MIDDLE, theme)))
                                .child(fold)
                                .child(small_icon(mark).text_color(glyph))
                                .child(
                                    div()
                                        .id(("target-commit-title", ix))
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .when(!incoming && !on, |el| el.text_color(theme.text_2))
                                        .child(label(commit))
                                        .when_some(tip, |el, tip| el.tooltip(Tooltip::text(tip))),
                                )
                                // Who wrote it, as on the Branches page.
                                .child(commit_author(
                                    ("target-commit-author", ix),
                                    commit,
                                    on,
                                    theme,
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.pick_on_page(&project, Picked::Commit(merge), merged, cx)
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        let (project, at) = (&menu_project, event.position);
                                        this.target_commit_menu(project, merge, merged, at, cx)
                                    }),
                                )
                                .into_any_element()
                        };
                        Some(match rows.get(ix)? {
                            Row::Heading(heading) => list_heading(heading.clone(), theme),
                            // Under a merge's commits' titles.
                            Row::Note(note) => h_flex()
                                .h(dp(crate::ui::ROW_HEIGHT))
                                .mx(dp(6.))
                                .pl(dp(8. + 14. + 8. + 12. + 8.))
                                .child(guide(FOLD_MIDDLE, theme))
                                .child(faint(*note, theme))
                                .into_any_element(),
                            Row::Merged(merge_ix, merged_ix) => {
                                let merge = upstream.commits.get(*merge_ix)?;
                                let (merged, _) =
                                    this.store.read(cx).merged.get(&merge.id)?.as_ref()?;
                                commit_row(merged.get(*merged_ix)?, Some(merge))
                            }
                            Row::Commit(commit_ix) => {
                                commit_row(upstream.commits.get(*commit_ix)?, None)
                            }
                            // A page at a time, on a click, saying so while it reads.
                            Row::Older if upstream.paging.get() == Paging::Loading => {
                                list_row("target-older", false, theme)
                                    .text_color(theme.text_3)
                                    .child("Loading…")
                                    .into_any_element()
                            }
                            // Saying how far back it has read, as a filter can find nothing
                            // in years of history.
                            Row::Older => {
                                let project = shown.clone();
                                let read_to = (upstream.commits.last()).map_or_else(
                                    || "Show older".to_owned(),
                                    |c| format!("Show older than {}", crate::ui::day_label(c.time)),
                                );
                                list_row("target-older", false, theme)
                                    .text_color(theme.text_2)
                                    .child(read_to)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_upstream(&project, true, false, cx);
                                        cx.notify();
                                    }))
                                    .into_any_element()
                            }
                            // Drawn once it scrolls into view, which reads the next page.
                            Row::More => {
                                let project = shown.clone();
                                if upstream.paging.get() == Paging::Idle {
                                    upstream.paging.set(Paging::Loading);
                                    let project = project.clone();
                                    cx.spawn(async move |this, cx| {
                                        this.update(cx, |this, cx| {
                                            this.load_upstream(&project, true, false, cx)
                                        })
                                        .ok();
                                    })
                                    .detach();
                                }
                                match upstream.paging.get() {
                                    Paging::Failed => list_row("target-more", false, theme)
                                        .text_color(theme.text_2)
                                        .child("Show older")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.load_upstream(&project, true, false, cx)
                                        }))
                                        .into_any_element(),
                                    _ => list_row("target-more", false, theme)
                                        .text_color(theme.text_3)
                                        .child("Loading…")
                                        .into_any_element(),
                                }
                            }
                        })
                    },
                );
                // The activity over the history, which scrolls between it and the people.
                self.list_column(Self::step_upstream, Some(Self::fold_upstream), None, cx)
                    .child(self.filter_field(&self.target_filter, window, cx))
                    .children(self.render_activity(project, upstream, theme, cx))
                    .child(list.flex_1().min_h(dp(LIST_MIN_HEIGHT)))
                    .children(self.render_people(project, upstream, window, cx))
                    .into_any_element()
            }
        };
        v_flex()
            .size_full()
            .children(unfinished)
            .when(!needs_doing.is_empty(), |el| {
                el.child(side_header(theme).text_size(dp(12.)).children(needs_doing))
            })
            .child(list)
            .into_any_element()
    }
}

/// Who wrote a commit listed beside the diff, their name on hover, and how long ago, in a row's
/// last columns, which a branch row's line up with.
pub(super) fn commit_author(
    id: impl Into<gpui::ElementId>,
    commit: &TargetCommit,
    on: bool,
    theme: &Theme,
) -> gpui::Div {
    let by = match &commit.agent {
        Some(agent) => format!("Committed by {}, written by {}", commit.author, agent.name),
        None => format!("Authored by {}", commit.author),
    };
    h_flex()
        .flex_none()
        .gap(dp(8.))
        .child(
            div()
                .id(id)
                .flex_none()
                .child(byline_avatar(
                    &commit.author,
                    &commit.email,
                    commit.agent.as_ref(),
                    14.,
                    theme,
                ))
                .tooltip(Tooltip::text(by)),
        )
        .child(quiet(age_column(Some(commit.time), theme), on, theme))
}

/// The days picked, said short: one by its date, a span by its ends, days apart by how
/// many; `None` while none is.
fn days_said(picked: &BTreeSet<i64>) -> Option<String> {
    let date = |day: i64| crate::ui::local_date(noon(day));
    let (&first, &last) = picked.first().zip(picked.last())?;
    Some(match picked.len() {
        1 => date(first),
        len if (last - first + 1) as usize == len => format!("{} – {}", date(first), date(last)),
        len => format!("{len} days"),
    })
}

/// What a thread reading `what` found, or nothing, saying why it couldn't.
fn joined<T>(read: std::thread::Result<anyhow::Result<T>>, what: &str) -> Option<T> {
    read.ok()?
        .inspect_err(|err| tracing::warn!(?err, "couldn't read {what}"))
        .ok()
}

/// A day's noon, clear of a clock change, to date it by.
pub(super) fn noon(day: i64) -> i64 {
    const DAY: i64 = 24 * 60 * 60;
    upstream::day_start(day) + DAY / 2
}

/// How busy the target has been: a bar for each day of `daily`, from `first_day`, the last
/// week's stronger. Pressing a day, shift-pressing another or dragging across them hands
/// `gesture` what the pointer did, for a [`SlotPick`]; Clear, beside the days picked, calls
/// `clear`.
fn activity_strip(
    daily: &[u32],
    first_day: i64,
    picked: &BTreeSet<i64>,
    theme: &Theme,
    cx: &Context<Detail>,
    gesture: impl Fn(&mut Detail, SlotGesture, &mut Context<Detail>) + Clone + 'static,
    clear: impl Fn(&mut Detail, &mut Context<Detail>) + 'static,
) -> AnyElement {
    const HEIGHT: f32 = 36.;
    let total: u32 = daily.iter().sum();
    let week: u32 = daily.iter().rev().take(7).sum();
    let most = daily.iter().copied().max().unwrap_or(0).max(1);
    let today = first_day + daily.len() as i64 - 1;
    let bars: Vec<_> = daily
        .iter()
        .enumerate()
        .map(|(ix, &n)| {
            let day = first_day + ix as i64;
            let recent = today - day < 7;
            let inside = picked.is_empty() || picked.contains(&day);
            let said = match n {
                0 => "quiet".to_owned(),
                n => plural(n as usize, "commit"),
            };
            let tip = format!("{} · {said}", crate::ui::local_weekday(noon(day)));
            // Room under the bars, so a band of days picked holds them as the quarters' does.
            let column = div()
                .id(("activity-day", ix))
                .flex_1()
                .h(dp(HEIGHT + 4.))
                .px(dp(1.))
                .pb(dp(4.))
                .flex()
                .items_end()
                .tooltip(Tooltip::text(tip));
            picks_slot(column, day, picked, theme, gesture.clone(), cx).child(
                div()
                    .w_full()
                    .h(dp((n as f32 / most as f32 * HEIGHT).max(2.)))
                    .rounded_t(fixed(1.))
                    .bg(match (n, picked.is_empty(), inside, recent) {
                        (0, ..) => theme.border_2,
                        (_, false, true, _) => theme.pop,
                        (_, false, false, _) => theme.pop.opacity(0.2),
                        (_, true, _, true) => theme.pop,
                        (_, true, _, false) => theme.pop.opacity(0.4),
                    }),
            )
        })
        .collect();
    let said = match days_said(picked) {
        Some(days) => {
            let n: u32 = (daily.iter().enumerate())
                .filter(|(ix, _)| picked.contains(&(first_day + *ix as i64)))
                .map(|(_, n)| n)
                .sum();
            h_flex()
                .gap(dp(6.))
                .child(faint(
                    format!("{days}: {}", plural(n as usize, "commit")),
                    theme,
                ))
                .child(
                    clear_button("activity-clear")
                        .on_click(cx.listener(move |this, _, _, cx| clear(this, cx))),
                )
                .into_any_element()
        }
        None => faint(
            format!(
                "{}, {week} in the last week",
                plural(total as usize, "commit")
            ),
            theme,
        )
        .into_any_element(),
    };
    v_flex()
        .px(dp(14.))
        .pt(dp(14.))
        .pb(dp(6.))
        .gap(dp(6.))
        // As tall with Clear as without, so picking a day moves nothing under it.
        .child(
            h_flex()
                .h(dp(20.))
                .gap(dp(8.))
                .child(section_label("Activity", theme).flex_grow(1.))
                .child(said),
        )
        .child(ends_drag(h_flex().children(bars), gesture, cx))
        .child(
            h_flex()
                .text_size(dp(11.))
                .child(faint(format!("{ACTIVITY_DAYS} days ago"), theme).flex_grow(1.))
                .child(faint("Today", theme)),
        )
        .into_any_element()
}

/// What Git said, as sentences: its lines without `fatal:` or `error:`, each capitalised and
/// ending in a full stop.
fn git_said(err: &str) -> String {
    (error_summary(err).lines())
        .map(|line| {
            line.trim_start_matches("fatal:")
                .trim_start_matches("error:")
                .trim()
        })
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut chars = line.chars();
            let first = chars.next().map(|c| c.to_uppercase().to_string());
            let mut said = first.unwrap_or_default() + chars.as_str();
            if !said.ends_with(['.', '!', '?']) {
                said.push('.');
            }
            said
        })
        .collect::<Vec<_>>()
        .join(" ")
}
