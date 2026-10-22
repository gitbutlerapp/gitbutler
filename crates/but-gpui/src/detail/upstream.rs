//! A project's page: where it is, when it last fetched, its target's README, how busy the
//! target has been and who's been committing to it, and its history, the commits the
//! workspace doesn't have yet first, each with the pull request that landed it. Selecting
//! the README shows it; selecting a commit shows its diff.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};

use super::Detail;
use super::panes::{
    Crumb, CrumbTo, PEOPLE_SHOWN, list_heading, measured, meta_item, meta_row, person_row,
    side_header, side_list, title_text, unfinished_banner,
};
use super::pull_request::TABS_HEIGHT;
use crate::app::{Collapse, Expand, SelectNext, SelectPrevious};
use crate::backend::upstream::{self, ACTIVITY_DAYS, Overview, Person, TargetCommit};

const DAY: i64 = 24 * 60 * 60;
use crate::backend::{self, CommitId, DiffSource, Forge};
use crate::state::{BranchStatus, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::emoji::ProjectIcon;
use crate::ui::illustration::Illustration;
use crate::ui::markdown;
use crate::ui::person::{byline, byline_avatar};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, age_column, button, error_summary, faint, fold_button, guide,
    h_flex, icon, list_row, muted, plural, quiet, section_label, shortcut, side_row, small_icon,
    v_flex,
};

/// Where the middle of a merge's chevron sits in its row: the row's padding and half the
/// chevron's step.
const FOLD_MIDDLE: f32 = 8. + 7.;

/// What the page opens on: the README first, as a repository's page does, else the newest
/// commit.
fn opening(readme: bool, commits: &[TargetCommit]) -> Option<Picked> {
    (readme.then_some(Picked::Readme)).or_else(|| commits.first().map(|c| Picked::Commit(c.id)))
}

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
const MIN_DIFF: f32 = 560.;

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
    people: Vec<Person>,
    /// The day the first of `daily` counts, as [`upstream::day_of`] numbers days.
    first_day: i64,
    /// How many branches aren't applied, as the Branches page lists them at first: with
    /// commits of their own, and no merged or closed pull request.
    unapplied: Option<usize>,
}

impl From<Overview> for Summary {
    fn from(overview: Overview) -> Self {
        Summary {
            readme: (overview.readme)
                .map(|(name, text)| (name, markdown::parse_showing_images(&text))),
            daily: overview.daily,
            people: overview.people,
            first_day: overview.first_day,
            unapplied: None,
        }
    }
}

