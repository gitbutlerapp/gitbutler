//! A project's Other machines page: its checkouts on the user's other machines, as the mesh
//! reports them, in a tree as the navigator's: each machine, its checkouts, their branches and
//! their commits, a line each, with the rest on hover. A checkout shows its uncommitted files,
//! whose diffs stay on that machine and open on but.dev; a branch or a commit shows its diff,
//! once fetched from the copy the mesh keeps, one at a time.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, Hsla, MouseButton, MouseDownEvent,
    SharedString, Stateful, UniformListScrollHandle, div, prelude::*, px,
};

use super::Detail;
use super::panes::{quiet, side_list, side_row};
use crate::app::{Collapse, Expand, SelectNext, SelectPrevious};
use crate::backend::mesh::{self, BranchStatus, Checkout};
use crate::backend::{self, CommitId, DiffSource};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::{
    ButtonStyle, Icon, Tooltip, age_column, button, commit_title, dot, empty_state, faint, h_flex,
    just_now, meta_count, plural, relative_time, small_icon, v_flex,
};

/// How many of a checkout's uncommitted files its view lists; the rest open on but.dev.
const LISTED: usize = 50;

/// A branch of a checkout on another machine: the project, the checkout by the mesh's id, and
/// the branch by name.
pub(super) type MeshBranch = (ProjectId, Arc<str>, String);

/// What's picked on the page: a checkout, which shows its uncommitted files, or one of its
/// branches or commits, which show their diff.
#[derive(Clone, PartialEq)]
pub(super) enum Pick {
    Checkout,
    Branch(String),
    /// A commit, by the branch it's listed under and its id.
    Commit(String, CommitId),
}

/// A branch's copy here.
pub(super) enum Fetch {
    /// Waiting its turn, or under way.
    Running,
    /// Fetched, and what it fetched.
    Done(CommitId),
    Failed(SharedString),
}

/// The Other machines page: what it fetched and is fetching, what's picked and what's folded.
#[derive(Default)]
pub(super) struct MachinesPage {
    /// Each branch's fetch, with the tip that machine reported when it started: a new one
    /// fetches it again.
    fetches: HashMap<MeshBranch, (Option<String>, Fetch)>,
    /// Branches to fetch, in turn, each with its project's path, the mesh's id for it and the
    /// tip that machine reported.
    queue: Vec<(MeshBranch, std::path::PathBuf, String, Option<String>)>,
    running: bool,
    _fetch: Option<gpui::Task<()>>,
    /// The pick on each project's page, with the checkout it's in, kept while another page
    /// shows.
    picks: HashMap<ProjectId, (Arc<str>, Pick)>,
    /// Machines folded to their row, by name.
    folded_machines: HashSet<(ProjectId, String)>,
    /// Checkouts folded to their row.
    folded_checkouts: HashSet<(ProjectId, Arc<str>)>,
    /// Branches whose commits show under them, folded until opened, as in the navigator.
    open_branches: HashSet<MeshBranch>,
    scroll: UniformListScrollHandle,
}

/// A row of the page's list, by index into the project's checkouts, a checkout's branches and
/// a branch's commits.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    /// A machine, by the first of its checkouts listed. It only folds.
    Machine(usize),
    Checkout(usize),
    Branch(usize, usize),
    Commit(usize, usize, usize),
}

