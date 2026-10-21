//! A project's page: where it is, when it last fetched, its target's README, how busy the
//! target has been and who's been committing to it, and its history, the commits the
//! workspace doesn't have yet first, each with the pull request that landed it. Selecting
//! the README shows it; selecting a commit shows its diff.

use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};

use super::Detail;
use super::panes::{
    Crumb, list_heading, meta_item, meta_row, quiet, side_header, side_list, side_row, title_text,
};
use super::pull_request::tab;
use crate::app::{SelectNext, SelectPrevious};
use crate::backend::upstream::{self, ACTIVITY_DAYS, Overview, TargetCommit};

const DAY: i64 = 24 * 60 * 60;
use crate::backend::{self, CommitId, DiffSource, Forge, Review};
use crate::palette::Choice;
use crate::state::{Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::emoji::ProjectIcon;
use crate::ui::illustration::Illustration;
use crate::ui::markdown;
use crate::ui::person::{author_avatar, byline_avatar};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    Icon, Tooltip, age_column, error_summary, faint, h_flex, icon, list_row, muted, plural,
    section_label, shortcut, small_icon, v_flex,
};

/// What a target commit is called: the pull request it landed, by its title, as Lite names
/// it, or else the commit's own title.
fn label(commit: &TargetCommit) -> &str {
    (commit.review.as_ref())
        .filter(|review| !review.title.is_empty())
        .map_or(&commit.title, |review| &review.title)
}

/// How many target commits a page holds.
const PAGE: u32 = 50;

/// A reload that changes nothing the list shows still refreshes it this often, for the
/// fetch time.
const STALE_AFTER: Duration = Duration::from_secs(60);

/// How wide the files beside a target commit's diff are, and how wide the diff has to stay
/// for them to show there rather than behind the button over the diff.
const BESIDE_FILES: f32 = 260.;
/// How wide a merge's commits are beside its diff, and how many are listed.
const COMMITS_COLUMN: f32 = 320.;
const MERGED_SHOWN: usize = 200;
/// How many of them show over the diff without room beside it, the rest scrolling; and how
/// short and, over the diff, how tall they can be dragged.
const MERGED_OVER_ROWS: usize = 6;
const MERGED_MIN: f32 = 64.;
const MERGED_OVER_MAX: f32 = 480.;

/// What dragging the line under a merge's commits carries.
struct MergedSplit;
const MIN_DIFF: f32 = 560.;

/// How many of the team the page lists.
const TEAM_SHOWN: usize = 5;

/// What's picked on the page: the README, or a commit of the target.
#[derive(Clone, Copy, PartialEq)]
enum Picked {
    Readme,
    Commit(CommitId),
}

/// The target summed up, its README read as Markdown off the UI thread.
struct Summary {
    readme: Option<(String, Vec<markdown::Block>)>,
    daily: Vec<u32>,
    team: Vec<(String, String, u32)>,
    /// When the days were counted back from.
    at: i64,
    /// How many branches aren't applied, as the Branches page lists them at first: with
    /// commits of their own, and no merged pull request.
    unapplied: Option<usize>,
}

impl From<Overview> for Summary {
    fn from(overview: Overview) -> Self {
        Summary {
            readme: (overview.readme)
                .map(|(name, text)| (name, markdown::parse_showing_images(&text))),
            daily: overview.daily,
            team: overview.team,
            at: overview.at,
            unapplied: None,
        }
    }
}

