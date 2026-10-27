//! A watched project's checkouts on the user's other machines, from the mesh, after its own
//! worktrees and drawn as they are: each checkout's branches, and their commits, fold as a
//! worktree's do, under a heading that starts folded. The crate's AGENTS.md has the
//! mesh's rules.

use std::sync::Arc;

use gpui::{AnyElement, App, Context, MouseButton, Window, div, prelude::*};

use super::rows::{COMMIT_INDENT, Row, Rows, found, matches};
use super::{ClickOpened, Navigator, RowStyle, STEP, change_counts, click_opens, double_click};
use crate::app::CopySelection;
use crate::backend::mesh::{self, BranchStatus};
use crate::overlays::MenuItem;
use crate::state::{MeshBranch, MeshPick, ProjectId, Section, Selection, Store};
use crate::ui::{Icon, Tooltip, faint, icon, small_icon};

impl Navigator {
    /// `id`'s checkouts on other machines under their heading, unless that's folded without
    /// the selection in them. Each lists its branches unless it's folded, and an open branch
    /// its commits, as a worktree does; a line rules each off.
    pub(super) fn mesh_rows(&self, store: &Store, id: &ProjectId, rows: &mut Rows) {
        let checkouts = store.mesh_checkouts(id);
        if checkouts.is_empty() {
            return;
        }
        rows.rule_off();
        rows.push(Row::Item(Selection::Section(id.clone(), Section::Mesh)));
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
                rows.open_in(STEP);
                for branch in &checkout.branches {
                    rows.push(at(MeshPick::Branch(branch.name.clone())));
                    let on_commit =
                        matches!(pick, Some(MeshPick::Commit(name, _)) if *name == branch.name);
                    let key =
                        ClickOpened::Elsewhere((id.clone(), key.clone(), branch.name.clone()));
                    if on_commit || self.is_open(&key) {
                        rows.open();
                        rows.extend(
                            (branch.commits.iter())
                                .map(|commit| at(MeshPick::Commit(branch.name.clone(), commit.id))),
                        );
                        rows.close();
                    }
                }
                rows.close();
            }
            rows.push(Row::WorktreeDivider);
        }
    }

    /// [`Self::mesh_rows`] under the filter, with `held` the words found in the project's
    /// name: the checkouts that match, or have branches that do, with those branches, ruled
    /// off from the rows above as there.
    pub(super) fn filtered_mesh_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        terms: &[String],
        held: u64,
        rows: &mut Rows,
    ) {
        let start = rows.rows.len();
        rows.rule_off();
        rows.push(Row::Item(Selection::Section(id.clone(), Section::Mesh)));
        let heading_end = rows.rows.len();
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
            rows.open_in(STEP);
            for branch in branches {
                rows.push_found(at(MeshPick::Branch(branch.name.clone())), true);
            }
            rows.close();
        }
        if rows.rows.len() == heading_end {
            rows.truncate(start);
        }
    }

    /// Over `id`'s checkouts elsewhere, lined up with its worktrees' chevrons: folded, it
    /// says how many there are.
    pub(super) fn mesh_heading(
        &self,
        ix: usize,
        id: &ProjectId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let open = self.mesh_open(id, cx);
        let checkouts = store.mesh_checkouts(id);
        let label = "Network worktrees";
        let row = self.section_heading(ix, id, &Section::Mesh, label, style, cx);
        row.when(!open, |el| {
            el.child(faint(checkouts.len().to_string(), style.theme).text_color(style.quiet()))
        })
        .tooltip(Tooltip::text(
            "Unfinished work on your other machines, from your mesh",
        ))
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
                let glyph = small_icon(glyph).text_color(style.meta(color));
                let title = commit.title.clone().into();
                style.commit_line(id, COMMIT_INDENT, glyph, title, false)
            }
            (MeshPick::Branch(name), Some(branch)) => {
                let fold = (project.clone(), key.clone(), name.clone());
                let clicked = ClickOpened::Elsewhere(fold.clone());
                let on_commit = matches!(&store.selection,
                    Some(Selection::Mesh(p, i, MeshPick::Commit(n, _))) if (p, i, n) == (project, key, name));
                let open = self.is_open(&clicked) || on_commit;
                let n = branch.commits.len();
                let unpushed = branch.commits.iter().filter(|c| !c.pushed).count();
                let (glyph, color) = match branch.status {
                    BranchStatus::Integrated => (Icon::BranchMerge, theme.purple),
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
                            .min_w_0()
                            .truncate()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(name.clone()),
                    )
                    .children(style.unpushed(unpushed))
                    .when(!open, |el| el.children(style.commits(n)))
                    .when(n > 0, |el| el.on_click(click_opens(clicked, toggle, cx)))
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
                    .child(self.fold(
                        ("mesh-fold", ix),
                        folds.then_some(open),
                        quiet,
                        cx,
                        toggle.clone(),
                    ))
                    .child(icon(Icon::Computer).text_color(style.muted()))
                    .child(
                        div()
                            .id(("mesh-name", ix))
                            .flex_grow(1.)
                            .min_w_0()
                            .truncate()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .when(!checkout.online, |el| el.text_color(style.muted()))
                            .when(checkout.error.is_some() && !style.solid, |el| {
                                el.text_color(theme.warn)
                            })
                            .tooltip(Tooltip::text(tip))
                            .child(checkout.label()),
                    )
                    .children(change_counts(
                        (files.len(), 0, (added as u32, removed as u32)),
                        style,
                    ))
                    .when(!open, |el| el.children(style.unpushed(unpushed)))
                    .when(folds, |el| el.on_click(double_click(toggle, cx)))
            }
            _ => return None,
        };
        Some(
            row.on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .into_any_element(),
        )
    }

    /// Whether `project`'s work elsewhere shows under its heading: unfolded, with the
    /// selection in it, or found by the filter.
    pub(super) fn mesh_open(&self, project: &ProjectId, cx: &App) -> bool {
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
        if open && on_commit {
            self.select(
                Selection::Mesh(project, id, MeshPick::Branch(name)),
                window,
                cx,
            )
        }
    }

    /// ⇧→ on a mesh row: unfold a checkout, or open a branch.
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
                cx.notify();
            }
            MeshPick::Branch(name) => {
                self.set_open(ClickOpened::Elsewhere((project, id, name)), true, cx)
            }
            MeshPick::Commit(..) => {}
        }
    }

    /// ⇧← on a mesh row, as a tree steps out: fold an open checkout or branch, or else go up to
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
                    false => self.select(Selection::Section(project, Section::Mesh), window, cx),
                }
            }
            MeshPick::Branch(name) => {
                let has_commits =
                    (checkout.branches.iter()).any(|b| b.name == name && !b.commits.is_empty());
                let key = (project.clone(), id.clone(), name);
                match has_commits && self.is_open(&ClickOpened::Elsewhere(key.clone())) {
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
            items.push(MenuItem::keyed(
                "Copy path",
                &CopySelection,
                cx,
                move |app, _, cx| app.copy_text(path.clone(), cx),
            ));
        }
        items
    }
}