impl Detail {
    /// Fetch the branches of `project`'s checkouts elsewhere that have no copy here as their
    /// machine reported them, the picked one first, and pick the first branch, or else the
    /// first checkout, if nothing is.
    pub(super) fn load_machines(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(path) = store.path(project) else {
            return;
        };
        let checkouts = store.mesh_checkouts(project);
        let still_there = |id: &str, pick: &Pick| {
            checkouts.iter().any(|c| {
                c.id == id
                    && match pick {
                        Pick::Checkout => true,
                        Pick::Branch(name) | Pick::Commit(name, _) => {
                            c.branches.iter().any(|b| b.name == *name)
                        }
                    }
            })
        };
        let picks = &mut self.mesh.picks;
        if !(picks.get(project)).is_some_and(|(id, pick)| still_there(id, pick)) {
            picks.remove(project);
            let first_branch = (checkouts.iter()).find_map(|c| {
                let name = c.branches.first()?.name.clone();
                Some((c.id.as_str().into(), Pick::Branch(name)))
            });
            let first = first_branch
                .or_else(|| (checkouts.first()).map(|c| (c.id.as_str().into(), Pick::Checkout)));
            if let Some(first) = first {
                picks.insert(project.clone(), first);
            }
        }
        let picked = match picks.get(project) {
            Some((id, Pick::Branch(name) | Pick::Commit(name, _))) => {
                Some((id.clone(), name.clone()))
            }
            Some((_, Pick::Checkout)) | None => None,
        };
        let mut wanted: Vec<_> = (checkouts.iter())
            .flat_map(|checkout| {
                let id: Arc<str> = checkout.id.as_str().into();
                (checkout.branches.iter()).map(move |branch| (id.clone(), branch))
            })
            .filter_map(|(id, branch)| {
                let mesh_id = branch.mesh_id.clone()?;
                let key = (project.clone(), id, branch.name.clone());
                let current = (self.mesh.fetches.get(&key)).is_some_and(|(tip, fetch)| {
                    *tip == branch.tip || matches!(fetch, Fetch::Running)
                });
                (!current).then(|| (key, path.clone(), mesh_id, branch.tip.clone()))
            })
            .collect();
        wanted.sort_by_key(|(key, ..)| picked.as_ref() != Some(&(key.1.clone(), key.2.clone())));
        for (key, path, mesh_id, tip) in wanted {
            self.mesh
                .fetches
                .insert(key.clone(), (tip.clone(), Fetch::Running));
            self.mesh.queue.push((key, path, mesh_id, tip));
        }
        self.fetch_next(cx);
    }

