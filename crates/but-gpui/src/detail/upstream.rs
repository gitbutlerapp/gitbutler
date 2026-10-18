//! A project's page: where it is, when it last fetched, and its target's history, the
//! commits the workspace doesn't have yet first, each with the pull request that landed
//! it. Selecting one shows its diff.

use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};

use super::Detail;
use super::panes::{list_heading, side_header, side_list};
use crate::backend::upstream::{self, TargetCommit};
use crate::backend::{self, CommitId, DiffSource, Forge};
use crate::state::{Overlay, ProjectId};
use crate::theme::Theme;
use crate::ui::{
    Icon, error_summary, faint, h_flex, icon, list_row, muted, plural, relative_time, small_icon,
    v_flex,
};

/// How many target commits a page holds.
const PAGE: u32 = 50;

/// A reload that changes nothing the list shows still refreshes it this often, for the
/// fetch time.
const STALE_AFTER: Duration = Duration::from_secs(60);

pub(super) struct Upstream {
    project: ProjectId,
    commits: Vec<TargetCommit>,
    more: bool,
    selected: Option<CommitId>,
    /// When the project last fetched successfully, and why the last fetch failed; `None`
    /// when it has no remote to fetch from.
    fetch: Option<(Option<i64>, Option<String>)>,
    /// The forge the project is on, when no account for it is connected.
    unconnected_forge: Option<Forge>,
    /// What the list was loaded for, and when.
    seen: (u64, Instant),
    /// Why reading the history failed, shown in place of the list.
    failed: Option<String>,
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
        let project = project.clone();
        let forge = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.forge);
        self._upstream_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                let page = upstream::target_commits(&path, after, PAGE)?;
                let connected = forge.is_none_or(|forge| {
                    (backend::accounts::accounts().iter()).any(|account| account.forge == forge)
                });
                Ok((
                    page,
                    upstream::fetch_status(&path)?,
                    forge.filter(|_| !connected),
                ))
            })
            .await;
            this.update(cx, |this, cx| {
                let ((commits, has_more), fetch, unconnected_forge) = match result {
                    Ok(loaded) => loaded,
                    // Paging further back keeps what's listed.
                    Err(err) if after.is_some() => {
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't read more of the history", format!("{err:#}"), cx)
                        });
                        return;
                    }
                    Err(err) => {
                        this.upstream = Some(Upstream {
                            project,
                            commits: Vec::new(),
                            more: false,
                            selected: None,
                            fetch: Some((None, None)),
                            unconnected_forge: None,
                            seen: (seen, Instant::now()),
                            failed: Some(format!("{err:#}")),
                        });
                        this.refresh_diff(false, cx);
                        return cx.notify();
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
                    fetch,
                    unconnected_forge,
                    seen: (seen, Instant::now()),
                    failed: None,
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

    /// Whether the project's page found it has no remote to fetch from.
    pub(super) fn no_remote(&self, project: &ProjectId) -> bool {
        self.upstream_for(project)
            .is_some_and(|u| u.fetch.is_none())
    }

    pub(super) fn upstream_source(&self, project: &ProjectId) -> Option<DiffSource> {
        Some(DiffSource::Commit(self.upstream_for(project)?.selected?))
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

    fn select_target_commit(&mut self, id: CommitId, cx: &mut Context<Self>) {
        if let Some(upstream) = &mut self.upstream {
            upstream.selected = Some(id);
        }
        self.show_picked(cx);
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
        let fetch = upstream.map(|u| u.fetch.clone());
        let fetch_error = upstream.and_then(|u| u.fetch.as_ref()?.1.clone());
        let about = side_header(theme)
            .text_size(px(12.))
            .child(
                div()
                    .font_family(theme.mono_font.clone())
                    .text_color(theme.text_2)
                    // Cut in the middle, keeping the folder's own name at its end.
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .child(crate::ui::short_path(&entry.project.path)),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_color(theme.text_2)
                    .child(small_icon(Icon::Refresh).text_color(theme.text_2))
                    .child(match fetch {
                        Some(None) => "No remote to fetch from".into(),
                        Some(Some((Some(at), _))) => match relative_time(at).as_str() {
                            "now" => "Fetched just now".into(),
                            ago => format!("Fetched {ago} ago"),
                        },
                        _ => "Not fetched yet".into(),
                    }),
            )
            .children(fetch_error.map(|err| {
                div()
                    .text_color(theme.danger)
                    .child(format!("The last fetch failed: {}", error_summary(&err)))
            }))
            .children(upstream.and_then(|u| u.unconnected_forge).map(|forge| {
                v_flex()
                    .items_start()
                    .mt(px(4.))
                    .gap(px(8.))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(theme.hover())
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
            .children(
                (entry.workspace.as_ref())
                    .filter(|ws| !ws.managed && ws.editing.is_none() && ws.target.is_some())
                    .map(|ws| {
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
                        v_flex()
                            .items_start()
                            .mt(px(4.))
                            .gap(px(8.))
                            .p(px(8.))
                            .rounded(px(6.))
                            .bg(theme.hover())
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
                                                    |path, _| {
                                                        backend::switch_back_to_workspace(path)
                                                    },
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
                    }),
            )
            .when(!entry.project.path.exists(), |el| {
                let remove = project.clone();
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
                            h_flex().child(crate::ui::button("forget", "Remove").small().on_click(
                                cx.listener(move |this, _, _, cx| {
                                    let project = remove.clone();
                                    this.store
                                        .update(cx, |store, cx| store.remove_project(&project, cx));
                                }),
                            )),
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
                            .child(icon(Icon::Warning).text_color(theme.danger))
                            .child(div().flex_1().min_w(px(0.)).child(err))
                    }),
            );
        let list = match upstream {
            None => crate::ui::empty_state("Loading…", "", theme),
            Some(Upstream {
                failed: Some(err), ..
            }) => crate::ui::empty_state(
                "Couldn't read the target's history",
                error_summary(err),
                theme,
            ),
            // The page lists what it read, whether or not the project is expanded and loaded.
            Some(upstream) if upstream.commits.is_empty() => {
                let project = project.clone();
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
                        muted(
                            "Pick the branch your work goes into, like origin/main",
                            theme,
                        )
                        .mb(px(8.)),
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
                    )
                    .into_any_element()
            }
            Some(upstream) => {
                let target = target.unwrap_or_else(|| "the target".into());
                let rows = self.upstream_rows(upstream, &target);
                side_list(
                    "target-commits",
                    rows.len(),
                    &self.upstream_scroll,
                    "target-scrollbar",
                    cx,
                    move |this, ix, theme, cx| {
                        let upstream = this.upstream.as_ref()?;
                        Some(match rows.get(ix)? {
                            Row::Heading(heading) => list_heading(heading.clone(), theme),
                            Row::Commit(commit_ix) => {
                                let commit = &upstream.commits[*commit_ix];
                                let id = commit.id;
                                let selected = upstream.selected == Some(id);
                                let glyph = if commit.incoming {
                                    theme.warn
                                } else {
                                    theme.text_3
                                };
                                list_row(("target-commit", ix), selected, theme)
                                    .child(small_icon(Icon::Commit).text_color(glyph))
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
                                    .child(faint(relative_time(commit.time), theme).flex_none())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_target_commit(id, cx)
                                    }))
                                    .into_any_element()
                            }
                            Row::More => {
                                let project = upstream.project.clone();
                                list_row("target-more", false, theme)
                                    .text_color(theme.text_2)
                                    .child("Show older")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_upstream(&project, true, false, cx)
                                    }))
                                    .into_any_element()
                            }
                        })
                    },
                )
                .into_any_element()
            }
        };
        v_flex()
            .size_full()
            .child(about)
            .child(list)
            .into_any_element()
    }
}
