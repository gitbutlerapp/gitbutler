//! A project's checkout on another machine, as the mesh reports it: its uncommitted files and
//! its branches with their commits, listed beside a diff. Opening it fetches its branches from
//! the copy the mesh keeps, one at a time, so a branch or one of its commits shows its diff as
//! a branch of this machine's would. Its uncommitted files' diffs stay on that machine, and
//! open on but.dev.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, ElementId, MouseButton, MouseDownEvent, SharedString, Window,
    div, prelude::*, px,
};

use super::Detail;
use super::panes::{list_heading, side_header, side_row, title_text};
use crate::backend::mesh::{self, BranchStatus, Checkout};
use crate::backend::{self, CommitId, DiffSource};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::person::person_avatar;
use crate::ui::{
    Icon, Tooltip, age_column, commit_title, dot, empty_state, faint, h_flex, icon, icon_button,
    just_now, plural, relative_time, small_icon, v_flex,
};

/// A branch of a checkout on another machine: the project, the checkout by the mesh's id, and
/// the branch by name.
pub(super) type MeshBranch = (ProjectId, Arc<str>, String);

/// What's picked on a machine's page, whose diff shows beside it.
#[derive(Clone, PartialEq)]
pub(super) enum Pick {
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

/// What the app has fetched from other machines, and what it's fetching.
#[derive(Default)]
pub(super) struct MeshFetches {
    /// Each branch's fetch, with the tip that machine reported when it started: a new one
    /// fetches it again.
    fetches: HashMap<MeshBranch, (Option<String>, Fetch)>,
    /// Branches to fetch, in turn, each with its project's path, the mesh's id for it and the
    /// tip that machine reported.
    queue: Vec<(MeshBranch, std::path::PathBuf, String, Option<String>)>,
    running: bool,
    /// The pick on each checkout's page, kept while another page shows.
    picks: HashMap<(ProjectId, Arc<str>), Pick>,
}

impl Detail {
    /// Fetch the branches of `project`'s checkout `id` that have no copy here as that machine
    /// reported them, the picked one first, and pick its first branch if nothing is.
    pub(super) fn load_machine(
        &mut self,
        project: &ProjectId,
        id: &Arc<str>,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let (Some(checkout), Some(path)) = (store.mesh_checkout(project, id), store.path(project))
        else {
            return;
        };
        let page = (project.clone(), id.clone());
        let picks = &mut self.mesh.picks;
        let picked = match picks.get(&page) {
            Some(Pick::Branch(name) | Pick::Commit(name, _))
                if checkout.branches.iter().any(|b| b.name == *name) =>
            {
                Some(name.clone())
            }
            _ => {
                picks.remove(&page);
                let first = checkout.branches.first().map(|b| b.name.clone());
                if let Some(first) = &first {
                    picks.insert(page.clone(), Pick::Branch(first.clone()));
                }
                first
            }
        };
        let mut wanted: Vec<_> = (checkout.branches.iter())
            .filter_map(|branch| {
                let mesh_id = branch.mesh_id.clone()?;
                let key = (project.clone(), id.clone(), branch.name.clone());
                let current = (self.mesh.fetches.get(&key)).is_some_and(|(tip, fetch)| {
                    *tip == branch.tip || matches!(fetch, Fetch::Running)
                });
                (!current).then(|| (key, path.clone(), mesh_id, branch.tip.clone()))
            })
            .collect();
        wanted.sort_by_key(|(key, ..)| Some(&key.2) != picked.as_ref());
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
            Some(Selection::Machine(project, id)) => Some((project, id)),
            None | Some(_) => None,
        };
        let next = (self.mesh.queue.iter())
            .position(|(key, ..)| {
                shown
                    .as_ref()
                    .is_some_and(|(p, i)| key.0 == *p && key.1 == *i)
            })
            .unwrap_or(0);
        let (key, path, mesh_id, tip) = self.mesh.queue.remove(next);
        self.mesh.running = true;
        self._mesh_fetch = cx.spawn(async move |this, cx| {
            let fetched = backend::run(move || mesh::fetch(&path, &mesh_id, tip.as_deref())).await;
            this.update(cx, |this, cx| {
                this.mesh.running = false;
                if let Some((_, fetch)) = this.mesh.fetches.get_mut(&key) {
                    *fetch = match fetched {
                        Ok(tip) => Fetch::Done(tip),
                        Err(err) => Fetch::Failed(format!("{err:#}").into()),
                    };
                }
                if matches!(this.selection(cx), Some(Selection::Machine(p, i)) if p == key.0 && i == key.1)
                {
                    this.refresh_diff(false, cx);
                }
                this.fetch_next(cx);
                cx.notify();
            })
            .ok();
        });
    }

    /// Show `project`'s checkout `id` with branch `name` picked, as clicking it in the
    /// navigator does.
    pub fn pick_machine_branch(
        &mut self,
        project: &ProjectId,
        id: &Arc<str>,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let page = (project.clone(), id.clone());
        self.mesh.picks.insert(page, Pick::Branch(name.to_owned()));
        self.show_picked(cx);
    }