    /// Start the next fetch in the queue, unless one runs. The picked branch's, once fetched,
    /// shows its diff.
    fn fetch_next(&mut self, cx: &mut Context<Self>) {
        if self.mesh.running || self.mesh.queue.is_empty() {
            return;
        }
        // The page shown first, as it may have changed since these were queued.
        let shown = match self.selection(cx) {
            Some(Selection::Machines(project)) => Some(project),
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
                if matches!(this.selection(cx), Some(Selection::Machines(p)) if p == key.0) {
                    this.refresh_diff(false, cx);
                }
                this.fetch_next(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// The rows `project`'s page lists: each machine, in the order the mesh first names it,
    /// then, unless folded, its checkouts, their branches and the commits of open branches.
    fn machine_rows(&self, project: &ProjectId, checkouts: &[Checkout]) -> Vec<Row> {
        let page = &self.mesh;
        let mut rows = Vec::new();
        for (first, checkout) in checkouts.iter().enumerate() {
            let machine = &checkout.machine;
            if checkouts[..first].iter().any(|c| c.machine == *machine) {
                continue;
            }
            rows.push(Row::Machine(first));
            if (page.folded_machines).contains(&(project.clone(), machine.clone())) {
                continue;
            }
            let on_it = checkouts.iter().enumerate().skip(first);
            for (c, checkout) in on_it.filter(|(_, c)| c.machine == *machine) {
                rows.push(Row::Checkout(c));
                let id: Arc<str> = checkout.id.as_str().into();
                if (page.folded_checkouts).contains(&(project.clone(), id.clone())) {
                    continue;
                }
                for (b, branch) in checkout.branches.iter().enumerate() {
                    rows.push(Row::Branch(c, b));
                    let key = (project.clone(), id.clone(), branch.name.clone());
                    if page.open_branches.contains(&key) {
                        rows.extend((0..branch.commits.len()).map(|k| Row::Commit(c, b, k)));
                    }
                }
            }
        }
        rows
    }

    fn pick(&mut self, project: &ProjectId, id: &Arc<str>, pick: Pick, cx: &mut Context<Self>) {
        let picked = (id.clone(), pick);
        if self.mesh.picks.get(project) != Some(&picked) {
            self.mesh.picks.insert(project.clone(), picked);
            self.show_picked(cx);
        }
    }

    /// What picking `row` picks; a machine's row picks nothing.
    fn row_pick(checkouts: &[Checkout], row: Row) -> Option<(Arc<str>, Pick)> {
        match row {
            Row::Machine(_) => None,
            Row::Checkout(c) => Some((checkouts.get(c)?.id.as_str().into(), Pick::Checkout)),
            Row::Branch(c, b) => {
                let checkout = checkouts.get(c)?;
                let name = checkout.branches.get(b)?.name.clone();
                Some((checkout.id.as_str().into(), Pick::Branch(name)))
            }
            Row::Commit(c, b, k) => {
                let checkout = checkouts.get(c)?;
                let branch = checkout.branches.get(b)?;
                let id = gix::ObjectId::from_hex(branch.commits.get(k)?.id.as_bytes()).ok()?;
                let pick = Pick::Commit(branch.name.clone(), CommitId(id));
                Some((checkout.id.as_str().into(), pick))
            }
        }
    }

    /// ↓ or ↑ (`delta`) in the list: the next or previous row that picks something, kept in
    /// view.
    fn step_machines(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(Selection::Machines(project)) = self.selection(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let checkouts = store.mesh_checkouts(&project);
        let rows = self.machine_rows(&project, checkouts);
        let picks: Vec<_> = (rows.iter())
            .map(|row| Self::row_pick(checkouts, *row))
            .collect();
        let current = self.mesh.picks.get(&project);
        let at = picks.iter().position(|pick| pick.as_ref() == current);
        let next = match at {
            Some(at) => (1..rows.len() as isize)
                .map(|step| at as isize + delta * step)
                .take_while(|ix| (0..rows.len() as isize).contains(ix))
                .map(|ix| ix as usize)
                .find(|ix| picks[*ix].is_some()),
            None => picks.iter().position(Option::is_some),
        };
        if let Some(ix) = next
            && let Some((id, pick)) = picks[ix].clone()
        {
            self.pick(&project, &id, pick, cx);
            self.mesh
                .scroll
                .scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
        }
    }

    /// → in the list: open what's picked, or step into it when it's open. ← folds it, or steps
    /// up from a commit to its branch and from a branch to its checkout.
    fn fold_machines(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(Selection::Machines(project)) = self.selection(cx) else {
            return;
        };
        let Some((id, pick)) = self.mesh.picks.get(&project).cloned() else {
            return;
        };
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(&project, &id) else {
            return;
        };
        match pick {
            Pick::Checkout if checkout.branches.is_empty() => {}
            Pick::Checkout => {
                let key = (project.clone(), id.clone());
                let folded = self.mesh.folded_checkouts.contains(&key);
                match (open, folded) {
                    (true, true) | (false, false) => self.toggle_checkout(key, cx),
                    (true, false) => self.step_machines(1, cx),
                    (false, true) => {}
                }
            }
            Pick::Branch(name) => {
                let has_commits =
                    (checkout.branches.iter()).any(|b| b.name == name && !b.commits.is_empty());
                let key = (project.clone(), id.clone(), name);
                let is_open = self.mesh.open_branches.contains(&key);
                match (open, is_open) {
                    (true, false) if has_commits => self.toggle_mesh_branch(key, cx),
                    (true, true) => self.step_machines(1, cx),
                    (false, true) => self.toggle_mesh_branch(key, cx),
                    (false, false) => self.pick(&project, &id, Pick::Checkout, cx),
                    (true, false) => {}
                }
            }
            Pick::Commit(name, _) if !open => self.pick(&project, &id, Pick::Branch(name), cx),
            Pick::Commit(..) => {}
        }
    }

    fn toggle_machine(&mut self, key: (ProjectId, String), cx: &mut Context<Self>) {
        if !self.mesh.folded_machines.remove(&key) {
            self.mesh.folded_machines.insert(key);
        }
        cx.notify();
    }

    fn toggle_checkout(&mut self, key: (ProjectId, Arc<str>), cx: &mut Context<Self>) {
        if !self.mesh.folded_checkouts.remove(&key) {
            self.mesh.folded_checkouts.insert(key);
        }
        cx.notify();
    }

    /// Show a branch's commits under it, or fold them away. Folding it over its picked commit
    /// picks the branch.
    fn toggle_mesh_branch(&mut self, key: MeshBranch, cx: &mut Context<Self>) {
        if self.mesh.open_branches.remove(&key) {
            let (project, id, name) = &key;
            let on_commit = matches!(self.mesh.picks.get(project),
                Some((i, Pick::Commit(n, _))) if i == id && n == name);
            if on_commit {
                self.pick(project, id, Pick::Branch(name.clone()), cx);
            }
        } else {
            self.mesh.open_branches.insert(key);
        }
        cx.notify();
    }

    /// Fold or open what's picked on `project`'s page, as a double-click on its row does.
    fn fold_picked(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        match self.mesh.picks.get(project).cloned() {
            Some((id, Pick::Checkout)) => self.toggle_checkout((project.clone(), id), cx),
            Some((id, Pick::Branch(name))) => {
                self.toggle_mesh_branch((project.clone(), id, name), cx)
            }
            Some((_, Pick::Commit(..))) | None => {}
        }
    }

    /// The checkout picked on `project`'s page, and the branch picked there or holding the
    /// commit picked there.
    pub(super) fn machine_picked(&self, project: &ProjectId) -> Option<(Arc<str>, Option<String>)> {
        let (id, pick) = self.mesh.picks.get(project)?;
        let branch = match pick {
            Pick::Checkout => None,
            Pick::Branch(name) | Pick::Commit(name, _) => Some(name.clone()),
        };
        Some((id.clone(), branch))
    }

    fn branch_fetch(&self, project: &ProjectId, id: &Arc<str>, name: &str) -> Option<&Fetch> {
        let key = (project.clone(), id.clone(), name.to_owned());
        self.mesh.fetches.get(&key).map(|(_, fetch)| fetch)
    }

    /// The diff of what's picked on `project`'s page, once its branch is here.
    pub(super) fn machine_source(&self, project: &ProjectId) -> Option<DiffSource> {
        let (id, pick) = self.mesh.picks.get(project)?;
        let (name, commit) = match pick {
            Pick::Checkout => return None,
            Pick::Branch(name) => (name, None),
            Pick::Commit(name, commit) => (name, Some(*commit)),
        };
        let Fetch::Done(tip) = self.branch_fetch(project, id, name)? else {
            return None;
        };
        Some(match commit {
            Some(commit) => DiffSource::Commit(commit),
            None => DiffSource::Branch {
                tip: *tip,
                ahead: None,
            },
        })
    }

    /// What the diff pane shows for `project`'s page without a diff: a picked checkout's
    /// uncommitted files, or why a branch has none yet.
    pub(super) fn machine_placeholder(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let picked = self.mesh.picks.get(project);
        let Some(((id, pick), checkout)) =
            picked.and_then(|p| Some((p, store.mesh_checkout(project, &p.0)?)))
        else {
            return empty_state(None, "", "", theme, cx);
        };
        let name = match pick {
            Pick::Checkout => {
                let server = store.mesh.connection().map(|c| c.server.as_str());
                return uncommitted_view(checkout, server, theme);
            }
            Pick::Branch(name) | Pick::Commit(name, _) => name,
        };
        let branch = checkout.branches.iter().find(|b| b.name == *name);
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

    pub(super) fn render_machines_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let checkouts = store.mesh_checkouts(project);
        if checkouts.is_empty() {
            return match store.mesh.read(project) {
                true => empty_state(
                    Some(Illustration::Papers),
                    "Nothing unfinished elsewhere",
                    "Work on this project on your other machines shows here",
                    theme,
                    cx,
                ),
                false => empty_state(None, "Reading your other machines…", "", theme, cx),
            };
        }
        let rows = self.machine_rows(project, checkouts);
        let project = project.clone();
        side_list(
            "machine-rows",
            rows.len(),
            &self.mesh.scroll,
            "machine-scrollbar",
            cx,
            move |this, ix, theme, window, cx| {
                let checkouts = this.store.read(cx).mesh_checkouts(&project);
                let solid = this.side_focus.is_focused(window);
                this.machine_row(&project, checkouts, (ix, *rows.get(ix)?), solid, theme, cx)
            },
        )
        .pt(px(6.))
        .track_focus(&self.side_focus)
        .key_context("SideList")
        .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_machines(1, cx)))
        .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_machines(-1, cx)))
        .on_action(cx.listener(|this, _: &Expand, _, cx| this.fold_machines(true, cx)))
        .on_action(cx.listener(|this, _: &Collapse, _, cx| this.fold_machines(false, cx)))
        .into_any_element()
    }