pub(super) struct Upstream {
    project: ProjectId,
    commits: Vec<TargetCommit>,
    more: bool,
    selected: Option<Picked>,
    /// The commit of the selected merge shown in place of everything it brought in.
    merged_pick: Option<CommitId>,
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
                                        .filter(|b| b.own_commits != Some(0))
                                        .filter(|b| {
                                            super::branches::status_keeps(
                                                BranchStatus::NotMerged,
                                                b,
                                            )
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
                let ((commits, mut has_more), fetch, unconnected_forge, summary) = match result {
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
                            merged_pick: None,
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
                let readme = summary.as_ref().is_some_and(|s| s.readme.is_some());
                let selected = was
                    .filter(|picked| match picked {
                        Picked::Readme => readme,
                        Picked::Commit(id) => list.iter().any(|c| c.id == *id),
                    })
                    .or_else(|| opening(readme, &list));
                this.upstream = Some(Upstream {
                    project: project.clone(),
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
                });
                this.refresh_diff(false, cx);
                this.load_readme_images(&project, cx);
                this.load_selected_merge(&project, cx);
                cx.notify();
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
            Picked::Readme => None,
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

    /// Whether the window leaves the diff out, as a pull request and the README take its
    /// place.
    pub fn diff_hidden(&self, cx: &gpui::App) -> bool {
        self.showing_review(cx) || self.showing_readme(cx) || self.showing_branches_overview(cx)
    }

    /// How far right of a list page's list the diff starts: past the files, when there's room
    /// for them; 0 when they're left to the button over the diff.
    pub fn beside_offset(&self, cx: &gpui::App) -> f32 {
        let shows = match self.store.read(cx).selection.as_ref() {
            Some(Selection::Project(project)) => self.showing_target_commit(project, cx),
            Some(Selection::Branches(project)) => self.selected_other_branch(project).is_some(),
            _ => false,
        };
        match shows
            && !self.showing_review(cx)
            && self.beside_width.get() >= BESIDE_FILES + MIN_DIFF
        {
            true => BESIDE_FILES,
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
        if self.open_merges.insert(id) {
            self.store
                .update(cx, |store, cx| store.load_merged(project, id, cx));
        } else {
            self.open_merges.remove(&id);
            let shows = (self.upstream_for(project))
                .is_some_and(|u| u.selected == Some(Picked::Commit(id)) && u.merged_pick.is_some());
            if shows {
                self.pick_merged(None, cx);
            }
        }
        cx.notify();
    }

    /// Show the merge's commit `pick` in its place, or with `None` all it brought in.
    fn pick_merged(&mut self, pick: Option<CommitId>, cx: &mut Context<Self>) {
        if let Some(upstream) = &mut self.upstream {
            upstream.merged_pick = pick;
        }
        self.message_open = false;
        self.show_picked(cx);
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
                .h(px(TABS_HEIGHT))
                .px(px(16.))
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
            .min_w(px(0.))
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
            .min_h(px(0.))
            .items_start()
            .when(offset > 0., |el| {
                el.child(
                    v_flex()
                        .w(px(offset))
                        .flex_none()
                        .h_full()
                        .border_r_1()
                        .border_color(theme.border_3)
                        .child(self.render_diff_file_list(theme, cx)),
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
                described = (markdown::parse(&review.body[..end]).iter())
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
        let title = label(commit).to_owned();
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
            .gap(px(6.))
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .id("target-message-title")
                    .text_size(px(15.))
                    .line_height(px(22.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .when(!open, |el| {
                        el.truncate().tooltip(Tooltip::text(title.clone()))
                    })
                    .child(SharedString::from(title.clone())),
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
                        .text_size(px(13.))
                        .line_height(px(LINE))
                        .text_color(theme.text_2)
                        .whitespace_normal()
                        // A long one scrolls rather than push the diff out of the window.
                        .max_h(px(LINE * 12.))
                        .overflow_y_scroll()
                        .child(SharedString::from(shown)),
                )
            })
            .child(height)
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

    /// The activity's days picked, if any, and which day is today.
    fn picked_days(&self, upstream: &Upstream) -> Option<(&DayPick, i64)> {
        let (project, pick) = self.activity_days.as_ref()?;
        let summary = upstream
            .summary
            .as_ref()
            .filter(|_| *project == upstream.project && !pick.days.is_empty())?;
        Some((pick, summary.first_day + summary.daily.len() as i64 - 1))
    }

    /// The person picked on `upstream`'s page, if any.
    fn picked_person<'a>(&self, upstream: &'a Upstream) -> Option<&'a Person> {
        let (project, email) = self.activity_person.as_ref()?;
        let summary = upstream
            .summary
            .as_ref()
            .filter(|_| *project == upstream.project)?;
        summary.people.iter().find(|person| person.email == *email)
    }

    fn upstream_rows(&self, upstream: &Upstream, target: &str, cx: &gpui::App) -> Vec<Row> {
        let picked = self.picked_days(upstream);
        let person = self.picked_person(upstream);
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
            picked.is_none_or(|(pick, today)| pick.keeps(Some(commit.time), today))
                && person.is_none_or(|person| person.landed.contains(&commit.id))
                && found(commit)
        };
        let mut rows = Vec::new();
        // The navigator lists the new ones on their own, so here they're only marked.
        for ix in (0..upstream.commits.len()).filter(|ix| shown(&upstream.commits[*ix])) {
            if rows.is_empty() {
                rows.push(Row::Heading(format!("On {target}")));
            }
            rows.push(Row::Commit(ix));
            let commit = &upstream.commits[ix];
            // An open merge lists the commits it brought in under it.
            if !(commit.merge && self.open_merges.contains(&commit.id)) {
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
        let first = (picked.and_then(|(pick, _)| pick.days.first()).copied())
            .or_else(|| {
                let bounded = person.is_some() || filtered;
                (upstream.summary.as_ref())
                    .filter(|_| bounded)
                    .map(|s| s.first_day)
            })
            .map(upstream::day_start);
        let past =
            (first.zip(upstream.commits.last())).is_some_and(|(start, oldest)| oldest.time < start);
        if upstream.more && !past {
            rows.push(Row::More);
        } else if rows.is_empty() && picked.is_some() {
            rows.push(Row::Heading(format!("No commits on {target} then")));
        } else if rows.is_empty() && (person.is_some() || filtered) {
            rows.push(Row::Heading(format!("No commits on {target} match")));
        }
        if upstream.more && past && filtered && picked.is_none() && person.is_none() {
            rows.push(Row::Older);
        }
        rows
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
            .gap(px(4.))
            .px(px(16.))
            .text_center()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(muted(detail.into(), theme).mb(px(8.)))
            .child(
                crate::ui::button("pick-target", "Choose a target branch…").on_click(cx.listener(
                    move |this, _, window, cx| {
                        let project = project.clone();
                        (this.app)
                            .update(cx, |app, cx| app.pick_target(project, window, cx))
                            .ok();
                    },
                )),
            )
            .into_any_element()
    }

    /// Pick the activity's days as `gesture` does, which filters the history to them.
    fn day_gesture(&mut self, project: &ProjectId, gesture: DayGesture, cx: &mut Context<Self>) {
        if self
            .activity_days
            .as_ref()
            .is_none_or(|(p, _)| p != project)
        {
            self.activity_days = Some((project.clone(), DayPick::default()));
        }
        if let Some((_, pick)) = &mut self.activity_days
            && pick.apply(gesture)
        {
            self.upstream_scroll
                .scroll_to_item(0, gpui::ScrollStrategy::Top);
            cx.notify();
        }
    }

    fn clear_days(&mut self, cx: &mut Context<Self>) {
        self.activity_days = None;
        cx.notify();
    }

    /// Filter the history to what landed `email`'s commits, or not at all.
    fn filter_landed(
        &mut self,
        project: &ProjectId,
        email: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.activity_person = email.map(|email| (project.clone(), email));
        self.upstream_scroll
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
        cx.notify();
    }

    /// The email of the person whose commits `project`'s history is filtered to, if any.
    fn picked_email(&self, project: &ProjectId) -> Option<&String> {
        (self.activity_person.as_ref())
            .filter(|(p, _)| p == project)
            .map(|(_, email)| email)
    }

    /// Whose commits landed on the target, with how many on the days picked or all the
    /// activity's, most first; the one picked stays, however few they have then.
    fn people_on_days<'a>(
        &self,
        project: &ProjectId,
        summary: &'a Summary,
    ) -> Vec<(&'a Person, usize)> {
        let days = (self.activity_days.as_ref())
            .filter(|(p, pick)| p == project && !pick.days.is_empty())
            .map(|(_, pick)| &pick.days);
        let who = self.picked_email(project);
        let on_days = |person: &Person| -> usize {
            (person.daily.iter().enumerate())
                .filter(|(ix, _)| {
                    days.is_none_or(|d| d.contains(&(summary.first_day + *ix as i64)))
                })
                .map(|(_, n)| *n as usize)
                .sum()
        };
        let mut people: Vec<(&Person, usize)> = (summary.people.iter())
            .map(|person| (person, on_days(person)))
            .filter(|(person, n)| *n > 0 || who == Some(&person.email))
            .collect();
        people.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        people
    }

    /// Back to the top of the history, its newest commit kept, as a filter changes.
    pub(super) fn show_newest_kept(&mut self, _cx: &mut Context<Self>) {
        self.upstream_scroll
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
    }

    /// Whose commits landed on the target, most first, counted on the days picked or all
    /// the activity's. Clicking one filters the history to what landed theirs.
    fn render_people(
        &self,
        project: &ProjectId,
        summary: &Summary,
        theme: &Theme,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if summary.people.is_empty() {
            return None;
        }
        let days = (self.activity_days.as_ref())
            .filter(|(p, pick)| p == project && !pick.days.is_empty())
            .map(|(_, pick)| &pick.days);
        let who = self.picked_email(project);
        let people = self.people_on_days(project, summary);
        let most = people.first().map_or(1, |(_, n)| *n);
        let listed = |ix: usize, person: &Person| ix < PEOPLE_SHOWN || who == Some(&person.email);
        let rest = (people.iter().enumerate())
            .filter(|(ix, (person, _))| !listed(*ix, person))
            .count();
        let solid = self.people_focused(window);
        let rows = (people.into_iter().enumerate())
            .filter(|(ix, (person, _))| listed(*ix, person))
            .map(|(ix, (person, n))| {
                let picked = who == Some(&person.email);
                let (project, email) = (project.clone(), person.email.clone());
                person_row(
                    ("upstream-person", ix),
                    (&person.name, &person.email),
                    (n, most),
                    (picked, solid),
                    theme,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter_landed(&project, Some(email.clone()), cx)
                }))
            })
            .collect();
        let counted = match days {
            None => format!("commits · {ACTIVITY_DAYS} days"),
            Some(_) => "commits · days picked".to_owned(),
        };
        let clear = who.is_some().then(|| {
            let project = project.clone();
            move |this: &mut Self, cx: &mut Context<Self>| this.filter_landed(&project, None, cx)
        });
        let empty = ("No commits landed then", rest);
        Some(self.people_card(counted, rows, empty, clear, theme, cx))
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
                (self.selected_target_commit(project)).map(|commit| commit.id.short())
            }
            None => None,
        };
        let Some(picked) = picked else {
            return;
        };
        // The target goes back to how the page opens, as the project's crumb does.
        let target = (self.store.read(cx).workspace(project)).and_then(|ws| ws.target_name());
        let page = Selection::Project(project.clone()).into();
        crumbs.push((target.unwrap_or("Target").to_owned().into(), Some(page)));
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

    /// Back to what the page opens on, as when it's picked again.
    pub(super) fn pick_opening(&mut self, cx: &mut Context<Self>) {
        let Some(upstream) = &self.upstream else {
            return;
        };
        let readme = (upstream.summary.as_ref()).is_some_and(|s| s.readme.is_some());
        if let Some(picked) = opening(readme, &upstream.commits) {
            self.pick_on_page(picked, cx)
        }
    }

    pub(super) fn pick_target_commit(&mut self, id: CommitId, cx: &mut Context<Self>) {
        self.pick_on_page(Picked::Commit(id), cx)
    }

    fn pick_on_page(&mut self, picked: Picked, cx: &mut Context<Self>) {
        if let Some(upstream) = &mut self.upstream {
            upstream.selected = Some(picked);
            upstream.merged_pick = None;
        }
        self.message_open = false;
        if let Some(Selection::Project(project)) = self.store.read(cx).selection.clone() {
            self.load_selected_merge(&project, cx);
        }
        self.show_picked(cx);
    }

    /// Select the target commit `delta` rows from the selected one, keeping it in view.
    /// → in the list: open the picked merge, or step into its commits when it's open, as
    /// on the Branches page. ← folds it, or steps from one of its commits back up to it.
    fn fold_upstream(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(upstream) = &self.upstream else {
            return;
        };
        let Some(Picked::Commit(id)) = upstream.selected else {
            return;
        };
        let merge = (upstream.commits.iter()).any(|commit| commit.id == id && commit.merge);
        let project = upstream.project.clone();
        match (
            open,
            upstream.merged_pick.is_some(),
            self.open_merges.contains(&id),
        ) {
            (true, false, false) if merge => self.toggle_merge(&project, id, cx),
            (true, false, true) => self.step_upstream(1, cx),
            (false, true, _) => self.pick_merged(None, cx),
            (false, false, true) => self.toggle_merge(&project, id, cx),
            _ => {}
        }
    }

    fn step_upstream(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(upstream) = &self.upstream else {
            return;
        };
        // In the order shown: the README over the list, then the target's commits.
        let readme = (upstream.summary.as_ref()).is_some_and(|s| s.readme.is_some());
        let merged = |ix: usize, m: usize| {
            let merge = upstream.commits[ix].id;
            let merged = self.store.read(cx).merged.get(&merge)?.as_ref()?;
            Some((Picked::Commit(merge), Some(merged.0.get(m)?.id)))
        };
        // A merge's commit by the merge and it.
        let picks: Vec<_> = (readme.then_some((0, (Picked::Readme, None))).into_iter())
            .chain(
                (self.upstream_rows(upstream, "", cx).iter().enumerate()).filter_map(|(row, r)| {
                    match r {
                        Row::Commit(ix) => {
                            Some((row, (Picked::Commit(upstream.commits[*ix].id), None)))
                        }
                        Row::Merged(ix, m) => Some((row, merged(*ix, *m)?)),
                        _ => None,
                    }
                }),
            )
            .collect();
        let current = (upstream.selected, upstream.merged_pick);
        let at = (picks.iter()).position(|(_, (picked, pick))| (Some(*picked), *pick) == current);
        let next = at.map_or(0, |at| at as isize + delta);
        let Some(&(row, (picked, pick))) = usize::try_from(next).ok().and_then(|ix| picks.get(ix))
        else {
            return;
        };
        self.pick_on_page(picked, cx);
        if pick.is_some() {
            self.pick_merged(pick, cx);
        }
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
                    let (format, bytes) = match format {
                        // An SVG's own images would be read from any path it names.
                        gpui::ImageFormat::Svg => {
                            let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
                            anyhow::ensure!(
                                !text.contains("<image") && !text.contains(":image"),
                                "{source} draws other images, which aren't shown"
                            );
                            (format, bytes)
                        }
                        // Its first frame alone, as that's all that's drawn: every frame of
                        // a long one would take a lot of memory.
                        gpui::ImageFormat::Gif => {
                            let frame = image::load_from_memory_with_format(
                                &bytes,
                                image::ImageFormat::Gif,
                            )?;
                            let mut png = std::io::Cursor::new(Vec::new());
                            frame.write_to(&mut png, image::ImageFormat::Png)?;
                            (gpui::ImageFormat::Png, png.into_inner())
                        }
                        _ => (format, bytes),
                    };
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
    /// a day, shift-clicking another or dragging across them filters the history to them.
    fn render_activity(
        &self,
        project: &ProjectId,
        summary: &Summary,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let none = BTreeSet::new();
        let picked = (self.activity_days.as_ref())
            .filter(|(p, _)| p == project)
            .map_or(&none, |(_, pick)| &pick.days);
        let project = project.clone();
        activity_strip(
            &summary.daily,
            summary.first_day,
            picked,
            "commit",
            theme,
            cx,
            move |this, gesture, cx| this.day_gesture(&project, gesture, cx),
            |this, cx| this.clear_days(cx),
        )
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
        // What's going on in it, in counts, after where it is on the same line, which gives
        // way first.
        let workspace = entry.workspace.as_ref();
        let applied = workspace.map_or(0, |ws| ws.branches().count());
        let unapplied = (self.upstream_for(project)).and_then(|u| u.summary.as_ref()?.unapplied);
        let worktrees = workspace.map_or(0, |ws| ws.worktrees.len());
        let network = store.mesh_checkouts(project);
        let network_branches: usize = network.iter().map(|c| c.branches.len()).sum();
        let meta = meta
            .child(
                meta_item(Icon::Branch, theme)
                    .flex_none()
                    .child(format!("{applied} applied")),
            )
            .children(unapplied.map(|n| {
                meta_item(Icon::Branch, theme)
                    .flex_none()
                    .id("unapplied-count")
                    .child(format!("{n} not applied"))
                    .tooltip(Tooltip::text(
                        "Branches in progress that aren't applied: with commits of their own, \
                         and no merged or closed pull request",
                    ))
            }))
            .when(worktrees > 0, |el| {
                el.child(
                    meta_item(Icon::Folder, theme)
                        .flex_none()
                        .child(plural(worktrees, "linked worktree")),
                )
            })
            .when(!network.is_empty(), |el| {
                el.child(meta_item(Icon::Globe, theme).flex_none().child(format!(
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
            .child(meta);
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
        let missing = !entry.project.path.exists();
        let fetch_error = upstream.and_then(|u| u.fetch.as_ref()?.1.clone());
        // Said in or outside the workspace, as the uncommitted changes say it.
        let unfinished = (entry.workspace.as_ref())
            .and_then(|ws| ws.git_operation)
            .map(|op| unfinished_banner(op, theme));
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
        // Where it is and when it fetched head its page; here, what needs doing, if anything.
        let needs_doing: Vec<AnyElement> = [
            fetch_error.map(|err| {
                div()
                    .text_color(theme.danger)
                    .child(format!("The last fetch failed: {}", error_summary(&err)))
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
                                this.app
                                    .update(cx, |app, cx| app.open_accounts(window, cx))
                                    .ok();
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
            entry.error.clone().filter(|_| !missing).map(|err| {
                h_flex()
                    .gap(px(8.))
                    .text_color(theme.danger)
                    .child(icon(Icon::Warning).text_color(theme.danger))
                    .child(div().flex_1().min_w(px(0.)).child(err))
                    .into_any_element()
            }),
        ]
        .into_iter()
        .flatten()
        .collect();
        let list = match upstream {
            // Its banner says so; reading its history fails with no more to say.
            _ if missing => div().flex_1().into_any_element(),
            None => crate::ui::empty_state(None, "Loading…", "", theme, cx),
            // Picking the target again is the way out of what it most often is, as a target that
            // more than one local branch tracks.
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
                let (title, detail) = match lost {
                    Some(name) => (
                        format!("{name} is gone"),
                        "The remote no longer has it. Pick the branch your work goes into now",
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
                            // Under a merge's commits' titles.
                            Row::Note(note) => h_flex()
                                .h(px(crate::ui::ROW_HEIGHT))
                                .mx(px(6.))
                                .pl(px(8. + 14. + 8. + 12. + 8.))
                                .child(guide(FOLD_MIDDLE, theme))
                                .child(faint(*note, theme))
                                .into_any_element(),
                            Row::Merged(merge_ix, merged_ix) => {
                                let merge = upstream.commits[*merge_ix].id;
                                let (merged, _) =
                                    this.store.read(cx).merged.get(&merge)?.as_ref()?;
                                let commit = merged.get(*merged_ix)?;
                                let id = commit.id;
                                let selected = upstream.selected == Some(Picked::Commit(merge))
                                    && upstream.merged_pick == Some(id);
                                let solid = this.side_focus.is_focused(window);
                                let on = selected && solid;
                                side_row(("merged-commit", ix), selected, solid, theme)
                                    // As a branch's commits are under it: its glyph and title
                                    // under the merge's, a line down from the merge's chevron.
                                    .child(guide(FOLD_MIDDLE, theme))
                                    .child(div().flex_none().w(px(14.)))
                                    .child(small_icon(Icon::Commit).text_color(match on {
                                        true => theme.text_inverted,
                                        false => theme.text_3,
                                    }))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .when(!on, |el| el.text_color(theme.text_2))
                                            .child(SharedString::from(commit.title.clone())),
                                    )
                                    .child(commit_author(
                                        ("merged-commit-author", ix),
                                        commit,
                                        on,
                                        theme,
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.pick_on_page(Picked::Commit(merge), cx);
                                        this.pick_merged(Some(id), cx);
                                    }))
                                    .into_any_element()
                            }
                            Row::Commit(commit_ix) => {
                                let commit = &upstream.commits[*commit_ix];
                                let id = commit.id;
                                let selected = upstream.selected == Some(Picked::Commit(id))
                                    && upstream.merged_pick.is_none();
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
                                // A merge's chevron folds the commits it brought in, by hand;
                                // the others keep its room, so the titles line up.
                                let fold = match commit.merge {
                                    false => div().flex_none().w(px(14.)).into_any_element(),
                                    true => {
                                        let project = upstream.project.clone();
                                        let open = this.open_merges.contains(&id);
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
                                side_row(("target-commit", ix), selected, solid, theme)
                                    .child(fold)
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
                                    // Who wrote it, as on the Branches page.
                                    .child(commit_author(
                                        ("target-commit-author", ix),
                                        commit,
                                        on,
                                        theme,
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.pick_on_page(Picked::Commit(id), cx)
                                    }))
                                    .into_any_element()
                            }
                            // Drawn once it scrolls into view, which reads the next page.
                            Row::Older => {
                                let project = upstream.project.clone();
                                list_row("target-older", false, theme)
                                    .text_color(theme.text_2)
                                    .child("Show older")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_upstream(&project, true, false, cx)
                                    }))
                                    .into_any_element()
                            }
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
                // and the people.
                v_flex()
                    .size_full()
                    .track_focus(&self.side_focus)
                    .key_context("SideList")
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_upstream(1, cx)))
                    .on_action(
                        cx.listener(|this, _: &SelectPrevious, _, cx| this.step_upstream(-1, cx)),
                    )
                    .on_action(cx.listener(|this, _: &Expand, _, cx| this.fold_upstream(true, cx)))
                    .on_action(
                        cx.listener(|this, _: &Collapse, _, cx| this.fold_upstream(false, cx)),
                    )
                    .child(self.filter_field(&self.target_filter, window, cx))
                    .children((summary.and_then(|s| s.readme.as_ref())).map(|(name, _)| {
                        let selected = upstream.selected == Some(Picked::Readme);
                        let solid = self.side_focus.is_focused(window);
                        let on = selected && solid;
                        side_row("readme", selected, solid, theme)
                            .child(small_icon(Icon::TextBlock).text_color(match on {
                                true => theme.text_inverted,
                                false => theme.text_3,
                            }))
                            .child(div().flex_1().min_w(px(0.)).truncate().child(name.clone()))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.pick_on_page(Picked::Readme, cx)),
                            )
                    }))
                    .children(summary.map(|s| self.render_activity(project, s, theme, cx)))
                    .child(list.flex_1().min_h(px(0.)))
                    .children(
                        summary.and_then(|s| self.render_people(project, s, theme, window, cx)),
                    )
                    .into_any_element()
            }
        };
        v_flex()
            .size_full()
            .children(unfinished)
            .when(!needs_doing.is_empty(), |el| {
                el.child(side_header(theme).text_size(px(12.)).children(needs_doing))
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
        .gap(px(8.))
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

/// A day's noon, clear of a clock change, to date it by.
pub(super) fn noon(day: i64) -> i64 {
    upstream::day_start(day) + DAY / 2
}

/// The days picked, said short: one by its date, a span by its ends, days apart by how
/// many; `None` while none is.
pub(super) fn days_said(picked: &BTreeSet<i64>) -> Option<String> {
    let date = |day: i64| crate::ui::local_date(noon(day));
    let (&first, &last) = picked.first().zip(picked.last())?;
    Some(match picked.len() {
        1 => date(first),
        len if (last - first + 1) as usize == len => format!("{} – {}", date(first), date(last)),
        len => format!("{len} days"),
    })
}

/// How busy something has been: a bar for each day of `daily`, from `first_day`, the last
/// week's stronger, counting `unit`s. Pressing a day, shift-pressing another or dragging
/// across them hands `gesture` what the pointer did, for a [`DayPick`]; Clear, beside the days
/// picked, calls `clear`.
#[allow(clippy::too_many_arguments)]
pub(super) fn activity_strip(
    daily: &[u32],
    first_day: i64,
    picked: &BTreeSet<i64>,
    unit: &str,
    theme: &Theme,
    cx: &Context<Detail>,
    gesture: impl Fn(&mut Detail, DayGesture, &mut Context<Detail>) + Clone + 'static,
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
            let (press, drag) = (gesture.clone(), gesture.clone());
            div()
                .id(("activity-day", ix))
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .tooltip(Tooltip::text(crate::ui::local_weekday(noon(day))))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        let (shift, toggle) = (event.modifiers.shift, event.modifiers.secondary());
                        press(this, DayGesture::Press { day, shift, toggle }, cx)
                    }),
                )
                .on_mouse_move(
                    cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                        if event.pressed_button == Some(gpui::MouseButton::Left) {
                            drag(this, DayGesture::Drag(day), cx)
                        }
                    }),
                )
                .child(
                    div()
                        .w_full()
                        .h(px((n as f32 / most as f32 * HEIGHT).max(2.)))
                        .rounded_t(px(1.))
                        .bg(match (n, !picked.is_empty(), inside, recent) {
                            (_, true, true, _) => theme.pop,
                            (0, ..) => theme.border_2,
                            (_, true, false, _) => theme.pop.opacity(0.2),
                            (_, false, _, true) => theme.pop,
                            (_, false, _, false) => theme.pop.opacity(0.4),
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
                .gap(px(6.))
                .child(faint(
                    format!("{days}: {}", plural(n as usize, unit)),
                    theme,
                ))
                .child(
                    button("activity-clear", "Clear")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(move |this, _, _, cx| clear(this, cx))),
                )
                .into_any_element()
        }
        None => faint(
            format!("{}, {week} in the last week", plural(total as usize, unit)),
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
        .child({
            let release = move |this: &mut Detail,
                                _: &gpui::MouseUpEvent,
                                _: &mut gpui::Window,
                                cx: &mut Context<Detail>| {
                gesture(this, DayGesture::Release, cx)
            };
            h_flex()
                .h(px(HEIGHT))
                .gap(px(2.))
                .items_end()
                .on_mouse_up(gpui::MouseButton::Left, cx.listener(release.clone()))
                .on_mouse_up_out(gpui::MouseButton::Left, cx.listener(release))
                .children(bars)
        })
        .child(
            h_flex()
                .text_size(px(11.))
                .child(faint(format!("{ACTIVITY_DAYS} days ago"), theme).flex_grow(1.))
                .child(faint("Today", theme)),
        )
        .into_any_element()
}

/// What the pointer did on an activity strip's day: pressed it, with shift or ⌘ (`toggle`)
/// held or not; moved onto it while pressed; or let go.
#[derive(Debug, Clone, Copy)]
pub(super) enum DayGesture {
    Press { day: i64, shift: bool, toggle: bool },
    Drag(i64),
    Release,
}

/// Days picked off an activity strip, as `upstream::day_of` numbers them, none for every day:
/// one by a click, another added or taken away by ⌘-click, and a span by shift-clicking from
/// the one clicked before or by dragging across them.
#[derive(Debug, Clone, Default)]
pub(super) struct DayPick {
    pub(super) days: BTreeSet<i64>,
    /// The day a shift-click picks the span from: the one clicked before.
    anchor: Option<i64>,
    /// While a press is held: the day the span picked runs from to the one under the pointer.
    drag_from: Option<i64>,
}

impl DayPick {
    /// Apply `gesture`, as a list picks its rows: a click picks a day alone, or lets go of
    /// the one day picked; ⌘ adds or takes away one; shift picks the days from the one
    /// clicked before; holding the press, the pointer moving across the days picks those it
    /// spans from where it was pressed. Whether the days picked changed.
    pub(super) fn apply(&mut self, gesture: DayGesture) -> bool {
        let before = self.days.clone();
        let span = |a: i64, b: i64| (a.min(b)..=a.max(b)).collect::<BTreeSet<_>>();
        match gesture {
            DayGesture::Press {
                day, shift: true, ..
            } if self.anchor.is_some() => {
                let from = self.anchor.unwrap_or(day);
                self.days = span(from, day);
                self.drag_from = Some(from);
            }
            DayGesture::Press {
                day, toggle: true, ..
            } => {
                if !self.days.remove(&day) {
                    self.days.insert(day);
                }
                self.anchor = Some(day);
                self.drag_from = None;
            }
            DayGesture::Press { day, .. } => {
                // Clicking the one day picked lets go of it, with nothing to drag from then.
                let clears = self.days.len() == 1 && self.days.contains(&day);
                self.drag_from = (!clears).then_some(day);
                self.days = self.drag_from.into_iter().collect();
                self.anchor = Some(day);
            }
            DayGesture::Drag(day) => {
                if let Some(from) = self.drag_from {
                    self.days = span(from, day);
                }
            }
            DayGesture::Release => self.drag_from = None,
        }
        self.days != before
    }

    /// Whether something last changed at `time`, if it's known, falls on a day picked, with
    /// every day picked while none is; one after `today` counts as today's.
    pub(super) fn keeps(&self, time: Option<i64>, today: i64) -> bool {
        self.days.is_empty()
            || time.is_some_and(|time| self.days.contains(&upstream::day_of(time).min(today)))
    }
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

#[cfg(test)]
mod tests {
    use super::{DayGesture, DayPick};

    #[test]
    fn days_are_picked_by_click_cmd_click_shift_click_and_drag() {
        use DayGesture::{Drag, Release};
        let press = |day| DayGesture::Press {
            day,
            shift: false,
            toggle: false,
        };
        let shift = |day| DayGesture::Press {
            day,
            shift: true,
            toggle: false,
        };
        let cmd = |day| DayGesture::Press {
            day,
            shift: false,
            toggle: true,
        };
        let mut pick = DayPick::default();
        let mut days = |gestures: &[DayGesture]| {
            for gesture in gestures {
                pick.apply(*gesture);
            }
            pick.days.iter().copied().collect::<Vec<_>>()
        };
        assert_eq!(days(&[press(10), Release]), [10], "a click picks a day");
        assert_eq!(
            days(&[shift(13), Release]),
            [10, 11, 12, 13],
            "shift picks the days between"
        );
        assert_eq!(
            days(&[cmd(20), Release, cmd(11), Release]),
            [10, 12, 13, 20],
            "⌘ adds or takes away one"
        );
        assert_eq!(
            days(&[press(20), Drag(18), Drag(17), Release]),
            [17, 18, 19, 20],
            "dragging picks the days crossed"
        );
        assert_eq!(days(&[Drag(5)]), [17, 18, 19, 20], "only while pressed");
        assert_eq!(
            days(&[press(12), Release, press(12), Drag(12), Release]),
            [] as [i64; 0],
            "clicking the one day picked again lets go, though the pointer moves a little"
        );
    }
}
