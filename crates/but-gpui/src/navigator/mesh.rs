//! A watched project's checkouts on the user's other machines, from the mesh, after its own
//! worktrees and drawn as they are: each checkout's branches, and their commits, fold as a
//! worktree's do, under a heading that starts folded. The crate's AGENTS.md has the
//! mesh's rules.

use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, MouseButton, Window, div, prelude::*, px,
};

use super::menu::keyed;
use super::{
    COMMIT_INDENT, ClickOpened, NEST, Navigator, Row, RowStyle, Rows, found, matches,
    open_on_click, section_heading, uncommitted_counts,
};
use crate::app::CopySelection;
use crate::backend::mesh::{self, BranchStatus};
use crate::overlays::MenuItem;
use crate::state::{MeshBranch, MeshPick, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::{
    Icon, Tooltip, commit_title, dot, faint, icon, just_now, meta_count, plural, section_label,
    small_icon,
};

impl Navigator {
    /// `id`'s checkouts on other machines under their heading, unless that's folded without
    /// the selection in them. Each lists its branches unless it's folded, and an open branch
    /// its commits, as a worktree does; a line rules each off.
    pub(super) fn mesh_rows(&self, store: &Store, id: &ProjectId, rows: &mut Rows) {
        let checkouts = store.mesh_checkouts(id);
        if checkouts.is_empty() {
            return;
        }
        if !matches!(rows.rows.last(), Some(Row::WorktreeDivider)) {
            rows.push(Row::WorktreeDivider);
        }
        rows.push(Row::MeshHeading(id.clone()));
        let selected = match &store.selection {
            Some(Selection::Mesh(project, checkout, pick)) if project == id => {
                Some((checkout, pick))
            }
            _ => None,
        };
        if !store.prefs.shown_mesh.contains(id.as_ref()) && selected.is_none() {
            return;
        }
        for checkout in checkouts {
            let key: Arc<str> = checkout.id.as_str().into();
            let at = |pick| Row::Item(Selection::Mesh(id.clone(), key.clone(), pick));
            let pick = selected.filter(|(on, _)| **on == key).map(|(_, pick)| pick);
            rows.push(at(MeshPick::Checkout));
            let inside = pick.and_then(MeshPick::branch).is_some();
            if inside || !self.folded_checkouts.contains(&(id.clone(), key.clone())) {
                rows.open();
                rows.inset = NEST;
                for branch in &checkout.branches {
                    rows.push(at(MeshPick::Branch(branch.name.clone())));
                    let on_commit =
                        matches!(pick, Some(MeshPick::Commit(name, _)) if *name == branch.name);
                    let key = (id.clone(), key.clone(), branch.name.clone());
                    if on_commit || self.mesh_branch_open(&key) {
                        rows.open();
                        rows.extend(
                            (branch.commits.iter())
                                .map(|commit| at(MeshPick::Commit(branch.name.clone(), commit.id))),
                        );
                        rows.close();
                    }
                }
                rows.inset = 0.;
                rows.close();
            }
            rows.push(Row::WorktreeDivider);
        }
    }

    /// [`Self::mesh_rows`] under the filter, with `held` the words found in the project's
    /// name: the checkouts that match, or have branches that do, with those branches.
    pub(super) fn filtered_mesh_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        terms: &[String],
        held: u64,
        rows: &mut Rows,
    ) {
        let start = rows.rows.len();
        rows.push(Row::MeshHeading(id.clone()));
        for checkout in store.mesh_checkouts(id) {
            let own = found(terms, &[&checkout.label()]);
            let hit = matches(terms, held, own);
            let branches: Vec<_> = (checkout.branches.iter())
                .filter(|b| matches(terms, held | own, found(terms, &[&b.name])))
                .collect();
            if !hit && branches.is_empty() {
                continue;
            }
            let key: Arc<str> = checkout.id.as_str().into();
            let at = |pick| Selection::Mesh(id.clone(), key.clone(), pick);
            rows.push_found(at(MeshPick::Checkout), hit);
            rows.open();
            rows.inset = NEST;
            for branch in branches {
                rows.push_found(at(MeshPick::Branch(branch.name.clone())), true);
            }
            rows.inset = 0.;
            rows.close();
        }
        if rows.rows.len() == start + 1 {
            rows.truncate(start);
        }
    }

    /// Over `id`'s checkouts elsewhere, lined up with its worktrees' chevrons: a click folds
    /// them all away, and folded, it says how many there are.
    pub(super) fn mesh_heading(
        &self,
        id: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let open = self.mesh_open(id, cx);
        let checkouts = store.mesh_checkouts(id);
        let active = (checkouts.iter()).any(|c| c.online && c.last_activity.is_some_and(just_now));
        let project = id.clone();
        let heading = ElementId::Name(format!("mesh-heading-{id}").into());
        let label = section_label("Network worktrees", theme);
        section_heading(heading, 6., open, Some(Icon::Globe), label, theme)
            .when(!open, |el| {
                el.child(faint(checkouts.len().to_string(), theme))
                    .when(active, |el| el.child(dot(theme.safe)))
            })
            .tooltip(Tooltip::text(
                "Unfinished work on your other machines, from your mesh",
            ))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let inside = matches!(&this.store.read(cx).selection,
                    Some(Selection::Mesh(p, ..)) if *p == project);
                this.toggle_shown(&project, |p| &mut p.shown_mesh, open, inside, window, cx)
            }))
            .into_any_element()
    }

    /// A row of another machine's: a checkout, drawn as a worktree's heading, or a branch or
    /// commit in it, drawn as one here.
    pub(super) fn mesh_row(
        &self,
        ix: usize,
        item: &Selection,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let Selection::Mesh(project, key, pick) = item else {
            return None;
        };
        let store = self.store.read(cx);
        let checkout = store.mesh_checkout(project, key)?;
        let theme = style.theme;
        let quiet = style.quiet();
        let branch = match pick.branch() {
            Some(name) => Some(checkout.branches.iter().find(|b| b.name == name)?),
            None => None,
        };
        let row = match (pick, branch) {
            (MeshPick::Commit(_, id), Some(branch)) => {
                let commit = (branch.commits.iter()).find(|c| c.id == *id)?;
                let (glyph, color) = match commit.pushed {
                    true => (Icon::CommitFill, style.muted()),
                    false => (Icon::Commit, theme.blue),
                };
                let id = format!("mesh-commit-{project}-{key}-{:?}", commit.id);
                (style.row(id, COMMIT_INDENT))
                    .gap(px(10.))
                    .aria_label(commit.title.clone())
                    .child(small_icon(glyph).text_color(style.meta(color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(commit_title(&commit.title)),
                    )
                    .children(commit.created_at.map(|at| style.age(at)))
            }
            (MeshPick::Branch(name), Some(branch)) => {
                let fold = (project.clone(), key.clone(), name.clone());
                let clicked = ClickOpened::Elsewhere(fold.clone());
                let on_commit = matches!(&store.selection,
                    Some(Selection::Mesh(p, i, MeshPick::Commit(n, _))) if (p, i, n) == (project, key, name));
                let open = self.mesh_branch_open(&fold) || on_commit;
                let n = branch.commits.len();
                let unpushed = branch.commits.iter().filter(|c| !c.pushed).count();
                let (glyph, color) = match branch.status {
                    BranchStatus::Integrated => (Icon::Tick, theme.safe),
                    BranchStatus::Diverged => (Icon::Branch, theme.warn),
                    BranchStatus::Other => (Icon::Branch, style.muted()),
                };
                let toggle = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                    this.toggle_mesh_branch(fold.clone(), open, window, cx)
                };
                (style.row(format!("mesh-branch-{project}-{key}-{name}"), 6.))
                    .aria_label(format!("Branch {name}"))
                    .child(self.fold(
                        ("mesh-branch-fold", ix),
                        (n > 0).then_some(open),
                        quiet,
                        cx,
                        toggle.clone(),
                    ))
                    .child(icon(glyph).text_color(style.meta(color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(name.clone()),
                    )
                    .children(style.unpushed(unpushed))
                    .when(!open, |el| {
                        el.children(meta_count(
                            Icon::Commit,
                            n,
                            style.muted(),
                            plural(n, "commit"),
                        ))
                    })
                    .when(n > 0, |el| el.on_click(open_on_click(clicked, cx)))
            }
            (MeshPick::Checkout, _) => {
                let fold = (project.clone(), key.clone());
                let inside = matches!(&store.selection,
                    Some(Selection::Mesh(p, i, pick)) if (p, i) == (project, key) && pick.branch().is_some());
                let open = inside || !self.folded_checkouts.contains(&fold);
                let files = &checkout.uncommitted;
                let (added, removed) =
                    (files.iter()).fold((0, 0), |(a, r), f| (a + f.additions, r + f.deletions));
                let unpushed: usize = (checkout.branches.iter())
                    .map(|b| b.commits.iter().filter(|c| !c.pushed).count())
                    .sum();
                let active = checkout.online && checkout.last_activity.is_some_and(just_now);
                let mut tip = checkout.path.clone();
                if !checkout.online {
                    tip.push_str("\nOffline");
                }
                if let Some(err) = &checkout.error {
                    tip.push_str(&format!("\nThe mesh couldn't read all of it: {err}"));
                }
                let toggle = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                    this.toggle_checkout(fold.clone(), open, window, cx)
                };
                let folds = !checkout.branches.is_empty();
                (style.row(format!("mesh-{project}-{key}"), 6.))
                    .aria_label(checkout.label())
                    .aria_expanded(open)
                    .child(self.fold(("mesh-fold", ix), folds.then_some(open), quiet, cx, toggle))
                    .child(icon(Icon::Computer).text_color(style.muted()))
                    .child(
                        div()
                            .id(("mesh-name", ix))
                            .flex_grow(1.)
                            .min_w(px(0.))
                            .truncate()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .when(!checkout.online, |el| el.text_color(style.muted()))
                            .when(checkout.error.is_some() && !style.solid, |el| {
                                el.text_color(theme.warn)
                            })
                            .tooltip(Tooltip::text(tip))
                            .child(checkout.label()),
                    )
                    // As a worktree's, the quiet "uncommitted" gives way before the name does.
                    .when(!files.is_empty(), |el| {
                        let lines = (added as u32, removed as u32);
                        el.child(uncommitted_counts((files.len(), 0, lines), style))
                    })
                    .when(!open, |el| el.children(style.unpushed(unpushed)))
                    .when(active, |el| el.child(style.active(("mesh-active", ix))))
            }
            _ => return None,
        };
        Some(
            row.on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .into_any_element(),
        )
    }

    /// Whether `key`'s commits show: as Settings start branches, unless folded or opened
    /// since, as a branch here.
    pub(super) fn mesh_branch_open(&self, key: &MeshBranch) -> bool {
        self.branches_start_open != self.flipped_mesh_branches.contains(key)
    }

    /// Whether `project`'s work elsewhere shows under its heading: unfolded, with the
    /// selection in it, or found by the filter.
    fn mesh_open(&self, project: &ProjectId, cx: &App) -> bool {
        let store = self.store.read(cx);
        store.prefs.shown_mesh.contains(project.as_ref())
            || matches!(&store.selection, Some(Selection::Mesh(p, ..)) if p == project)
            || self.filtering(cx)
    }

    /// Fold checkout `key`'s branches, or unfold them. Folding them over the selection takes
    /// it up to the checkout, as a worktree does.
    fn toggle_checkout(
        &mut self,
        key: (ProjectId, Arc<str>),
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !open {
            self.folded_checkouts.remove(&key);
            return cx.notify();
        }
        let inside = matches!(&self.store.read(cx).selection,
            Some(Selection::Mesh(p, id, pick)) if (p, id) == (&key.0, &key.1) && pick.branch().is_some());
        let (project, id) = key.clone();
        self.folded_checkouts.insert(key);
        match inside {
            true => self.select(Selection::Mesh(project, id, MeshPick::Checkout), window, cx),
            false => cx.notify(),
        }
    }

    /// Show branch `key`'s commits, or fold them away. Folding them over the selected one
    /// takes the selection up to the branch.
    fn toggle_mesh_branch(
        &mut self,
        key: MeshBranch,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Shown open over its selected commit whatever was stored, it only flips when that
        // agrees.
        self.set_open(ClickOpened::Elsewhere(key.clone()), !open, cx);
        let (project, id, name) = key;
        let on_commit = matches!(&self.store.read(cx).selection,
            Some(Selection::Mesh(p, i, MeshPick::Commit(n, _))) if (p, i, n) == (&project, &id, &name));
        match open && on_commit {
            true => self.select(
                Selection::Mesh(project, id, MeshPick::Branch(name)),
                window,
                cx,
            ),
            false => cx.notify(),
        }
    }

    /// → on a mesh row: unfold a checkout, or open a branch.
    pub(super) fn expand_mesh(
        &mut self,
        project: ProjectId,
        id: Arc<str>,
        pick: MeshPick,
        cx: &mut Context<Self>,
    ) {
        match pick {
            MeshPick::Checkout => {
                self.folded_checkouts.remove(&(project, id));
            }
            MeshPick::Branch(name) => {
                self.set_open(ClickOpened::Elsewhere((project, id, name)), true, cx)
            }
            MeshPick::Commit(..) => {}
        }
        cx.notify();
    }

    /// ← on a mesh row, as a tree steps out: fold an open checkout or branch, or else go up to
    /// what holds it.
    pub(super) fn collapse_mesh(
        &mut self,
        project: ProjectId,
        id: Arc<str>,
        pick: MeshPick,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(&project, &id) else {
            return;
        };
        match pick {
            MeshPick::Checkout => {
                let key = (project.clone(), id);
                match !checkout.branches.is_empty() && !self.folded_checkouts.contains(&key) {
                    true => self.toggle_checkout(key, true, window, cx),
                    false => self.select(Selection::Project(project), window, cx),
                }
            }
            MeshPick::Branch(name) => {
                let has_commits =
                    (checkout.branches.iter()).any(|b| b.name == name && !b.commits.is_empty());
                let key = (project.clone(), id.clone(), name);
                match has_commits && self.mesh_branch_open(&key) {
                    true => self.toggle_mesh_branch(key, true, window, cx),
                    false => {
                        self.select(Selection::Mesh(project, id, MeshPick::Checkout), window, cx)
                    }
                }
            }
            MeshPick::Commit(name, _) => self.select(
                Selection::Mesh(project, id, MeshPick::Branch(name)),
                window,
                cx,
            ),
        }
    }

    /// What right-clicking a mesh row offers: for a checkout, it on but.dev, where its
    /// uncommitted changes are, and its path there; for a branch or a commit on one, bringing
    /// the branch here and its page on but.dev.
    pub(super) fn mesh_menu(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let Selection::Mesh(project, id, pick) = target else {
            return Vec::new();
        };
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let branch =
            (pick.branch()).and_then(|name| checkout.branches.iter().find(|b| b.name == name));
        let managed = store.workspace(project).is_some_and(|ws| ws.managed);
        if let Some((branch, mesh_id)) = branch.and_then(|b| Some((b, b.mesh_id.clone()?)))
            && managed
        {
            let (project, name) = (project.clone(), branch.name.clone());
            items.push(MenuItem::new("Apply here", move |app, _, cx| {
                let (mesh_id, name) = (mesh_id.clone(), name.clone());
                app.store.update(cx, |store, cx| {
                    store.mesh_apply(&project, mesh_id, name, cx)
                })
            }));
        }
        if let Some(connection) = store.mesh.connection() {
            let url = mesh::link(&connection.server, id, pick.branch());
            items.push(MenuItem::new("Open on but.dev", move |_, _, cx| {
                cx.open_url(&url)
            }));
        }
        if *pick == MeshPick::Checkout {
            let path = checkout.path.clone();
            items.push(keyed("Copy path", &CopySelection, cx, move |app, _, cx| {
                app.copy_text(path.clone(), cx)
            }));
        }
        items
    }
}