    /// The list's row `ix`, `row`, on one line with what doesn't fit on hover. A click picks
    /// it, a double-click or its chevron folds it, and right-clicking offers what it can do.
    fn machine_row(
        &self,
        project: &ProjectId,
        checkouts: &[Checkout],
        (ix, row): (usize, Row),
        solid: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picked = Self::row_pick(checkouts, row);
        let selected = picked.is_some() && picked.as_ref() == self.mesh.picks.get(project);
        let on = selected && solid;
        let ink = |color| if on { theme.text_inverted } else { color };
        // A click picks the row and a double-click folds it; its chevron folds it only.
        let picks = |row: Stateful<Div>| {
            let (project, picked) = (project.clone(), picked.clone());
            row.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.side_focus, cx);
                if let Some((id, pick)) = picked.clone() {
                    this.pick(&project, &id, pick, cx);
                    if event.click_count() == 2 {
                        this.fold_picked(&project, cx);
                    }
                }
            }))
        };
        let menu = |row: Stateful<Div>| {
            let (project, picked) = (project.clone(), picked.clone());
            row.on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.side_focus, cx);
                    let Some((id, pick)) = picked.clone() else {
                        return;
                    };
                    this.pick(&project, &id, pick.clone(), cx);
                    let items = match &pick {
                        Pick::Checkout => this.checkout_menu(&project, &id, cx),
                        Pick::Branch(name) | Pick::Commit(name, _) => {
                            this.machine_branch_menu(&project, &id, name, cx)
                        }
                    };
                    (this.app)
                        .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                        .ok();
                }),
            )
        };
        let el = match row {
            Row::Machine(c) => {
                let checkout = checkouts.get(c)?;
                let key = (project.clone(), checkout.machine.clone());
                let open = !self.mesh.folded_machines.contains(&key);
                // The newest change of its checkouts.
                let last = (checkouts.iter())
                    .filter(|o| o.machine == checkout.machine)
                    .filter_map(|o| o.last_activity)
                    .max();
                let active = checkout.online && last.is_some_and(just_now);
                let (color, status) = match (active, checkout.online) {
                    (true, _) => (theme.safe, "Online, changed in the last 2 minutes"),
                    (false, true) => (theme.text_3, "Online"),
                    (false, false) => (theme.text_3.opacity(0.4), "Offline"),
                };
                side_row(("machine", ix), false, false, theme)
                    .pl(px(2.))
                    .child(chevron(
                        ("machine-fold", ix),
                        Some(open),
                        theme.text_3,
                        theme,
                    ))
                    .child(small_icon(Icon::Globe).text_color(theme.text_2))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .when(!checkout.online, |el| el.text_color(theme.text_2))
                            .child(checkout.machine.clone()),
                    )
                    .child(
                        div()
                            .id(("machine-online", ix))
                            .child(dot(color))
                            .tooltip(Tooltip::text(status)),
                    )
                    .child(age_column(last, theme))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_machine(key.clone(), cx)
                    }))
            }
            Row::Checkout(c) => {
                let checkout = checkouts.get(c)?;
                let id: Arc<str> = checkout.id.as_str().into();
                let key = (project.clone(), id);
                let open = !self.mesh.folded_checkouts.contains(&key);
                let unpushed: usize = (checkout.branches.iter())
                    .map(|b| b.commits.iter().filter(|c| !c.pushed).count())
                    .sum();
                let files = checkout.uncommitted.len();
                let tip = match &checkout.error {
                    Some(err) => {
                        format!("{}\nThe mesh couldn't read all of it: {err}", checkout.path)
                    }
                    None => checkout.path.clone(),
                };
                let fold = (!checkout.branches.is_empty()).then_some(open);
                let row = side_row(("checkout", ix), selected, solid, theme)
                    .pl(px(16.))
                    .child(fold_on_click(
                        chevron(("checkout-fold", ix), fold, ink(theme.text_3), theme),
                        cx,
                        move |this, cx| this.toggle_checkout(key.clone(), cx),
                    ))
                    .child(small_icon(Icon::Folder).text_color(ink(theme.text_2)))
                    .child(
                        div()
                            .id(("checkout-name", ix))
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .when(checkout.error.is_some() && !on, |el| {
                                el.text_color(theme.warn)
                            })
                            .child(place(checkout))
                            .tooltip(Tooltip::text(tip)),
                    )
                    .children(meta_count(
                        Icon::File,
                        files,
                        ink(theme.warn),
                        plural(files, "uncommitted file"),
                    ))
                    .when(!open, |el| {
                        el.children(meta_count(
                            Icon::ArrowUp,
                            unpushed,
                            ink(theme.blue),
                            plural(unpushed, "unpushed commit"),
                        ))
                    })
                    .child(quiet(age_column(checkout.last_activity, theme), on, theme));
                menu(picks(row))
            }
            Row::Branch(c, b) => {
                let checkout = checkouts.get(c)?;
                let branch = checkout.branches.get(b)?;
                let id: Arc<str> = checkout.id.as_str().into();
                let fetch = match self.branch_fetch(project, &id, &branch.name) {
                    Some(Fetch::Running) => Some((
                        Icon::Refresh,
                        ink(theme.text_3),
                        format!("Fetching it from {}", checkout.machine),
                    )),
                    Some(Fetch::Failed(err)) => Some((
                        Icon::Warning,
                        ink(theme.warn),
                        format!("Couldn't fetch it: {err}"),
                    )),
                    Some(Fetch::Done(_)) | None => None,
                };
                let key = (project.clone(), id, branch.name.clone());
                let open = self.mesh.open_branches.contains(&key);
                let (glyph, color) = branch_glyph(branch.status, theme);
                let unpushed = branch.commits.iter().filter(|c| !c.pushed).count();
                let commits = branch.commits.len();
                let fold = (commits > 0).then_some(open);
                let row = side_row(("branch", ix), selected, solid, theme)
                    .pl(px(30.))
                    .child(fold_on_click(
                        chevron(("branch-fold", ix), fold, ink(theme.text_3), theme),
                        cx,
                        move |this, cx| this.toggle_mesh_branch(key.clone(), cx),
                    ))
                    .child(small_icon(glyph).text_color(ink(color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(ink(theme.blue))
                            .child(branch.name.clone()),
                    )
                    .children(fetch.map(|(glyph, color, tip)| {
                        div()
                            .id(("branch-fetch", ix))
                            .child(small_icon(glyph).text_color(color))
                            .tooltip(Tooltip::text(tip))
                    }))
                    // Unpushed commits, or else, folded, how many it has; never both.
                    .children(meta_count(
                        Icon::ArrowUp,
                        unpushed,
                        ink(theme.blue),
                        plural(unpushed, "unpushed commit"),
                    ))
                    .when(unpushed == 0 && !open, |el| {
                        el.children(meta_count(
                            Icon::Commit,
                            commits,
                            ink(theme.text_3),
                            plural(commits, "commit"),
                        ))
                    });
                menu(picks(row))
            }
            Row::Commit(c, b, k) => {
                let checkout = checkouts.get(c)?;
                let branch = checkout.branches.get(b)?;
                let commit = branch.commits.get(k)?;
                let id: Arc<str> = checkout.id.as_str().into();
                let here = matches!(
                    self.branch_fetch(project, &id, &branch.name),
                    Some(Fetch::Done(_))
                );
                let tip = match (&commit.author, commit.created_at) {
                    (Some(author), Some(at)) => format!("{author}, {} ago", relative_time(at)),
                    (Some(author), None) => author.clone(),
                    (None, Some(at)) => format!("{} ago", relative_time(at)),
                    (None, None) => commit.id.chars().take(7).collect(),
                };
                let row = side_row(("commit", ix), selected, solid, theme)
                    .pl(px(52.))
                    .when(!here && !on, |el| el.text_color(theme.text_2))
                    .child(
                        small_icon(Icon::Commit).text_color(ink(match commit.pushed {
                            true => theme.text_3,
                            false => theme.blue,
                        })),
                    )
                    .child(
                        div()
                            .id(("commit-title", ix))
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(commit_title(&commit.title))
                            .tooltip(Tooltip::text(tip)),
                    )
                    .child(quiet(age_column(commit.created_at, theme), on, theme));
                menu(picks(row))
            }
        };
        Some(el.into_any_element())
    }

    /// What right-clicking a checkout offers: it on but.dev, where its uncommitted changes
    /// are, and its path there.
    fn checkout_menu(&self, project: &ProjectId, id: &Arc<str>, cx: &gpui::App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        if let Some(connection) = store.mesh.connection() {
            let url = mesh::link(&connection.server, id, None);
            items.push(MenuItem::new("Open on but.dev", move |_, _, cx| {
                cx.open_url(&url)
            }));
        }
        let path = checkout.path.clone();
        items.push(MenuItem::new("Copy path", move |app, _, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
            app.store.update(cx, |store, cx| store.snack("Copied", cx));
        }));
        items
    }

    /// What right-clicking a branch, or one of its commits, offers: bringing it here, and its
    /// page on but.dev.
    fn machine_branch_menu(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        name: &str,
        cx: &gpui::App,
    ) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let Some(branch) = (store.mesh_checkout(project, id))
            .and_then(|c| c.branches.iter().find(|b| b.name == name))
        else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let managed = store.workspace(project).is_some_and(|ws| ws.managed);
        if let Some(mesh_id) = branch.mesh_id.clone().filter(|_| managed) {
            let (project, name) = (project.clone(), branch.name.clone());
            items.push(MenuItem::new("Apply here", move |app, _, cx| {
                let (mesh_id, name) = (mesh_id.clone(), name.clone());
                app.store.update(cx, |store, cx| {
                    store.mesh_apply(&project, mesh_id, name, cx)
                })
            }));
        }
        if let Some(connection) = store.mesh.connection() {
            let url = mesh::link(&connection.server, id, Some(name));
            items.push(MenuItem::new("Open on but.dev", move |_, _, cx| {
                cx.open_url(&url)
            }));
        }
        items
    }
}