pub(super) struct Upstream {
    project: ProjectId,
    commits: Vec<TargetCommit>,
    more: bool,
    selected: Option<Picked>,
    /// The README and the activity, read with the newest page; `None` until read, or when
    /// there's no target to read them from.
    summary: Option<Summary>,
    /// When the project last fetched successfully, and why the last fetch failed; `None`
    /// when it has no remote to fetch from.
    fetch: Option<(Option<i64>, Option<String>)>,
    /// The forge the project is on, when no account for it is connected.
    unconnected_forge: Option<Forge>,
    /// What the list was loaded for, and when.
    seen: (u64, Instant),
    /// Why reading the history failed, shown in place of the list.
    failed: Option<String>,
    /// Reading the next page, which starts once its row scrolls into view.
    paging: Cell<Paging>,
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
    More,
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
        let current = self.upstream.as_ref().filter(|u| u.project == *project);
        if !force
            && !more
            && current.is_some_and(|u| {
                u.failed.is_none() && u.seen.0 == seen && u.seen.1.elapsed() < STALE_AFTER
            })
        {
            return;
        }
        let after = current
            .filter(|_| more)
            .and_then(|u| u.commits.last())
            .map(|commit| commit.id);
        // A reload reads as many as are listed, so the older pages and the selection stay.
        let limit = match more {
            true => PAGE,
            false => current.map_or(PAGE, |u| (u.commits.len() as u32).max(PAGE)),
        };
        let project = project.clone();
        let workspace = self.store.read(cx).workspace(&project);
        let forge = workspace.and_then(|ws| ws.forge);
        // Only with the newest page: paging further back keeps what was read.
        let target = (workspace.and_then(|ws| ws.target.as_ref()))
            .map(|target| target.name.clone())
            .filter(|_| after.is_none());
        let now = crate::ui::now();
        self._upstream_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                let page = upstream::target_commits(&path, after, limit)?;
                let connected = forge.is_none_or(|forge| {
                    (backend::accounts::accounts().iter()).any(|account| account.forge == forge)
                });
                // The history shows without it.
                let summary = target.and_then(|target| {
                    (upstream::overview(&path, &target, now))
                        .inspect_err(|err| tracing::warn!(?err, "couldn't sum up the target"))
                        .ok()
                        .map(|overview| Summary {
                            unapplied: (backend::branches::other_branches(&path))
                                .inspect_err(|err| tracing::warn!(?err, "couldn't count branches"))
                                .ok()
                                .map(|branches| {
                                    (branches.iter())
                                        .filter(|b| b.own_commits.unwrap_or(0) > 0)
                                        .filter(|b| {
                                            (b.review.as_ref()).is_none_or(|r| {
                                                r.state != backend::ReviewState::Merged
                                            })
                                        })
                                        .count()
                                }),
                            ..Summary::from(overview)
                        })
                });
                Ok((
                    page,
                    upstream::fetch_status(&path)?,
                    forge.filter(|_| !connected),
                    summary,
                ))
            })
            .await;
            this.update(cx, |this, cx| {
                let ((commits, has_more), fetch, unconnected_forge, summary) = match result {
                    Ok(loaded) => loaded,
                    // Paging further back keeps what's listed.
                    Err(err) if after.is_some() => {
                        if let Some(upstream) = this.upstream_for(&project) {
                            upstream.paging.set(Paging::Failed);
                            cx.notify();
                        }
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't read more of the history", format!("{err:#}"), cx)
                        });
                        return;
                    }
                    Err(err) => {
                        // When it last fetched is still known, though the history can't be read.
                        let fetch = (this.upstream_for(&project).map(|u| u.fetch.clone()))
                            .unwrap_or(Some((None, None)));
                        this.upstream = Some(Upstream {
                            project,
                            commits: Vec::new(),
                            more: false,
                            selected: None,
                            summary: None,
                            fetch,
                            unconnected_forge: None,
                            seen: (seen, Instant::now()),
                            failed: Some(format!("{err:#}")),
                            paging: Cell::new(Paging::Idle),
                        });
                        this.refresh_diff(false, cx);
                        return cx.notify();
                    }
                };
                let previous = this.upstream.take().filter(|u| u.project == project);
                // A page further back says nothing of whether the newest are current, as when
                // it cut short the reload after a fetch.
                let now = (seen, Instant::now());
                let (mut list, selected, seen, summary) = match previous {
                    Some(previous) if after.is_some() => (
                        previous.commits,
                        previous.selected,
                        previous.seen,
                        previous.summary,
                    ),
                    Some(previous) => (Vec::new(), previous.selected, now, summary),
                    None => (Vec::new(), None, now, summary),
                };
                list.extend(commits);
                // The README first, as a repository's page opens on it.
                let readme = summary.as_ref().is_some_and(|s| s.readme.is_some());
                let selected = selected
                    .filter(|picked| match picked {
                        Picked::Readme => readme,
                        Picked::Commit(id) => list.iter().any(|c| c.id == *id),
                    })
                    .or_else(|| readme.then_some(Picked::Readme))
                    .or_else(|| list.first().map(|c| Picked::Commit(c.id)));
                this.upstream = Some(Upstream {
                    project: project.clone(),
                    commits: list,
                    more: has_more,
                    selected,
                    summary,
                    fetch,
                    unconnected_forge,
                    seen,
                    failed: None,
                    paging: Cell::new(Paging::Idle),
                });
                this.refresh_diff(false, cx);
                this.load_readme_images(&project, cx);
                this.load_selected_merge(&project, cx);
                cx.notify();
                // The window draws the diff beside the list, or the README doesn't need it.
                this.app.update(cx, |_, cx| cx.notify()).ok();
            })
            .ok();
        });
    }

    fn upstream_fingerprint(&self, project: &ProjectId, cx: &Context<Self>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let store = self.store.read(cx);
        // A fetch that brought nothing in still changes when the page says it fetched.
        store.fetched_at(project).hash(&mut hasher);
        if let Some(workspace) = store.workspace(project) {
            workspace
                .target
                .as_ref()
                .map(|t| (&t.name, t.behind))
                .hash(&mut hasher);
            for branch in workspace.branches() {
                branch.commits.last().map(|c| c.id.0).hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn upstream_for(&self, project: &ProjectId) -> Option<&Upstream> {
        self.upstream.as_ref().filter(|u| u.project == *project)
    }

    /// The pull request the selected target commit landed, if it landed one.
    pub(super) fn selected_target_review(&self, project: &ProjectId) -> Option<&Review> {
        self.selected_target_commit(project)?.review.as_ref()
    }

    fn selected_target_commit(&self, project: &ProjectId) -> Option<&TargetCommit> {
        let upstream = self.upstream_for(project)?;
        let Picked::Commit(id) = upstream.selected? else {
            return None;
        };
        upstream.commits.iter().find(|c| c.id == id)
    }

    pub(super) fn upstream_source(&self, project: &ProjectId) -> Option<DiffSource> {
        match self.upstream_for(project)?.selected? {
            // Or the commit of a merge picked to show in its place.
            Picked::Commit(id) => Some(DiffSource::Commit(
                self.merged_commit(project).map_or(id, |c| c.id),
            )),
            Picked::Readme => None,
        }
    }

    /// Whether the project's page shows a commit of its target, with its message over its
    /// diff, as it does unless it's editing a commit.
    pub(super) fn showing_target_commit(&self, project: &ProjectId) -> bool {
        self.selected_target_commit(project).is_some()
    }

    /// The merge selected on the project's page, and the commits it brought in, once read.
    fn selected_merge(
        &self,
        project: &ProjectId,
    ) -> Option<(&TargetCommit, Option<&[TargetCommit]>)> {
        let commit = self.selected_target_commit(project).filter(|c| c.merge)?;
        let merged = self.merged.get(&commit.id).and_then(|m| m.as_deref());
        Some((commit, merged))
    }

    /// Whether the project's page shows a merge, which has a Commits tab.
    pub(super) fn on_merge(&self, cx: &gpui::App) -> bool {
        match self.store.read(cx).selection.as_ref() {
            Some(Selection::Project(project)) => self.selected_merge(project).is_some(),
            _ => false,
        }
    }

    /// The commit of a merge picked to show in its place.
    fn merged_commit(&self, project: &ProjectId) -> Option<&TargetCommit> {
        let (_, merged) = self.selected_merge(project)?;
        let pick = self.merged_pick?;
        merged?.iter().find(|c| c.id == pick)
    }

    /// Whether the Commits tab lists a merge's commits beside the diff, there being room;
    /// without, it lists them alone, and a commit picked opens on the Changes tab.
    fn commits_beside(&self) -> bool {
        self.beside_width.get() >= COMMITS_COLUMN + MIN_DIFF
    }

    fn showing_commits(&self, project: &ProjectId) -> bool {
        self.commits_tab && self.selected_merge(project).is_some()
    }

    /// Whether the window leaves the diff out, as a pull request and the README take its
    /// place.
    pub fn diff_hidden(&self, cx: &gpui::App) -> bool {
        self.showing_review(cx) || self.showing_readme(cx)
    }

    /// How tall a merge's commits are over its diff, without room for them beside it: a few
    /// rows, which scroll, and the rule under them.
    pub(super) fn merged_over_height(&self, project: &ProjectId) -> f32 {
        let Some((_, Some(merged))) = self.selected_merge(project) else {
            return 0.;
        };
        if !self.showing_commits(project) || self.commits_beside() {
            return 0.;
        }
        let rows = merged.len() + usize::from(merged.len() > 1);
        let fits = rows.min(MERGED_OVER_ROWS) as f32 * crate::ui::ROW_HEIGHT + 8.;
        let dragged = self
            .merged_split
            .map(|h| h.clamp(MERGED_MIN, MERGED_OVER_MAX));
        dragged.unwrap_or(fits) + 1.
    }

    /// How tall a merge's commits are over its files: as dragged, or as tall as they need.
    fn merged_split_height(&self, project: &ProjectId) -> f32 {
        let rows = (self.selected_merge(project))
            .and_then(|(_, merged)| merged)
            .map_or(1, |merged| merged.len() + usize::from(merged.len() > 1));
        let fits = rows as f32 * crate::ui::ROW_HEIGHT + 8.;
        self.merged_split.unwrap_or(fits).max(MERGED_MIN)
    }

    /// The line under a merge's commits, which drags to make them taller or shorter, and a
    /// double-click puts back to as tall as they need.
    fn merged_split_handle(&self, theme: &Theme, cx: &Context<Self>) -> impl IntoElement {
        div()
            .id("merged-split")
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(-3.))
            .h(px(5.))
            .group("merged-split")
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .block_mouse_except_scroll()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(2.))
                    .h(px(1.))
                    .group_hover("merged-split", |el| el.bg(theme.border_1)),
            )
            .on_drag(MergedSplit, |_, _, _, cx| cx.new(|_| gpui::EmptyView))
            .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    this.merged_split = None;
                    cx.notify();
                    this.app.update(cx, |_, cx| cx.notify()).ok();
                }
            }))
    }

    /// Follow the split's drag: the commits as tall as from their top to the pointer.
    fn drag_merged_split(
        &mut self,
        event: &gpui::DragMoveEvent<MergedSplit>,
        cx: &mut Context<Self>,
    ) {
        let height = f32::from(event.event.position.y - event.bounds.top());
        self.merged_split = Some(height.max(MERGED_MIN));
        cx.notify();
        // The diff under them starts lower or higher.
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    /// How far right of the project page's list the diff starts: past a merge's commits on
    /// its Commits tab, and the files, when there's room for them; 0 when they're left to the
    /// button over the diff.
    pub fn beside_offset(&self, cx: &gpui::App) -> f32 {
        let Some(Selection::Project(project)) = self.store.read(cx).selection.as_ref() else {
            return 0.;
        };
        // A merge's commits over the diff leave its files to the button over it.
        if !self.showing_target_commit(project)
            || self.showing_review(cx)
            || self.merged_over_height(project) > 0.
        {
            return 0.;
        }
        let commits = match self.showing_commits(project) && self.commits_beside() {
            true => COMMITS_COLUMN,
            false => 0.,
        };
        match self.beside_width.get() >= commits + BESIDE_FILES + MIN_DIFF {
            true => commits + BESIDE_FILES,
            false => commits,
        }
    }

    /// Read the commits the merge `id` brought in, unless they've been read.
    fn load_merged(&mut self, project: &ProjectId, id: CommitId, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        if self.merged.contains_key(&id) {
            return;
        }
        self.merged.insert(id, None);
        cx.spawn(async move |this, cx| {
            let read =
                backend::run(move || upstream::merged_commits(&path, id, MERGED_SHOWN)).await;
            this.update(cx, |this, cx| {
                let commits = read
                    .inspect_err(|err| tracing::warn!(?err, "couldn't read a merge's commits"))
                    .unwrap_or_default();
                this.merged.insert(id, Some(commits));
                cx.notify();
                this.app.update(cx, |_, cx| cx.notify()).ok();
            })
            .ok();
        })
        .detach();
    }

    /// Read the selected commit's merged commits, if it's a merge.
    fn load_selected_merge(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let merge = (self.selected_target_commit(project))
            .filter(|c| c.merge)
            .map(|c| c.id);
        if let Some(id) = merge {
            self.load_merged(project, id, cx);
        }
    }

    /// Show the merge's commit `pick` in its place, or with `None` all it brought in.
    fn pick_merged(&mut self, pick: Option<CommitId>, cx: &mut Context<Self>) {
        self.merged_pick = pick;
        self.message_open = false;
        self.show_picked(cx);
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    fn set_target_tab(
        &mut self,
        review: bool,
        commits: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.review_tab = review;
        self.commits_tab = commits;
        window.focus(&self.side_focus, cx);
        cx.notify();
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    /// Beside the project page's list, its selected commit: its message, or the message of
    /// the commit of a merge picked; tabs when it landed a pull request or is a merge; and
    /// under them the pull request, the merge's commits, and the files when there's room, the
    /// window drawing the diff over the rest.
    pub(super) fn render_target_beside(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(commit) = self.selected_target_commit(project) else {
            return div().flex_1().into_any_element();
        };
        let (width, app) = (self.beside_width.clone(), self.app.clone());
        let measure_width = gpui::canvas(
            move |bounds, _, cx| {
                let drawn = f32::from(bounds.size.width);
                if width.replace(drawn) != drawn {
                    cx.defer(move |cx| {
                        app.update(cx, |_, cx| cx.notify()).ok();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let review = commit.review.as_ref();
        let merge = self.selected_merge(project);
        let commits = self.showing_commits(project);
        let reviewing = self.review_tab && review.is_some();
        let offset = self.beside_offset(cx);
        let files = |width: f32| {
            v_flex()
                .w(px(width))
                .flex_none()
                .h_full()
                .border_r_1()
                .border_color(theme.border_3)
                .child(self.render_diff_file_list(theme, cx))
        };
        let below = if reviewing {
            self.render_review_tab(project, theme, cx)
        } else if commits && !self.commits_beside() {
            // Over the diff, which the window draws under them.
            v_flex()
                .flex_1()
                .child(
                    v_flex()
                        .relative()
                        .flex_none()
                        .h(px(self.merged_over_height(project) - 1.))
                        .py(px(4.))
                        .border_b_1()
                        .border_color(theme.border_3)
                        .on_drag_move(
                            cx.listener(|this, event, _, cx| this.drag_merged_split(event, cx)),
                        )
                        .child(self.render_merged_list(project, theme, cx))
                        .child(self.merged_split_handle(theme, cx)),
                )
                .into_any_element()
        } else if commits {
            h_flex()
                .flex_1()
                .min_h(px(0.))
                .items_start()
                .child(
                    v_flex()
                        .w(px(COMMITS_COLUMN))
                        .flex_none()
                        .h_full()
                        .border_r_1()
                        .border_color(theme.border_3)
                        .map(|el| match offset == COMMITS_COLUMN {
                            // Without room for the files beside them, they go under the
                            // commits, which take as much of the column as they need, up to a
                            // limit, or as dragged; each scrolls.
                            true => el
                                .child(
                                    v_flex()
                                        .relative()
                                        .flex_none()
                                        .h(px(self.merged_split_height(project)))
                                        .max_h(gpui::relative(match self.merged_split {
                                            Some(_) => 0.85,
                                            None => 0.6,
                                        }))
                                        .border_b_1()
                                        .border_color(theme.border_3)
                                        .on_drag_move(cx.listener(|this, event, _, cx| {
                                            this.drag_merged_split(event, cx)
                                        }))
                                        .child(self.render_merged_list(project, theme, cx))
                                        .child(self.merged_split_handle(theme, cx)),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_h(px(0.))
                                        .child(self.render_diff_file_list(theme, cx)),
                                ),
                            false => el.child(self.render_merged_list(project, theme, cx)),
                        }),
                )
                .when(offset > COMMITS_COLUMN, |el| {
                    el.child(files(offset - COMMITS_COLUMN))
                })
                .into_any_element()
        } else {
            h_flex()
                .flex_1()
                .min_h(px(0.))
                .items_start()
                .when(offset > 0., |el| el.child(files(offset)))
                .into_any_element()
        };
        let tabs = (review.is_some() || merge.is_some()).then(|| {
            let changed = (self.shown_diff().map(|diff| diff.files.len())).filter(|n| *n > 0);
            let count = merge.and_then(|(_, merged)| merged).map(<[_]>::len);
            // Without the commits beside the diff, a menu picks one to show.
            let menu = (merge.is_some() && !commits && !reviewing)
                .then(|| self.render_merged_menu(project, theme, cx));
            h_flex()
                .flex_none()
                .h(px(super::pull_request::TABS_HEIGHT))
                .px(px(16.))
                .items_end()
                .border_b_1()
                .border_color(theme.border_3)
                .child(
                    h_flex()
                        .flex_grow(1.)
                        .ml(px(-8.))
                        // A merge's commits first, as it opens on them.
                        .when(merge.is_some(), |el| {
                            el.child(
                                tab("tab-commits", "Commits", commits && !reviewing, theme)
                                    .children(count.map(|n| faint(n.to_string(), theme)))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.set_target_tab(false, true, window, cx)
                                    })),
                            )
                        })
                        .child(
                            tab("tab-changes", "Changes", !reviewing && !commits, theme)
                                .children(changed.map(|n| faint(n.to_string(), theme)))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_target_tab(false, false, window, cx)
                                })),
                        )
                        .when(review.is_some(), |el| {
                            el.child(
                                tab("tab-review", "Pull request", reviewing, theme).on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.set_target_tab(true, false, window, cx)
                                    }),
                                ),
                            )
                        }),
                )
                .children(menu.map(|menu| div().pb(px(6.)).child(menu)))
        });
        let message = self.merged_commit(project).unwrap_or(commit);
        v_flex()
            .relative()
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .child(self.render_target_message(message, theme, cx))
            .children(tabs)
            .child(below)
            .child(measure_width)
            .into_any_element()
    }

    /// The menu over a merge's diff that picks one of its commits to show, or all of them.
    fn render_merged_menu(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let shown =
            (self.merged_commit(project)).map_or("All commits".to_owned(), |c| c.title.clone());
        let project = project.clone();
        h_flex()
            .id("merged-menu")
            .max_w(px(260.))
            .gap(px(4.))
            .px(px(8.))
            .py(px(2.))
            .rounded(px(6.))
            .border_1()
            .border_color(theme.border_2)
            .cursor_pointer()
            .hover(|el| el.bg(theme.hover()))
            .text_size(px(12.))
            .child(div().min_w(px(0.)).truncate().child(shown))
            .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
            .on_click(cx.listener(move |this, _, window, cx| {
                let Some((_, Some(merged))) = this.selected_merge(&project) else {
                    return;
                };
                let detail = cx.entity();
                let pick = |label: String, pick: Option<CommitId>| {
                    let detail = detail.clone();
                    Choice::new(label, move |_, _, cx| {
                        detail.update(cx, |detail, cx| detail.pick_merged(pick, cx));
                    })
                };
                let choices = std::iter::once(pick("All commits".to_owned(), None))
                    .chain(merged.iter().map(|c| pick(c.title.clone(), Some(c.id))))
                    .collect();
                (this.app)
                    .update(cx, |app, cx| {
                        app.open_picker("Show a commit of the merge", "Show", choices, window, cx)
                    })
                    .ok();
            }))
            .into_any_element()
    }

    /// A merge's commits, under a row for all of them when there's more than one; a click
    /// shows one in the diff.
    fn render_merged_list(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some((_, merged)) = self.selected_merge(project) else {
            return div().into_any_element();
        };
        let Some(merged) = merged else {
            return crate::ui::empty_state(None, "Loading…", "", theme, cx);
        };
        let project = project.clone();
        side_list(
            "merged-commits",
            // One commit is all there is: no row for all of them.
            merged.len() + usize::from(merged.len() > 1),
            &self.merged_scroll,
            "merged-scrollbar",
            cx,
            move |this, ix, theme, _, cx| {
                let (_, merged) = this.selected_merge(&project)?;
                let merged = merged?;
                let pick = match (ix, merged.len()) {
                    (_, 1) => Some(merged.first()?),
                    (0, _) => None,
                    (ix, _) => Some(merged.get(ix - 1)?),
                };
                let id = pick.map(|c| c.id);
                let selected = this.merged_pick == id || merged.len() == 1;
                let row = side_row(("merged-commit", ix), selected, false, theme)
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_merged(id, cx)));
                Some(match pick {
                    None => row
                        .child(small_icon(Icon::BranchMerge).text_color(theme.text_3))
                        .child(
                            div()
                                .flex_1()
                                .child(format!("All {} commits merged", merged.len())),
                        )
                        .into_any_element(),
                    Some(commit) => row
                        .child(small_icon(Icon::Commit).text_color(theme.text_3))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .child(SharedString::from(commit.title.clone())),
                        )
                        .child(byline_avatar(
                            &commit.author,
                            &commit.email,
                            commit.agent.as_ref(),
                            14.,
                            theme,
                        ))
                        .child(quiet(age_column(Some(commit.time), theme), false, theme))
                        .into_any_element(),
                })
            },
        )
        .flex_1()
        .min_h(px(0.))
        .into_any_element()
    }

    /// A target commit's message: its title, who wrote it and when, and the first lines of
    /// the rest, with a button to show it all.
    fn render_target_message(
        &self,
        commit: &TargetCommit,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        const LINES: usize = 3;
        // A merge names the pull request by its title, which the body often repeats.
        let body = (commit.body.lines())
            .filter(|line| {
                commit
                    .review
                    .as_ref()
                    .is_none_or(|r| line.trim() != r.title)
            })
            .collect::<Vec<_>>();
        let start = body
            .iter()
            .position(|line| !line.trim().is_empty())
            .unwrap_or(body.len());
        let body = &body[start..];
        let long = body.len() > LINES;
        let open = self.message_open;
        let shown = match open || !long {
            true => body.join("\n"),
            false => body[..LINES].join("\n"),
        };
        let (height, app) = (self.target_message_height.clone(), self.app.clone());
        let measure = gpui::canvas(
            move |bounds, _, cx| {
                // And the rule along its bottom.
                let drawn = f32::from(bounds.size.height) + 1.;
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
            .gap(px(6.))
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(SharedString::from(label(commit).to_owned())),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child(byline_avatar(
                        &commit.author,
                        &commit.email,
                        commit.agent.as_ref(),
                        14.,
                        theme,
                    ))
                    .child(format!(
                        "{} · {} · {}",
                        commit.author,
                        crate::ui::ago(commit.time),
                        commit.id.short()
                    )),
            )
            .when(!shown.is_empty(), |el| {
                el.child(
                    div()
                        .text_color(theme.text_2)
                        .whitespace_normal()
                        .child(SharedString::from(shown)),
                )
            })
            .when(long, |el| {
                el.child(
                    div()
                        .id("target-message-more")
                        .text_size(px(12.))
                        .text_color(theme.blue)
                        .cursor_pointer()
                        .child(match open {
                            true => "Show less",
                            false => "Show more",
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.message_open = !this.message_open;
                            cx.notify();
                            this.app.update(cx, |_, cx| cx.notify()).ok();
                        })),
                )
            })
            .child(measure)
            .into_any_element()
    }

    /// Whether the project's page shows its README, in place of a diff.
    pub fn showing_readme(&self, cx: &gpui::App) -> bool {
        match self.store.read(cx).selection.as_ref() {
            Some(Selection::Project(project)) => {
                self.editing(project, cx).is_none()
                    && (self.upstream_for(project))
                        .is_some_and(|u| u.selected == Some(Picked::Readme))
            }
            _ => false,
        }
    }

    /// The times the activity's picked days span, from the start of the first to the end of
    /// the last, the last day running on past now.
    fn picked_span(&self, upstream: &Upstream) -> Option<(i64, i64)> {
        let (project, first, last) = self.activity_days.as_ref()?;
        let summary = upstream
            .summary
            .as_ref()
            .filter(|_| *project == upstream.project)?;
        let days = summary.daily.len();
        let start = summary.at - (days - first) as i64 * DAY;
        let end = match last + 1 == days {
            true => i64::MAX,
            false => summary.at - (days - 1 - last) as i64 * DAY,
        };
        Some((start, end))
    }

    fn upstream_rows(&self, upstream: &Upstream, target: &str) -> Vec<Row> {
        let span = self.picked_span(upstream);
        let shown = |commit: &TargetCommit| {
            span.is_none_or(|(start, end)| (start..end).contains(&commit.time))
        };
        let (incoming, rest): (Vec<usize>, Vec<usize>) = (0..upstream.commits.len())
            .filter(|ix| shown(&upstream.commits[*ix]))
            .partition(|ix| upstream.commits[*ix].incoming);
        let mut rows = Vec::new();
        if !incoming.is_empty() {
            rows.push(Row::Heading(format!(
                "New on {target} · {}",
                plural(incoming.len(), "commit")
            )));
            rows.extend(incoming.into_iter().map(Row::Commit));
        }
        if !rest.is_empty() {
            let heading = match rows.is_empty() {
                true => format!("On {target}"),
                false => format!("Earlier on {target}"),
            };
            rows.push(Row::Heading(heading));
            rows.extend(rest.into_iter().map(Row::Commit));
        }
        // Read further back only while the picked days may have more.
        let past = (span.zip(upstream.commits.last()))
            .is_some_and(|((start, _), oldest)| oldest.time < start);
        if upstream.more && !past {
            rows.push(Row::More);
        }
        rows
    }

    /// Pick the activity's day `ix`, or with `extend` the days from the one picked before
    /// to it; picking the one day picked again shows every day.
    fn pick_days(&mut self, project: &ProjectId, ix: usize, extend: bool, cx: &mut Context<Self>) {
        let anchor = self.activity_anchor.filter(|_| extend);
        let same =
            matches!(&self.activity_days, Some((p, a, b)) if p == project && *a == ix && *b == ix);
        self.activity_days = match anchor {
            Some(anchor) => Some((project.clone(), anchor.min(ix), anchor.max(ix))),
            None if same => None,
            None => Some((project.clone(), ix, ix)),
        };
        if anchor.is_none() {
            self.activity_anchor = self.activity_days.as_ref().map(|_| ix);
        }
        self.upstream_scroll
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
        cx.notify();
    }

    fn clear_days(&mut self, cx: &mut Context<Self>) {
        self.activity_days = None;
        self.activity_anchor = None;
        cx.notify();
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
        let picked = match upstream.selected {
            Some(Picked::Readme) => (upstream.summary.as_ref())
                .and_then(|s| s.readme.as_ref())
                .map(|(name, _)| name.clone()),
            Some(Picked::Commit(_)) => {
                (self.selected_target_commit(project)).map(|commit| label(commit).to_owned())
            }
            None => None,
        };
        let Some(picked) = picked else {
            return;
        };
        let target = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.target.as_ref())
            .map_or("Target".to_owned(), |target| target.name.clone());
        crumbs.push((target.into(), None));
        crumbs.push((picked.into(), None));
        if let Some(commit) = self.merged_commit(project) {
            crumbs.push((commit.title.clone().into(), None));
        }
    }

    fn pick_on_page(&mut self, picked: Picked, cx: &mut Context<Self>) {
        if let Some(upstream) = &mut self.upstream {
            upstream.selected = Some(picked);
        }
        self.message_open = false;
        self.merged_pick = None;
        if let Some(Selection::Project(project)) = self.store.read(cx).selection.clone() {
            self.load_selected_merge(&project, cx);
            // A merge opens on the commits it merged.
            if self.selected_merge(&project).is_some() {
                (self.commits_tab, self.review_tab) = (true, false);
            }
        }
        self.show_picked(cx);
        // The window draws the diff beside the list, or the README doesn't need it.
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    /// Select the target commit `delta` rows from the selected one, keeping it in view.
    fn step_upstream(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(upstream) = &self.upstream else {
            return;
        };
        // In the order shown: the README over the list, then the new commits first, under
        // their own heading.
        let readme = (upstream.summary.as_ref()).is_some_and(|s| s.readme.is_some());
        let picks: Vec<_> = (readme.then_some((0, Picked::Readme)).into_iter())
            .chain(
                (self.upstream_rows(upstream, "").iter().enumerate()).filter_map(
                    |(row, r)| match r {
                        Row::Commit(ix) => Some((row, Picked::Commit(upstream.commits[*ix].id))),
                        _ => None,
                    },
                ),
            )
            .collect();
        let at = (picks.iter()).position(|(_, picked)| Some(*picked) == upstream.selected);
        let next = at.map_or(0, |at| at as isize + delta);
        let Some(&(row, picked)) = usize::try_from(next).ok().and_then(|ix| picks.get(ix)) else {
            return;
        };
        self.pick_on_page(picked, cx);
        (self.upstream_scroll).scroll_to_item(row, gpui::ScrollStrategy::Nearest);
    }

    /// Load the images the project's README shows that haven't been asked for yet, each on
    /// its own, decoded off the UI thread so each draws as it arrives.
    fn load_readme_images(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        const MAX_IMAGES: usize = 40;
        let store = self.store.read(cx);
        let (Some(path), Some(target)) = (
            store.path(project),
            (store.workspace(project).and_then(|ws| ws.target.as_ref())).map(|t| t.name.clone()),
        ) else {
            return;
        };
        let Some((_, blocks)) =
            (self.upstream_for(project)).and_then(|u| u.summary.as_ref()?.readme.as_ref())
        else {
            return;
        };
        let sources = markdown::image_sources(blocks);
        for src in sources.into_iter().take(MAX_IMAGES) {
            if !self
                .readme_images_asked
                .insert((project.clone(), src.clone()))
            {
                continue;
            }
            let (path, target, svg) = (path.clone(), target.clone(), cx.svg_renderer());
            let project = project.clone();
            cx.spawn(async move |this, cx| {
                let source = src.clone();
                let loaded = backend::run(move || {
                    let bytes = upstream::readme_image(&path, &target, &source)?;
                    let format = image_format(&bytes)
                        .ok_or_else(|| anyhow::anyhow!("{source} isn't an image"))?;
                    gpui::Image::from_bytes(format, bytes).to_image_data(svg)
                })
                .await;
                this.update(cx, |this, cx| match loaded {
                    Ok(image) => {
                        (this.readme_images.entry(project).or_default()).insert(src, image);
                        cx.notify();
                    }
                    Err(err) => tracing::debug!(?err, "couldn't load a README's image"),
                })
                .ok();
            })
            .detach();
        }
    }

    /// How busy the target has been: a bar for each day, the last week's stronger. Clicking
    /// a day, or shift-clicking a span of them, filters the history to them.
    fn render_activity(
        &self,
        project: &ProjectId,
        summary: &Summary,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        const HEIGHT: f32 = 36.;
        let daily = &summary.daily;
        let total: u32 = daily.iter().sum();
        let week: u32 = daily.iter().rev().take(7).sum();
        let most = daily.iter().copied().max().unwrap_or(0).max(1);
        let picked = (self.activity_days.as_ref())
            .filter(|(p, ..)| p == project)
            .map(|(_, first, last)| (*first, *last));
        // A day's bar by its own last moment, for its date.
        let day_of = |ix: usize| summary.at - (daily.len() - 1 - ix) as i64 * DAY - 1;
        let bars = daily.iter().enumerate().map(|(ix, &n)| {
            let days_ago = daily.len() - 1 - ix;
            let when = match days_ago {
                0 => "in the last day".to_owned(),
                1 => "a day ago".to_owned(),
                days => format!("{days} days ago"),
            };
            let recent = days_ago < 7;
            let inside = picked.is_none_or(|(first, last)| (first..=last).contains(&ix));
            let project = project.clone();
            div()
                .id(("activity-day", ix))
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .cursor_pointer()
                .tooltip(Tooltip::text(format!(
                    "{}, {when}",
                    plural(n as usize, "commit")
                )))
                .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                    this.pick_days(&project, ix, event.modifiers().shift, cx)
                }))
                .child(
                    div()
                        .w_full()
                        .h(px((n as f32 / most as f32 * HEIGHT).max(2.)))
                        .rounded_t(px(1.))
                        .bg(match (n, picked.is_some(), inside, recent) {
                            (0, ..) => theme.border_2,
                            (_, true, true, _) => theme.pop,
                            (_, true, false, _) => theme.pop.opacity(0.2),
                            (_, false, _, true) => theme.pop,
                            (_, false, _, false) => theme.pop.opacity(0.4),
                        }),
                )
        });
        let said = match picked {
            Some((first, last)) => {
                let n: u32 = daily[first..=last].iter().sum();
                let days = match first == last {
                    true => crate::ui::local_date(day_of(first)),
                    false => format!(
                        "{} – {}",
                        crate::ui::local_date(day_of(first)),
                        crate::ui::local_date(day_of(last))
                    ),
                };
                h_flex()
                    .gap(px(8.))
                    .child(faint(
                        format!("{days}: {}", plural(n as usize, "commit")),
                        theme,
                    ))
                    .child(
                        div()
                            .id("activity-clear")
                            .text_color(theme.blue)
                            .cursor_pointer()
                            .child("Clear")
                            .on_click(cx.listener(|this, _, _, cx| this.clear_days(cx))),
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
            .px(px(14.))
            .pt(px(14.))
            .pb(px(6.))
            .gap(px(6.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(section_label("Activity", theme).flex_grow(1.))
                    .child(said),
            )
            .child(
                h_flex()
                    .h(px(HEIGHT))
                    .gap(px(2.))
                    .items_end()
                    .children(bars),
            )
            .child(
                h_flex()
                    .text_size(px(11.))
                    .child(faint(format!("{ACTIVITY_DAYS} days ago"), theme).flex_grow(1.))
                    .child(faint("Today", theme)),
            )
            .into_any_element()
    }

    /// The target's README, beside the list in place of a diff.
    pub(super) fn render_readme(
        &self,
        project: &ProjectId,
        theme: &Theme,
        _cx: &Context<Self>,
    ) -> AnyElement {
        let readme = (self.upstream_for(project)).and_then(|u| u.summary.as_ref()?.readme.as_ref());
        let Some((_, blocks)) = readme else {
            return div().flex_1().into_any_element();
        };
        let doc = v_flex()
            .w_full()
            .max_w(px(760.))
            .px(px(24.))
            .py(px(20.))
            .child(markdown::render(
                "readme",
                blocks,
                self.readme_images
                    .get(project)
                    .unwrap_or(&Default::default()),
                theme,
            ));
        let page = div()
            .id("readme")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.readme_scroll)
            .child(doc);
        scrolled(
            page,
            "readme-scrollbar",
            Scroll::Div(self.readme_scroll.clone()),
            theme,
        )
        .flex_1()
        .h_full()
        .min_w(px(0.))
        .into_any_element()
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
        let mark = ProjectIcon::parse(store.project_icon(project)).render(px(16.), theme);
        let fetch = self.upstream_for(project).map(|u| u.fetch.clone());
        let picked = project.clone();
        // As wide as the other pages' icons, so the titles line up.
        let trigger = div()
            .id("project-icon")
            .flex_none()
            .w(px(16.))
            .h(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .hover(|el| el.bg(theme.hover()))
            .text_size(px(16.))
            .child(mark)
            // No tooltip: it would sit over the picker's search field.
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    // Just under the button, which the click is half its height above.
                    let at = event.position() + gpui::point(px(0.), px(14.));
                    let project = picked.clone();
                    (this.app)
                        .update(cx, |app, cx| app.open_icon_picker(project, at, window, cx))
                        .ok();
                }),
            );
        let meta = meta_row(theme)
            .child(
                div()
                    .min_w(px(0.))
                    // Cut in the middle, keeping the folder's own name at its end.
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .child(crate::ui::short_path(&entry.project.path)),
            )
            .when(entry.project.path.exists(), |el| {
                let busy = store.fetching(project);
                let said = match &fetch {
                    Some(None) => "No remote to fetch from".into(),
                    _ if busy => "Fetching…".into(),
                    Some(Some((Some(at), _))) => format!("Fetched {}", crate::ui::ago(*at)),
                    _ => "Not fetched yet".into(),
                };
                let item = meta_item(Icon::Refresh, theme).flex_none().child(said);
                // When it last fetched, which fetches again on a click, as ⌥⇧F does.
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
        // What's going on in it, in counts.
        let workspace = entry.workspace.as_ref();
        let applied = workspace.map_or(0, |ws| ws.branches().count());
        let unapplied = (self.upstream_for(project)).and_then(|u| u.summary.as_ref()?.unapplied);
        let worktrees = workspace.map_or(0, |ws| ws.worktrees.len());
        let network = store.mesh_checkouts(project);
        let network_branches: usize = network.iter().map(|c| c.branches.len()).sum();
        let counts = meta_row(theme)
            .child(meta_item(Icon::Branch, theme).child(format!("{applied} applied")))
            .children(unapplied.map(|n| {
                meta_item(Icon::Branch, theme)
                    .id("unapplied-count")
                    .child(format!("{n} not applied"))
                    .tooltip(Tooltip::text(
                        "Branches with commits of their own that aren't in your workspace",
                    ))
            }))
            .when(worktrees > 0, |el| {
                el.child(meta_item(Icon::Folder, theme).child(plural(worktrees, "linked worktree")))
            })
            .when(!network.is_empty(), |el| {
                el.child(meta_item(Icon::Globe, theme).child(format!(
                    "{} on other machines, {}",
                    plural(network.len(), "worktree"),
                    match network_branches {
                        1 => "1 branch".to_owned(),
                        n => format!("{n} branches"),
                    }
                )))
            });
        let main = v_flex()
            .gap(px(10.))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(8.))
                    .child(trigger)
                    .child(title_text(entry.project.name.clone())),
            )
            .child(meta)
            .child(counts);
        let target = Selection::Project(project.clone());
        self.page_header(main, target, None, None, theme, cx)
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
            return crate::ui::empty_state(
                Some(Illustration::Papers),
                "Project not found",
                "",
                theme,
                cx,
            );
        };
        let target = entry
            .workspace
            .as_ref()
            .and_then(|ws| ws.target.as_ref())
            .map(|t| t.name.clone());
        let upstream = self.upstream_for(project);
        let fetch_error = upstream.and_then(|u| u.fetch.as_ref()?.1.clone());
        // Where it is and when it fetched head its page; here, what needs doing, if anything.
        // Checked out outside the workspace, with one to go back to.
        let outside = (entry.workspace.as_ref())
            .filter(|ws| !ws.managed && ws.editing.is_none() && ws.target.is_some());
        let needs_doing = fetch_error.is_some()
            || upstream.is_some_and(|u| u.unconnected_forge.is_some())
            || !entry.project.path.exists()
            || entry.error.is_some()
            || outside.is_some();
        // A box of what needs doing, with the button that does it.
        let callout = |ground: gpui::Hsla| {
            v_flex()
                .items_start()
                .mt(px(4.))
                .gap(px(8.))
                .p(px(8.))
                .rounded(px(6.))
                .bg(ground)
        };
        let about = side_header(theme)
            .text_size(px(12.))
            .children(fetch_error.map(|err| {
                div()
                    .text_color(theme.danger)
                    .child(format!("The last fetch failed: {}", error_summary(&err)))
            }))
            .children(upstream.and_then(|u| u.unconnected_forge).map(|forge| {
                callout(theme.hover())
                    .child(div().text_color(theme.text_1).child(format!(
                        "Connect {} to see pull requests and CI here.",
                        forge.name()
                    )))
                    .child(
                        crate::ui::button("connect-forge", "Connect…")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.app
                                    .update(cx, |app, cx| app.open_accounts(window, cx))
                                    .ok();
                            })),
                    )
            }))
            .children(outside.map(|ws| {
                // Until Git finishes, switching to the workspace has to wait.
                let text = match (&ws.checked_out, ws.git_operation) {
                    (_, Some(op)) => format!(
                        "Git is partway through {op}. Finish it in a terminal with \
                                 `{op} --continue`, or stop it with `{op} --abort`."
                    ),
                    (Some(branch), None) => format!(
                        "Working on {branch} directly. Use the GitButler workspace to \
                                 work on several branches at once."
                    ),
                    (None, None) => "Working outside the workspace. Use the GitButler \
                                             workspace to work on several branches at once."
                        .to_owned(),
                };
                let project = project.clone();
                callout(theme.hover())
                    .child(div().text_color(theme.text_1).child(text))
                    .when(ws.git_operation.is_none(), |el| {
                        el.child(
                            crate::ui::button("switch-back", "Switch to the workspace")
                                .small()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.store.update(cx, |store, cx| {
                                        store.run(
                                            &project,
                                            "Switch to the workspace",
                                            Overlay::default(),
                                            |path, _| backend::switch_back_to_workspace(path),
                                            |store, (), cx| {
                                                store.snack(
                                                    "Switched to the GitButler workspace",
                                                    cx,
                                                )
                                            },
                                            cx,
                                        )
                                    })
                                })),
                        )
                    })
            }))
            .when(!entry.project.path.exists(), |el| {
                let remove = project.clone();
                el.child(
                    callout(theme.danger.opacity(0.08))
                        .child(
                            div()
                                .text_color(theme.danger)
                                .child("The project's folder isn't there any more."),
                        )
                        .child(crate::ui::button("forget", "Remove").small().on_click(
                            cx.listener(move |this, _, _, cx| {
                                let project = remove.clone();
                                this.store
                                    .update(cx, |store, cx| store.remove_project(&project, cx));
                            }),
                        )),
                )
            })
            .children(
                entry
                    .error
                    .clone()
                    .filter(|_| entry.project.path.exists())
                    .map(|err| {
                        h_flex()
                            .gap(px(8.))
                            .text_color(theme.danger)
                            .child(icon(Icon::Warning).text_color(theme.danger))
                            .child(div().flex_1().min_w(px(0.)).child(err))
                    }),
            );
        let missing = !entry.project.path.exists();
        let list = match upstream {
            // Its banner says so; reading its history fails with no more to say.
            _ if missing => div().flex_1().into_any_element(),
            None => crate::ui::empty_state(None, "Loading…", "", theme, cx),
            Some(Upstream {
                failed: Some(err), ..
            }) => crate::ui::empty_state(
                Some(Illustration::Papers),
                "Couldn't read the target's history",
                error_summary(err),
                theme,
                cx,
            ),
            // The page lists what it read, whether or not the project is expanded and loaded.
            Some(upstream) if upstream.commits.is_empty() => {
                let project = project.clone();
                // A target the remote renamed or deleted is said so, not as if never picked.
                let lost = (entry.workspace.as_ref()).and_then(|ws| ws.lost_target.clone());
                let (title, detail) = match lost {
                    Some(name) => (
                        format!("{name} is gone"),
                        "The remote no longer has it. Pick the branch your work goes into now",
                    ),
                    None => (
                        "No target branch".to_owned(),
                        "Pick the branch your work goes into, like origin/main",
                    ),
                };
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .px(px(16.))
                    .text_center()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(muted(detail, theme).mb(px(8.)))
                    .child(
                        crate::ui::button("pick-target", "Choose a target branch…").on_click(
                            cx.listener(move |this, _, window, cx| {
                                let project = project.clone();
                                this.app
                                    .update(cx, |app, cx| app.pick_target(project, window, cx))
                                    .ok();
                            }),
                        ),
                    )
                    .into_any_element()
            }
            Some(upstream) => {
                let target = target.unwrap_or_else(|| "the target".into());
                let rows = self.upstream_rows(upstream, &target);
                let list = side_list(
                    "target-commits",
                    rows.len(),
                    &self.upstream_scroll,
                    "target-scrollbar",
                    cx,
                    move |this, ix, theme, window, cx| {
                        let upstream = this.upstream.as_ref()?;
                        Some(match rows.get(ix)? {
                            Row::Heading(heading) => list_heading(heading.clone(), theme),
                            Row::Commit(commit_ix) => {
                                let commit = &upstream.commits[*commit_ix];
                                let id = commit.id;
                                let selected = upstream.selected == Some(Picked::Commit(id));
                                let solid = this.side_focus.is_focused(window);
                                let on = selected && solid;
                                let glyph = match (on, commit.incoming) {
                                    (true, _) => theme.text_inverted,
                                    (false, true) => theme.warn,
                                    (false, false) => theme.text_3,
                                };
                                // One that landed a pull request by the request, as Lite
                                // names it, its number, merge commit and ID on hover.
                                let tip = (commit.review.as_ref()).map(|review| {
                                    let short = id.short();
                                    format!("#{} · {} · {short}", review.number, commit.title)
                                });
                                let mark = match commit.review {
                                    Some(_) => Icon::BranchMerge,
                                    None => Icon::Commit,
                                };
                                side_row(("target-commit", ix), selected, solid, theme)
                                    .child(small_icon(mark).text_color(glyph))
                                    .child(
                                        div()
                                            .id(("target-commit-title", ix))
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .when(!commit.incoming && !on, |el| {
                                                el.text_color(theme.text_2)
                                            })
                                            .child(SharedString::from(label(commit).to_owned()))
                                            .when_some(tip, |el, tip| {
                                                el.tooltip(Tooltip::text(tip))
                                            }),
                                    )
                                    // Who wrote it, as on the Branches page; their name on
                                    // hover.
                                    .child(
                                        div()
                                            .id(("target-commit-author", ix))
                                            .flex_none()
                                            .child(byline_avatar(
                                                &commit.author,
                                                &commit.email,
                                                commit.agent.as_ref(),
                                                14.,
                                                theme,
                                            ))
                                            .tooltip(Tooltip::text(match &commit.agent {
                                                Some(agent) => format!(
                                                    "Committed by {}, written by {}",
                                                    commit.author, agent.name
                                                ),
                                                None => format!("Authored by {}", commit.author),
                                            })),
                                    )
                                    .child(quiet(age_column(Some(commit.time), theme), on, theme))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        window.focus(&this.side_focus, cx);
                                        this.pick_on_page(Picked::Commit(id), cx)
                                    }))
                                    .into_any_element()
                            }
                            // Drawn once it scrolls into view, which reads the next page.
                            Row::More => {
                                let project = upstream.project.clone();
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
                let summary = upstream.summary.as_ref();
                // The README and the activity over the history, which scrolls between them
                // and the team.
                v_flex()
                    .size_full()
                    .track_focus(&self.side_focus)
                    .key_context("SideList")
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_upstream(1, cx)))
                    .on_action(
                        cx.listener(|this, _: &SelectPrevious, _, cx| this.step_upstream(-1, cx)),
                    )
                    .children((summary.and_then(|s| s.readme.as_ref())).map(|(name, _)| {
                        let selected = upstream.selected == Some(Picked::Readme);
                        let solid = self.side_focus.is_focused(window);
                        let on = selected && solid;
                        side_row("readme", selected, solid, theme)
                            .mt(px(6.))
                            .child(small_icon(Icon::TextBlock).text_color(match on {
                                true => theme.text_inverted,
                                false => theme.text_3,
                            }))
                            .child(div().flex_1().min_w(px(0.)).truncate().child(name.clone()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                window.focus(&this.side_focus, cx);
                                this.pick_on_page(Picked::Readme, cx)
                            }))
                    }))
                    .children(summary.map(|s| self.render_activity(project, s, theme, cx)))
                    .child(list.flex_1().min_h(px(0.)))
                    .children(summary.and_then(|s| team(&s.team, theme)))
                    .into_any_element()
            }
        };
        v_flex()
            .size_full()
            .when(needs_doing, |el| el.child(about))
            .child(list)
            .into_any_element()
    }
}