    fn pick(&mut self, project: &ProjectId, id: &Arc<str>, pick: Pick, cx: &mut Context<Self>) {
        let page = (project.clone(), id.clone());
        if self.mesh.picks.get(&page) != Some(&pick) {
            self.mesh.picks.insert(page, pick);
            self.show_picked(cx);
        }
    }

    /// The branch picked on checkout `id`'s page, or holding the commit picked there.
    pub(super) fn machine_picked_branch(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
    ) -> Option<String> {
        match self.machine_pick(project, id)? {
            Pick::Branch(name) | Pick::Commit(name, _) => Some(name.clone()),
        }
    }

    fn machine_pick(&self, project: &ProjectId, id: &Arc<str>) -> Option<&Pick> {
        self.mesh.picks.get(&(project.clone(), id.clone()))
    }

    fn branch_fetch(&self, project: &ProjectId, id: &Arc<str>, name: &str) -> Option<&Fetch> {
        let key = (project.clone(), id.clone(), name.to_owned());
        self.mesh.fetches.get(&key).map(|(_, fetch)| fetch)
    }

    /// The diff of what's picked on checkout `id`'s page, once its branch is here.
    pub(super) fn machine_source(&self, project: &ProjectId, id: &Arc<str>) -> Option<DiffSource> {
        let (name, commit) = match self.machine_pick(project, id)? {
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

    /// What the diff pane says while the pick on checkout `id`'s page has no diff yet.
    pub(super) fn machine_waiting(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        cx: &gpui::App,
    ) -> SharedString {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return "".into();
        };
        let name = match self.machine_pick(project, id) {
            Some(Pick::Branch(name) | Pick::Commit(name, _)) => name,
            None if checkout.branches.is_empty() => {
                return "Nothing committed there yet. Its uncommitted files open on but.dev".into();
            }
            None => return "Pick a branch or a commit".into(),
        };
        let branch = checkout.branches.iter().find(|b| b.name == *name);
        match self.branch_fetch(project, id, name) {
            _ if branch.is_some_and(|b| b.mesh_id.is_none()) => {
                format!("The mesh keeps no copy of {name}").into()
            }
            Some(Fetch::Failed(err)) => format!("Couldn't fetch {name}: {err}").into(),
            Some(Fetch::Running) | None => {
                format!("Fetching {name} from {}…", checkout.machine).into()
            }
            Some(Fetch::Done(_)) => "".into(),
        }
    }

    pub(super) fn render_machine_side(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return empty_state(
                Some(Illustration::Papers),
                "Not on the mesh anymore",
                "That machine has nothing unfinished here now",
                theme,
                cx,
            );
        };
        let server = store.mesh.connection().map(|c| c.server.clone());
        let header = side_header(theme)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(icon(Icon::Globe).text_color(theme.text_1))
                    .child(title_text(checkout.label()).flex_1().truncate())
                    .children(server.as_ref().map(|server| {
                        let url = mesh::link(server, &checkout.id, None);
                        icon_button("machine-on-but-dev", Icon::Link)
                            .small()
                            .tooltip("Open on but.dev, with its uncommitted changes")
                            .on_click(move |_, _, cx| cx.open_url(&url))
                    })),
            )
            .child(
                h_flex()
                    .gap(px(12.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .child(dot(match checkout.online {
                                true => theme.safe,
                                false => theme.text_3,
                            }))
                            .child(match checkout.online {
                                true => "Online",
                                false => "Offline",
                            }),
                    )
                    .children(checkout.last_activity.map(|at| match just_now(at) {
                        true => "Active now".to_owned(),
                        false => format!("Active {} ago", relative_time(at)),
                    })),
            )
            .child(
                div()
                    .id("machine-path")
                    .text_size(px(12.))
                    .text_color(theme.text_3)
                    .truncate()
                    .tooltip(Tooltip::text(checkout.path.clone()))
                    .child(checkout.path.clone()),
            )
            .children(checkout.error.as_ref().map(|err| {
                div()
                    .text_size(px(12.))
                    .text_color(theme.warn)
                    .child(format!("The mesh couldn't read all of it: {err}"))
            }));
        let solid = self.files_focused(window);
        v_flex()
            .size_full()
            .child(header)
            .child(
                v_flex()
                    .id("machine-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .pb(px(16.))
                    .when(!checkout.uncommitted.is_empty(), |el| {
                        el.child(list_heading(
                            plural(checkout.uncommitted.len(), "uncommitted file"),
                            theme,
                        ))
                        .children(
                            checkout.uncommitted.iter().enumerate().map(|(ix, file)| {
                                let url =
                                    server.as_ref().map(|s| mesh::link(s, &checkout.id, None));
                                side_row(("machine-file", ix), false, false, theme)
                                    .tooltip(Tooltip::text(
                                        "Its diff is on that machine: open it on but.dev",
                                    ))
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
                                    .when_some(url, |el, url| {
                                        el.on_click(move |_, _, cx| cx.open_url(&url))
                                    })
                            }),
                        )
                    })
                    .children(self.machine_branches(project, id, checkout, solid, theme, cx)),
            )
            .into_any_element()
    }

    /// Each branch of `checkout`, with how its fetch stands, then its commits; the picked one,
    /// or the one holding the picked commit, is lit.
    fn machine_branches(
        &self,
        project: &ProjectId,
        id: &Arc<str>,
        checkout: &Checkout,
        solid: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if checkout.branches.is_empty() {
            return rows;
        }
        let noun = match checkout.branches.len() {
            1 => "1 branch".to_owned(),
            n => format!("{n} branches"),
        };
        rows.push(list_heading(noun, theme));
        let picked = self.machine_pick(project, id);
        for (bx, branch) in checkout.branches.iter().enumerate() {
            let (glyph, color) = branch_glyph(branch.status, theme);
            let unpushed = branch.commits.iter().filter(|c| !c.pushed).count();
            let fetch = self.branch_fetch(project, id, &branch.name);
            let here = matches!(fetch, Some(Fetch::Done(_)));
            let lit = picked == Some(&Pick::Branch(branch.name.clone()));
            let status = match fetch {
                Some(Fetch::Running) => Some(faint("fetching…", theme).into_any_element()),
                Some(Fetch::Failed(err)) => Some(
                    div()
                        .id(("machine-branch-failed", bx))
                        .child(small_icon(Icon::Warning).text_color(theme.warn))
                        .tooltip(Tooltip::text(format!("Couldn't fetch it: {err}")))
                        .into_any_element(),
                ),
                Some(Fetch::Done(_)) | None => None,
            };
            let (pick_project, pick_id, name) = (project.clone(), id.clone(), branch.name.clone());
            let menu = (project.clone(), id.clone(), branch.name.clone());
            rows.push(
                side_row(("machine-branch", bx), lit, solid, theme)
                    .mt(px(4.))
                    .child(icon(glyph).text_color(if lit && solid {
                        theme.text_inverted
                    } else {
                        color
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(branch.name.clone()),
                    )
                    .children(status)
                    .when(unpushed > 0, |el| {
                        el.child(
                            h_flex()
                                .gap(px(3.))
                                .text_size(px(12.))
                                .text_color(theme.blue)
                                .child(small_icon(Icon::ArrowUp).text_color(theme.blue))
                                .child(unpushed.to_string()),
                        )
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.pick(&pick_project, &pick_id, Pick::Branch(name.clone()), cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            let (project, id, name) = &menu;
                            this.pick(project, id, Pick::Branch(name.clone()), cx);
                            let items = this.machine_branch_menu(project, id, name, cx);
                            this.app
                                .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                                .ok();
                        }),
                    )
                    .into_any_element(),
            );
            rows.extend(branch.commits.iter().enumerate().map(|(ix, commit)| {
                let commit_id = gix::ObjectId::from_hex(commit.id.as_bytes())
                    .ok()
                    .map(CommitId);
                let pick = commit_id.map(|c| Pick::Commit(branch.name.clone(), c));
                let lit = pick.is_some() && picked == pick.as_ref();
                let row_id = SharedString::from(format!("machine-commit-{bx}-{ix}"));
                let (pick_project, pick_id) = (project.clone(), id.clone());
                side_row(ElementId::Name(row_id), lit, solid, theme)
                    .pl(px(32.))
                    .when(!here, |el| el.text_color(theme.text_2))
                    .child(small_icon(Icon::Commit).text_color(match commit.pushed {
                        true => theme.text_3,
                        false => theme.blue,
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(commit_title(&commit.title)),
                    )
                    .children(commit.author.as_ref().map(|author| {
                        div()
                            .id(ElementId::Name(format!("machine-author-{bx}-{ix}").into()))
                            .flex_none()
                            .child(person_avatar(author, 14., theme))
                            .tooltip(Tooltip::text(format!("Authored by {author}")))
                    }))
                    .child(age_column(commit.created_at, theme))
                    .when_some(pick, |el, pick| {
                        el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.pick(&pick_project, &pick_id, pick.clone(), cx)
                        }))
                    })
                    .into_any_element()
            }));
        }
        rows
    }

    /// What right-clicking a branch on a machine's page offers: bringing it here, and its page
    /// on but.dev.
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

/// What a branch on another machine shows as: merged, or still to finish.
pub(crate) fn branch_glyph(status: BranchStatus, theme: &Theme) -> (Icon, gpui::Hsla) {
    match status {
        BranchStatus::Integrated => (Icon::Tick, theme.safe),
        BranchStatus::Diverged => (Icon::Branch, theme.warn),
        BranchStatus::Pushed | BranchStatus::Unpushed | BranchStatus::Empty => {
            (Icon::Branch, theme.text_2)
        }
    }
}