/// Where a checkout is on its machine, as its row names it: its worktree's name, or else its
/// folder's.
fn place(checkout: &Checkout) -> String {
    (checkout.worktree.clone())
        .or_else(|| checkout.folder.clone())
        .or_else(|| {
            let folder = Path::new(&checkout.path).file_name()?;
            Some(folder.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| checkout.path.clone())
}

/// A row's fold control: a chevron, down while `open`, or a dot when there's nothing under the
/// row to fold (`None`).
fn chevron(
    id: impl Into<ElementId>,
    open: Option<bool>,
    color: Hsla,
    theme: &Theme,
) -> Stateful<Div> {
    let step = div()
        .id(id)
        .flex_none()
        .w(px(14.))
        .h(px(16.))
        .flex()
        .items_center()
        .justify_center();
    match open {
        None => step.child(small_icon(Icon::BulletDisc).text_color(theme.text_3.opacity(0.6))),
        Some(open) => step.rounded(px(3.)).hover(|el| el.bg(theme.hover())).child(
            small_icon(match open {
                true => Icon::ChevronDown,
                false => Icon::ChevronRight,
            })
            .text_color(color),
        ),
    }
}

/// `step`, a row's chevron, folding the row on a click without picking it.
fn fold_on_click(
    step: Stateful<Div>,
    cx: &Context<Detail>,
    fold: impl Fn(&mut Detail, &mut Context<Detail>) + 'static,
) -> Stateful<Div> {
    step.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            fold(this, cx)
        }))
}

