//! A project's page: where it is, when it last fetched, and its target's history, the
//! commits the workspace doesn't have yet first, each with the pull request that landed
//! it. Selecting one shows its diff.

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px, uniform_list};

use super::Detail;
use crate::backend::upstream::{self, TargetCommit};
use crate::backend::{self, CommitId, DiffSource};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::{
    Icon, ROW_HEIGHT, h_flex, icon, list_row, plural, relative_time, small_icon, v_flex,
};

/// How many target commits a page holds.
const PAGE: u32 = 50;

/// A reload that changes nothing the list shows still refreshes it this often, for the
/// fetch time.
const STALE_AFTER: Duration = Duration::from_secs(60);

pub(super) struct Upstream {
    pub project: ProjectId,
    pub commits: Vec<TargetCommit>,
    pub more: bool,
    pub selected: Option<CommitId>,
    /// When the project last fetched successfully, and why the last fetch failed.
    pub fetched: Option<i64>,
    /// The forge the project is on, when no account for it is connected.
    pub unconnected_forge: Option<String>,
    pub fetch_error: Option<String>,
    /// What the list was loaded for, and when.
    seen: (u64, Instant),
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
        let seen = self.upstream_fingerprint(project, cx);
        let current = self.upstream.as_ref().filter(|u| u.project == *project);
        if !force
            && !more
            && current.is_some_and(|u| u.seen.0 == seen && u.seen.1.elapsed() < STALE_AFTER)
        {
            return;
        }
        let after = current
            .filter(|_| more)
            .and_then(|u| u.commits.last())
            .map(|commit| commit.id);
        let project = project.clone();
        let forge = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.forge.clone());
        self._upstream_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                let page = upstream::target_commits(&path, after, PAGE)?;
                let connected = forge.as_ref().is_none_or(|forge| {
                    backend::accounts::accounts()
                        .iter()
                        .any(|account| account.forge.name() == forge)
                });
                Ok((
                    page,
                    upstream::fetch_status(&path)?,
                    forge.filter(|_| !connected),
                ))
            })
            .await;
            this.update(cx, |this, cx| {
                let ((commits, has_more), (fetched, fetch_error), unconnected_forge) = match result
                {
                    Ok(loaded) => loaded,
                    Err(err) => {
                        this.store.update(cx, |store, cx| {
                            store.error(
                                "Couldn't read the target's history",
                                format!("{err:#}"),
                                cx,
                            )
                        });
                        return;
                    }
                };
                let previous = this.upstream.take().filter(|u| u.project == project);
                let (mut list, selected) = match previous {
                    Some(previous) if after.is_some() => (previous.commits, previous.selected),
                    Some(previous) => (Vec::new(), previous.selected),
                    None => (Vec::new(), None),
                };
                // The first page stops where the workspace forked off; paging goes on below.
                let more = has_more || (after.is_none() && !commits.is_empty());
                list.extend(commits);
                let selected = selected
                    .filter(|id| list.iter().any(|c| c.id == *id))
                    .or_else(|| list.first().map(|c| c.id));
                this.upstream = Some(Upstream {
                    project,
                    commits: list,
                    more,
                    selected,
                    fetched,
                    fetch_error,
                    unconnected_forge,
                    seen: (seen, Instant::now()),
                });
                this.refresh_diff(false, cx);
                cx.notify();
            })
            .ok();
        });
    }

    fn upstream_fingerprint(&self, project: &ProjectId, cx: &Context<Self>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if let Some(workspace) = self.store.read(cx).workspace(project) {
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

    pub(super) fn upstream_for(&self, project: &ProjectId) -> Option<&Upstream> {
        self.upstream.as_ref().filter(|u| u.project == *project)
    }

    pub(super) fn upstream_source(&self, project: &ProjectId) -> Option<DiffSource> {
        Some(DiffSource::Commit(self.upstream_for(project)?.selected?))
    }

    fn upstream_rows(&self, upstream: &Upstream, target: &str) -> Vec<Row> {
        let incoming: Vec<usize> = (0..upstream.commits.len())
            .filter(|ix| upstream.commits[*ix].incoming)
            .collect();
        let mut rows = Vec::new();
        if !incoming.is_empty() {
            rows.push(Row::Heading(format!(
                "New on {target} · {}",
                plural(incoming.len(), "commit")
            )));
            rows.extend(incoming.into_iter().map(Row::Commit));
        }
        let rest: Vec<usize> = (0..upstream.commits.len())
            .filter(|ix| !upstream.commits[*ix].incoming)
            .collect();
        if !rest.is_empty() {
            rows.push(Row::Heading("In your workspace".into()));
            rows.extend(rest.into_iter().map(Row::Commit));
        }
        if upstream.more {
            rows.push(Row::More);
        }
        rows
    }

    fn select_target_commit(&mut self, id: CommitId, cx: &mut Context<Self>) {
        if let Some(upstream) = &mut self.upstream {
            upstream.selected = Some(id);
        }
        self.refresh_diff(true, cx);
        cx.notify();
    }

    pub(super) fn render_upstream_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return crate::ui::empty_state("Project not found", "", theme);
        };
        let target = entry
            .workspace
            .as_ref()
            .and_then(|ws| ws.target.as_ref())
            .map(|t| t.name.clone());
        let upstream = self.upstream_for(project);
        let fetched = upstream.and_then(|u| u.fetched);
        let fetch_error = upstream.and_then(|u| u.fetch_error.clone());
        let about = v_flex()
            .flex_none()
            .p(px(16.))
            .gap(px(6.))
            .border_b_1()
            .border_color(theme.border_3)
            .text_size(px(12.))
            .child(
                div()
                    .font_family(theme.mono_font.clone())
                    .text_color(theme.text_2)
                    .truncate()
                    .child(entry.project.path.display().to_string()),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_color(theme.text_2)
                    .child(small_icon(Icon::Refresh))
                    .child(match fetched {
                        Some(at) => format!("Fetched {} ago", relative_time(at)),
                        None => "Not fetched yet".into(),
                    }),
            )
            .children(fetch_error.map(|err| {
                div()
                    .text_color(theme.danger)
                    .child(format!("The last fetch failed: {err}"))
            }))
            .children(
                upstream
                    .and_then(|u| u.unconnected_forge.clone())
                    .map(|forge| {
                        h_flex()
                            .mt(px(4.))
                            .gap(px(8.))
                            .p(px(8.))
                            .rounded(px(6.))
                            .bg(theme.fill_gray.opacity(0.06))
                            .child(div().flex_1().text_color(theme.text_1).child(format!(
                                "Connect {forge} to see pull requests and CI here."
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
                    }),
            )
            .when(!entry.project.path.exists(), |el| {
                let (locate, remove) = (project.clone(), project.clone());
                el.child(
                    v_flex()
                        .mt(px(4.))
                        .gap(px(8.))
                        .p(px(8.))
                        .rounded(px(6.))
                        .bg(theme.danger.opacity(0.08))
                        .child(
                            div()
                                .text_color(theme.danger)
                                .child("The project's folder isn't there any more."),
                        )
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .child(crate::ui::button("locate", "Locate…").small().on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        let project = locate.clone();
                                        this.app
                                            .update(cx, |app, cx| app.locate_project(project, cx))
                                            .ok();
                                    }),
                                ))
                                .child(
                                    crate::ui::button("forget", "Remove")
                                        .small()
                                        .style(crate::ui::ButtonStyle::Ghost)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let project = remove.clone();
                                            this.store.update(cx, |store, cx| {
                                                store.remove_project(&project, cx)
                                            });
                                        })),
                                ),
                        ),
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
                            .child(icon(Icon::Warning))
                            .child(div().flex_1().child(err))
                    }),
            );
        let Some(upstream) = upstream else {
            return v_flex()
                .size_full()
                .child(about)
                .child(crate::ui::empty_state("Loading…", "", theme))
                .into_any_element();
        };
        // The page lists what it read, whether or not the project is expanded and loaded.
        if upstream.commits.is_empty() {
            let project = project.clone();
            return v_flex()
                .size_full()
                .child(about)
                .child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("No target branch"),
                        )
                        .child(
                            div()
                                .mb(px(8.))
                                .text_size(px(12.))
                                .text_color(theme.text_2)
                                .child("Pick the branch your work goes into, like origin/main"),
                        )
                        .child(
                            crate::ui::button("pick-target", "Choose a target branch…").on_click(
                                cx.listener(move |this, _, window, cx| {
                                    let project = project.clone();
                                    this.app
                                        .update(cx, |app, cx| app.pick_target(project, window, cx))
                                        .ok();
                                }),
                            ),
                        ),
                )
                .into_any_element();
        }
        let target = target.unwrap_or_else(|| "the target".into());
        let count = self.upstream_rows(upstream, &target).len();
        let list = uniform_list(
            "target-commits",
            count,
            cx.processor(move |this, range: Range<usize>, _, cx| {
                let theme = Theme::get(cx).clone();
                let Some(upstream) = this.upstream.as_ref() else {
                    return Vec::new();
                };
                let rows = this.upstream_rows(upstream, &target);
                range
                    .filter_map(|ix| rows.get(ix).map(|row| (ix, row)))
                    .map(|(ix, row)| match row {
                        Row::Heading(heading) => div()
                            .w_full()
                            .h(px(ROW_HEIGHT))
                            .flex()
                            .items_end()
                            .px(px(14.))
                            .pb(px(4.))
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(SharedString::from(heading.clone()))
                            .into_any_element(),
                        Row::Commit(commit_ix) => {
                            let commit = &upstream.commits[*commit_ix];
                            let id = commit.id;
                            div()
                                .w_full()
                                .child(
                                    list_row(
                                        ("target-commit", ix),
                                        upstream.selected == Some(id),
                                        &theme,
                                    )
                                    .child(small_icon(Icon::Commit).text_color(
                                        if commit.incoming {
                                            theme.warn
                                        } else {
                                            theme.text_3
                                        },
                                    ))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .when(!commit.incoming, |el| {
                                                el.text_color(theme.text_2)
                                            })
                                            .child(SharedString::from(commit.title.clone())),
                                    )
                                    .when_some(commit.review.as_ref(), |el, (number, _)| {
                                        el.child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(theme.text_2)
                                                .child(SharedString::from(number.clone())),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_size(px(11.))
                                            .text_color(theme.text_3)
                                            .child(relative_time(commit.time)),
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| this.select_target_commit(id, cx),
                                    )),
                                )
                                .into_any_element()
                        }
                        Row::More => {
                            let project = upstream.project.clone();
                            div()
                                .w_full()
                                .child(
                                    list_row("target-more", false, &theme)
                                        .text_color(theme.text_2)
                                        .child("Show older")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.load_upstream(&project, true, true, cx)
                                        })),
                                )
                                .into_any_element()
                        }
                    })
                    .collect()
            }),
        )
        .track_scroll(&self.upstream_scroll)
        .size_full();
        v_flex()
            .size_full()
            .child(about)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(list)
                    .children(scrollbar(
                        "target-scrollbar",
                        Scroll::Uniform(self.upstream_scroll.clone()),
                        theme,
                    )),
            )
            .into_any_element()
    }
}
