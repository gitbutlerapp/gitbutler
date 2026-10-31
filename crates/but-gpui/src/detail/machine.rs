//! What the detail pane shows of a checkout on another machine, picked in the navigator: its
//! uncommitted files, whose diffs stay on that machine and open on but.dev, or a branch's or a
//! commit's diff, once fetched from the copy the mesh keeps, one at a time.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{AnyElement, App, Context, Div, SharedString, div, prelude::*};

use super::Detail;
use super::panes::{
    Crumb, Lane, and_more, grouped, meta_item, overview_heading, overview_row, overview_sections,
    overview_split, overview_tiles, overview_title, worktree_lanes,
};
use crate::backend::mesh::{self, Checkout};
use crate::backend::{self, CommitId, DiffSource};
use crate::state::{MeshBranch, MeshPick, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::file_icons::file_icon;
use crate::ui::{
    ButtonStyle, Icon, age_column, button, commit_title, dp, empty_state, faint, h_flex, just_now,
    plural, small_icon, v_flex,
};

/// How many of a checkout's uncommitted files its view lists; the rest open on but.dev.
const LISTED: usize = 50;

/// How many of the newest commits, and of the files changed most, Network worktrees' page
/// lists.
const SHOWN: usize = 8;

/// A branch's copy here.
pub(super) enum Fetch {
    /// Waiting its turn, or under way.
    Running,
    /// Fetched, and what it fetched.
    Done(CommitId),
    Failed(SharedString),
}

/// What's fetched of other machines' branches, and what's still to fetch.
#[derive(Default)]
pub(super) struct MeshFetches {
    /// Each branch's fetch, with the tip that machine reported when it started: a new one
    /// fetches it again.
    fetches: HashMap<MeshBranch, (Option<String>, Fetch)>,
    /// Branches to fetch, in turn, each with its project's path, the mesh's id for it and the
    /// tip that machine reported.
    queue: Vec<(MeshBranch, std::path::PathBuf, String, Option<String>)>,
    running: bool,
    _fetch: Option<gpui::Task<()>>,
}

impl Detail {
    /// Fetch the branches of `project`'s checkouts elsewhere that have no copy here as their
    /// machine reported them, the selected one first; with `retry`, as on selecting a page of
    /// them, those that failed too.
    pub(super) fn load_machines(
        &mut self,
        project: &ProjectId,
        retry: bool,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(path) = store.path(project) else {
            return;
        };
        let picked = match &store.selection {
            Some(Selection::Mesh(p, id, pick)) if p == project => {
                pick.branch().map(|name| (id.clone(), name.to_owned()))
            }
            _ => None,
        };
        let wanted: Vec<_> = (store.mesh_checkouts(project).iter())
            .flat_map(|checkout| {
                let id: Arc<str> = checkout.id.as_str().into();
                (checkout.branches.iter()).map(move |branch| (id.clone(), branch))
            })
            .filter_map(|(id, branch)| {
                let mesh_id = branch.mesh_id.clone()?;
                let key = (project.clone(), id, branch.name.clone());
                let current = (self.mesh.fetches.get(&key)).is_some_and(|(tip, fetch)| {
                    let failed = retry && matches!(fetch, Fetch::Failed(_));
                    (*tip == branch.tip && !failed) || matches!(fetch, Fetch::Running)
                });
                (!current).then(|| (key, path.clone(), mesh_id, branch.tip.clone()))
            })
            .collect();
        for (key, path, mesh_id, tip) in wanted {
            // A copy fetched already shows until the newer one replaces it.
            let fetch = match self.mesh.fetches.remove(&key) {
                Some((_, done @ Fetch::Done(_))) => done,
                _ => Fetch::Running,
            };
            self.mesh.fetches.insert(key.clone(), (tip.clone(), fetch));
            self.mesh.queue.retain(|(queued, ..)| *queued != key);
            self.mesh.queue.push((key, path, mesh_id, tip));
        }
        // The selected branch goes first, queued now or before.
        let first = (self.mesh.queue.iter())
            .position(|(key, ..)| picked.as_ref() == Some(&(key.1.clone(), key.2.clone())));
        if let Some(first) = first {
            let picked = self.mesh.queue.remove(first);
            self.mesh.queue.insert(0, picked);
        }
        self.fetch_next(cx);
    }

    /// Start the next fetch in the queue, unless one runs. The selected branch's, once
    /// fetched, shows its diff.
    fn fetch_next(&mut self, cx: &mut Context<Self>) {
        if self.mesh.running || self.mesh.queue.is_empty() {
            return;
        }
        // The project shown first, as it may have changed since these were queued.
        let shown = match self.selection(cx) {
            Some(Selection::Mesh(project, ..)) => Some(project),
            None | Some(_) => None,
        };
        let next = (self.mesh.queue.iter())
            .position(|(key, ..)| shown.as_ref() == Some(&key.0))
            .unwrap_or(0);
        let (key, path, mesh_id, tip) = self.mesh.queue.remove(next);
        self.mesh.running = true;
        self.mesh._fetch = Some(cx.spawn(async move |this, cx| {
            let fetched = backend::run(move || mesh::fetch(&path, &mesh_id, tip.as_deref())).await;
            this.update(cx, |this, cx| {
                this.mesh.running = false;
                if let Some((_, fetch)) = this.mesh.fetches.get_mut(&key) {
                    *fetch = match fetched {
                        Ok(tip) => Fetch::Done(tip),
                        Err(err) => Fetch::Failed(format!("{err:#}").into()),
                    };
                }
                if matches!(this.selection(cx), Some(Selection::Mesh(p, ..)) if p == key.0) {
                    this.refresh_diff(false, cx);
                }
                this.fetch_next(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    fn branch_fetch(&self, project: &ProjectId, id: &Arc<str>, name: &str) -> Option<&Fetch> {
        let key = (project.clone(), id.clone(), name.to_owned());
        self.mesh.fetches.get(&key).map(|(_, fetch)| fetch)
    }

    /// The diff of `pick` in `project`'s checkout `id` elsewhere, once its branch is here.
    pub(super) fn machine_source(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        pick: &MeshPick,
    ) -> Option<DiffSource> {
        let Fetch::Done(tip) = self.branch_fetch(project, id, pick.branch()?)? else {
            return None;
        };
        Some(match pick {
            MeshPick::Commit(_, commit) => DiffSource::Commit(*commit),
            MeshPick::Checkout | MeshPick::Branch(_) => DiffSource::Branch {
                tip: *tip,
                ahead: None,
            },
        })
    }

    /// What the diff pane shows for `pick` without a diff: that a checkout's diffs stay on its
    /// machine, or why a branch has none yet.
    pub(super) fn machine_placeholder(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        pick: &MeshPick,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return empty_state(None, "", "", theme, cx);
        };
        let Some(name) = pick.branch() else {
            let line = match checkout.uncommitted.is_empty() {
                true => format!("Nothing uncommitted on {}", checkout.machine),
                false => format!("Their diffs are on {}", checkout.machine),
            };
            let server = store.mesh.connection().map(|c| c.server.as_str());
            let url = server.map(|server| mesh::link(server, &checkout.id, None));
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap(dp(12.))
                .child(faint(line, theme).text_size(dp(13.)))
                .children(url.filter(|_| !checkout.uncommitted.is_empty()).map(|url| {
                    button("checkout-on-but-dev", "Open on but.dev")
                        .small()
                        .icon(Icon::Link)
                        .style(ButtonStyle::Ghost)
                        .on_click(move |_, _, cx| cx.open_url(&url))
                }))
                .into_any_element();
        };
        let branch = checkout.branches.iter().find(|b| b.name == name);
        let waiting = match self.branch_fetch(project, id, name) {
            _ if branch.is_some_and(|b| b.mesh_id.is_none()) => {
                format!("The mesh keeps no copy of {name}")
            }
            Some(Fetch::Failed(err)) => format!("Couldn't fetch {name}: {err}"),
            Some(Fetch::Running) | None => format!("Fetching {name} from {}…", checkout.machine),
            Some(Fetch::Done(_)) => String::new(),
        };
        empty_state(None, "", waiting, theme, cx)
    }

    /// Beside the diff: a checkout's uncommitted files, or a branch's or commit's files.
    pub(super) fn render_mesh_side(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        pick: &MeshPick,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        match (pick, store.mesh_checkout(project, id)) {
            (MeshPick::Checkout, Some(checkout)) => uncommitted_list(checkout, theme, cx),
            _ => self.render_diff_file_list(theme, cx),
        }
    }

    /// Network worktrees' page, summing up the work on the user's other machines as the
    /// Branches page sums up the branches under way: how much of it there is and how many of
    /// its checkouts are online; a lane for each checkout of its commits a day; then the
    /// newest commits and the files with the most lines changed there. A row opens what it is.
    pub(super) fn render_machines_overview(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let checkouts = self.store.read(cx).mesh_checkouts(project);
        let commits = || {
            (checkouts.iter()).flat_map(|checkout| {
                (checkout.branches.iter()).flat_map(move |branch| {
                    branch.commits.iter().map(move |c| (checkout, branch, c))
                })
            })
        };
        let machines: HashSet<&str> = checkouts.iter().map(|c| c.machine.as_str()).collect();
        let unpushed = commits().filter(|(.., commit)| !commit.pushed).count();
        let files: usize = checkouts.iter().map(|c| c.uncommitted.len()).sum();
        let tiles = [
            (
                "Worktrees",
                grouped(checkouts.len()),
                "Checkouts with work this machine doesn't have",
            ),
            (
                "Machines",
                grouped(machines.len()),
                "The machines they're on",
            ),
            (
                "Unpushed",
                grouped(unpushed),
                "Commits only those machines have",
            ),
            (
                "Uncommitted",
                grouped(files),
                "Files changed there and not committed",
            ),
        ];
        let count = |keep: &dyn Fn(&Checkout) -> bool| checkouts.iter().filter(|c| keep(c)).count();
        let parts = [
            (theme.safe, theme.safe, "active now", count(&|c| active(c))),
            (
                theme.text_2,
                theme.text_2,
                "online",
                count(&|c| c.online && !active(c)),
            ),
            (
                theme.text_3,
                theme.border_1,
                "offline",
                count(&|c| !c.online),
            ),
        ]
        .map(|(color, fill, said, n)| (Icon::Computer, color, fill, said.to_owned(), n));

        // Who did the most lately first.
        let mut by_activity: Vec<&Checkout> = checkouts.iter().collect();
        by_activity.sort_by_key(|c| std::cmp::Reverse(c.last_activity));
        let lanes = by_activity.into_iter().map(|c| Lane {
            at: Selection::Mesh(project.clone(), c.id.as_str().into(), MeshPick::Checkout),
            glyph: Icon::Computer,
            name: c.label(),
            tag: match (active(c), c.online) {
                (true, _) => Some(("active".to_owned(), theme.safe)),
                (false, false) => Some(("offline".to_owned(), theme.text_3)),
                (false, true) => None,
            },
            branches: (c.branches.iter().map(|b| b.name.as_str()))
                .collect::<Vec<_>>()
                .join(", "),
            commits: (c.branches.iter().flat_map(|b| &b.commits))
                .filter_map(|commit| commit.created_at)
                .collect(),
            changed: c.last_activity,
        });
        let worktrees = worktree_lanes(lanes, checkouts.len(), theme, cx);

        let mut recent: Vec<_> = commits().collect();
        recent.sort_by_key(|(.., commit)| std::cmp::Reverse(commit.created_at));
        let commit_rows = (recent.iter().take(SHOWN).enumerate()).map(|(ix, (c, b, commit))| {
            let (glyph, color) = match commit.pushed {
                true => (Icon::CommitFill, theme.text_2),
                false => (Icon::Commit, theme.blue),
            };
            let pick = MeshPick::Commit(b.name.clone(), commit.id);
            let at = Selection::Mesh(project.clone(), c.id.as_str().into(), pick);
            overview_row(("mesh-commit", ix), at, cx)
                .child(small_icon(glyph).text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(commit_title(&commit.title)),
                )
                .child(
                    faint(format!("{} · {}", b.name, c.machine), theme)
                        .min_w_0()
                        .truncate(),
                )
                .child(age_column(commit.created_at, theme))
        });
        let commits_section = v_flex()
            .child(overview_heading("Recent commits", div(), theme))
            .when(recent.is_empty(), |el| {
                el.child(nothing("No commits there", theme))
            })
            .children(commit_rows)
            .children(and_more(recent.len().saturating_sub(SHOWN), theme));

        let mut changed: Vec<(&Checkout, &mesh::File)> = (checkouts.iter())
            .flat_map(|c| c.uncommitted.iter().map(move |file| (c, file)))
            .collect();
        changed.sort_by_key(|(_, f)| std::cmp::Reverse(f.additions + f.deletions));
        let file_rows = (changed.iter().take(SHOWN).enumerate()).map(|(ix, (c, file))| {
            let at = Selection::Mesh(project.clone(), c.id.as_str().into(), MeshPick::Checkout);
            overview_row(("mesh-file", ix), at, cx)
                .child(file_icon(&file.path, cx))
                .child(div().flex_1().min_w_0().truncate().child(file.path.clone()))
                .child(faint(c.label(), theme).min_w_0().truncate())
                .child(diff::stats(
                    file.additions as u32,
                    file.deletions as u32,
                    theme,
                ))
        });
        let files_section = v_flex()
            .child(overview_heading("Uncommitted", div(), theme))
            .when(changed.is_empty(), |el| {
                el.child(nothing("Nothing uncommitted there", theme))
            })
            .children(file_rows)
            .children(and_more(changed.len().saturating_sub(SHOWN), theme));

        v_flex()
            .id("machines-overview")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .px(dp(14.))
                    .py(dp(16.))
                    .gap(dp(6.))
                    .child(overview_title(
                        "Network worktrees",
                        "From your mesh".to_owned(),
                        theme,
                    ))
                    .child(overview_tiles(tiles, theme))
                    .children(overview_split(parts, theme))
                    // The lanes want the width a cell a day needs; the lists sit side by side
                    // under them where they fit.
                    .child(overview_sections([
                        (Some(worktrees.into_any_element()), 880.),
                        (Some(commits_section.into_any_element()), 340.),
                        (Some(files_section.into_any_element()), 300.),
                    ])),
            )
            .into_any_element()
    }

    /// What the project's header counts of its checkouts on other machines, while it has any.
    pub(super) fn mesh_meta(&self, project: &ProjectId, theme: &Theme, cx: &App) -> Option<Div> {
        let checkouts = self.store.read(cx).mesh_checkouts(project);
        let branches = checkouts.iter().map(|c| c.branches.len()).sum();
        (!checkouts.is_empty()).then(|| {
            meta_item(Icon::Globe, theme).flex_none().child(format!(
                "{} on other machines, {}",
                plural(checkouts.len(), "worktree"),
                plural(branches, "branch"),
            ))
        })
    }
}

/// The crumbs after the project's for `pick` in its checkout `id` on another machine: the
/// checkout, the branch and the commit, each but the last going to what it names.
pub(super) fn mesh_crumbs(
    store: &Store,
    project: &ProjectId,
    id: &Arc<str>,
    pick: &MeshPick,
    crumbs: &mut Vec<Crumb>,
) {
    let Some(checkout) = store.mesh_checkout(project, id) else {
        return;
    };
    let at = |pick| Some(Selection::Mesh(project.clone(), id.clone(), pick).into());
    let up = |on: bool, pick| if on { at(pick) } else { None };
    let below = pick.branch().is_some();
    crumbs.push((checkout.label().into(), up(below, MeshPick::Checkout)));
    if let Some(name) = pick.branch() {
        let on = matches!(pick, MeshPick::Commit(..));
        crumbs.push((
            name.to_owned().into(),
            up(on, MeshPick::Branch(name.into())),
        ));
    }
    if let MeshPick::Commit(_, commit) = pick {
        crumbs.push((commit.short().into(), None));
    }
}

/// A checkout's uncommitted files, as that machine reported them.
fn uncommitted_list(checkout: &Checkout, theme: &Theme, cx: &App) -> AnyElement {
    let files = &checkout.uncommitted;
    let title = match files.len() {
        0 => "Nothing uncommitted".to_owned(),
        n => plural(n, "uncommitted file"),
    };
    let more = files.len().saturating_sub(LISTED);
    let listed = files.iter().take(LISTED).map(|file| {
        h_flex()
            .gap(dp(8.))
            .child(file_icon(&file.path, cx))
            .child(div().flex_1().min_w_0().truncate().child(file.path.clone()))
            .child(diff::stats(
                file.additions as u32,
                file.deletions as u32,
                theme,
            ))
    });
    v_flex()
        .size_full()
        .p(dp(12.))
        .gap(dp(8.))
        .child(
            v_flex()
                .gap(dp(2.))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                .child(faint(checkout.path.clone(), theme)),
        )
        .child(v_flex().gap(dp(4.)).children(listed))
        .when(more > 0, |el| {
            el.child(faint(format!("and {more} more"), theme))
        })
        .into_any_element()
}

/// Whether `checkout`'s machine is online and something there changed in the last two
/// minutes.
fn active(checkout: &Checkout) -> bool {
    checkout.online && checkout.last_activity.is_some_and(just_now)
}

/// What a section of Network worktrees' page says in place of rows.
fn nothing(said: &'static str, theme: &Theme) -> Div {
    faint(said, theme).px(dp(14.)).py(dp(4.))
}