/// A checkout's uncommitted files, for the diff pane: their diffs are on that machine, so a
/// button opens it on but.dev.
fn uncommitted_view(checkout: &Checkout, server: Option<&str>, theme: &Theme) -> AnyElement {
    let files = &checkout.uncommitted;
    let title = match files.len() {
        0 => format!("Nothing uncommitted on {}", checkout.machine),
        n => format!("{} on {}", plural(n, "uncommitted file"), checkout.machine),
    };
    let more = files.len().saturating_sub(LISTED);
    v_flex()
        .p(px(24.))
        .gap(px(12.))
        .max_w(px(640.))
        .child(
            v_flex()
                .gap(px(2.))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                .child(faint(checkout.path.clone(), theme)),
        )
        .when(!files.is_empty(), |el| {
            let listed = files.iter().take(LISTED).map(|file| {
                h_flex()
                    .gap(px(8.))
                    .child(small_icon(Icon::File).text_color(theme.warn))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(file.path.clone()),
                    )
                    .child(faint(
                        format!("+{} −{}", file.additions, file.deletions),
                        theme,
                    ))
            });
            el.child(v_flex().gap(px(4.)).children(listed))
                .when(more > 0, |el| {
                    el.child(faint(format!("and {more} more"), theme))
                })
        })
        .children(server.map(|server| {
            let url = mesh::link(server, &checkout.id, None);
            h_flex().child(
                button("checkout-on-but-dev", "Open on but.dev")
                    .small()
                    .icon(Icon::Link)
                    .style(ButtonStyle::Ghost)
                    .tooltip("Their diffs are on that machine")
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            )
        }))
        .into_any_element()
}

/// What a branch on another machine shows as: merged, or still to finish.
fn branch_glyph(status: BranchStatus, theme: &Theme) -> (Icon, Hsla) {
    match status {
        BranchStatus::Integrated => (Icon::Tick, theme.safe),
        BranchStatus::Diverged => (Icon::Branch, theme.warn),
        BranchStatus::Pushed | BranchStatus::Unpushed | BranchStatus::Empty => {
            (Icon::Branch, theme.text_2)
        }
    }
}
