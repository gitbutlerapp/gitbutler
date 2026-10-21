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
use crate::app::{SelectNext, SelectPrevious};
use crate::backend::upstream::{self, ACTIVITY_DAYS, Overview, TargetCommit};
use crate::backend::{self, CommitId, DiffSource, Forge, Review};
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
}

impl From<Overview> for Summary {
    fn from(overview: Overview) -> Self {
        Summary {
            readme: (overview.readme)
                .map(|(name, text)| (name, markdown::parse_showing_images(&text))),
            daily: overview.daily,
            team: overview.team,
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
                        .map(Summary::from)
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
            Picked::Commit(id) => Some(DiffSource::Commit(id)),
            Picked::Readme => None,
        }
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

    fn upstream_rows(&self, upstream: &Upstream, target: &str) -> Vec<Row> {
        let (incoming, rest): (Vec<usize>, Vec<usize>) =
            (0..upstream.commits.len()).partition(|ix| upstream.commits[*ix].incoming);
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
        if upstream.more {
            rows.push(Row::More);
        }
        rows
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
    }

    fn pick_on_page(&mut self, picked: Picked, cx: &mut Context<Self>) {
        if let Some(upstream) = &mut self.upstream {
            upstream.selected = Some(picked);
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
                    .children(summary.map(|s| activity(&s.daily, theme)))
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

/// How busy the target has been: a bar for each day, the last week's stronger.
fn activity(daily: &[u32], theme: &Theme) -> AnyElement {
    const HEIGHT: f32 = 36.;
    let total: u32 = daily.iter().sum();
    let week: u32 = daily.iter().rev().take(7).sum();
    let most = daily.iter().copied().max().unwrap_or(0).max(1);
    let bars = daily.iter().enumerate().map(|(ix, &n)| {
        let days_ago = daily.len() - 1 - ix;
        let when = match days_ago {
            0 => "in the last day".to_owned(),
            1 => "a day ago".to_owned(),
            days => format!("{days} days ago"),
        };
        let recent = days_ago < 7;
        div()
            .id(("activity-day", ix))
            .flex_1()
            .h_full()
            .flex()
            .items_end()
            .tooltip(Tooltip::text(format!(
                "{}, {when}",
                plural(n as usize, "commit")
            )))
            .child(
                div()
                    .w_full()
                    .h(px((n as f32 / most as f32 * HEIGHT).max(2.)))
                    .rounded_t(px(1.))
                    .bg(match (n, recent) {
                        (0, _) => theme.border_2,
                        (_, true) => theme.pop,
                        (_, false) => theme.pop.opacity(0.4),
                    }),
            )
    });
    v_flex()
        .px(px(14.))
        .pt(px(14.))
        .pb(px(6.))
        .gap(px(6.))
        .child(
            h_flex()
                .gap(px(8.))
                .child(section_label("Activity", theme).flex_grow(1.))
                .child(faint(
                    format!(
                        "{}, {week} in the last week",
                        plural(total as usize, "commit")
                    ),
                    theme,
                )),
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