/// Who's been committing to the target, most first, each with a bar of how much.
fn team(team: &[(String, String, u32)], theme: &Theme) -> Option<AnyElement> {
    let most = team.first()?.2.max(1);
    let rows = team.iter().take(TEAM_SHOWN).map(|(name, email, n)| {
        h_flex()
            .gap(px(8.))
            .child(author_avatar(name, email, 20., theme))
            .child(
                v_flex()
                    .flex_grow(1.)
                    .min_w(px(0.))
                    .gap(px(3.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_grow(1.)
                                    .min_w(px(0.))
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(faint(n.to_string(), theme)),
                    )
                    .child(
                        div().h(px(4.)).rounded(px(2.)).bg(theme.border_3).child(
                            div()
                                .h_full()
                                .rounded(px(2.))
                                .w(gpui::relative(*n as f32 / most as f32))
                                .bg(theme.pop),
                        ),
                    ),
            )
    });
    let rest = team.len().saturating_sub(TEAM_SHOWN);
    Some(
        v_flex()
            .flex_none()
            .m(px(12.))
            .p(px(12.))
            .gap(px(10.))
            .border_1()
            .border_color(theme.border_3)
            .rounded(px(8.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(section_label("Team", theme).flex_grow(1.))
                    .child(faint(format!("commits · {ACTIVITY_DAYS} days"), theme)),
            )
            .children(rows)
            .when(rest > 0, |el| {
                el.child(faint(format!("and {}", plural(rest, "other")), theme))
            })
            .into_any_element(),
    )
}

/// What kind of image `bytes` are, by how they start.
fn image_format(bytes: &[u8]) -> Option<gpui::ImageFormat> {
    use gpui::ImageFormat;
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    Some(match bytes {
        [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
        [0xff, 0xd8, 0xff, ..] => ImageFormat::Jpeg,
        [b'G', b'I', b'F', b'8', ..] => ImageFormat::Gif,
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => ImageFormat::Webp,
        [b'B', b'M', ..] => ImageFormat::Bmp,
        _ if text.contains("<svg") => ImageFormat::Svg,
        _ => return None,
    })
}
