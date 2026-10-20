//! What the detail pane shows of a checkout on another machine, picked in the navigator: its
//! uncommitted files, whose diffs stay on that machine and open on but.dev, or a branch's or a
//! commit's diff, once fetched from the copy the mesh keeps, one at a time.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};

use super::Detail;
use crate::backend::mesh::{self, Checkout};
use crate::backend::{self, CommitId, DiffSource};
use crate::state::{MeshPick, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::{
    ButtonStyle, Icon, button, empty_state, faint, h_flex, plural, small_icon, v_flex,
};

/// How many of a checkout's uncommitted files its view lists; the rest open on but.dev.
const LISTED: usize = 50;

/// A branch of a checkout on another machine: the project, the checkout by the mesh's id, and
/// the branch by name.
type MeshBranch = (ProjectId, Arc<str>, String);

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
    /// machine reported them, the selected one first.
    pub(super) fn load_machines(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
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
        let mut wanted: Vec<_> = (store.mesh_checkouts(project).iter())
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
                .gap(px(12.))
                .child(faint(line, theme).text_size(px(13.)))
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
            (MeshPick::Checkout, Some(checkout)) => uncommitted_list(checkout, theme),
            _ => self.render_diff_file_list(theme, cx),
        }
    }
}

/// A checkout's uncommitted files, as that machine reported them.
fn uncommitted_list(checkout: &Checkout, theme: &Theme) -> AnyElement {
    let files = &checkout.uncommitted;
    let title = match files.len() {
        0 => "Nothing uncommitted".to_owned(),
        n => plural(n, "uncommitted file"),
    };
    let more = files.len().saturating_sub(LISTED);
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
    v_flex()
        .size_full()
        .p(px(12.))
        .gap(px(8.))
        .child(
            v_flex()
                .gap(px(2.))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                .child(faint(checkout.path.clone(), theme)),
        )
        .child(v_flex().gap(px(4.)).children(listed))
        .when(more > 0, |el| {
            el.child(faint(format!("and {more} more"), theme))
        })
        .into_any_element()
}
