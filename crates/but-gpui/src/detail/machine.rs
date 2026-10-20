//! A project's Other machines page: each of its checkouts on the user's other machines, as the
//! mesh reports them, with their uncommitted files and their branches with their commits,
//! listed beside a diff. Opening it fetches those branches from the copy the mesh keeps, one at
//! a time, so a branch or one of its commits shows its diff as a branch of this machine's
//! would. Uncommitted files' diffs stay on their machine, and open on but.dev.

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
    /// The pick on each project's page, with the checkout it's in, kept while another page
    /// shows.
    picks: HashMap<ProjectId, (Arc<str>, Pick)>,
}

impl Detail {
    /// Fetch the branches of `project`'s checkouts elsewhere that have no copy here as their
    /// machine reported them, the picked one first, and pick the first branch if nothing is.
    pub(super) fn load_machines(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(path) = store.path(project) else {
            return;
        };
        let checkouts = store.mesh_checkouts(project);
        let listed = |id: &str, name: &str| {
            (checkouts.iter()).any(|c| c.id == id && c.branches.iter().any(|b| b.name == name))
        };
        let picks = &mut self.mesh.picks;
        let picked = match picks.get(project) {
            Some((id, Pick::Branch(name) | Pick::Commit(name, _))) if listed(id, name) => {
                Some((id.clone(), name.clone()))
            }
            _ => {
                picks.remove(project);
                let first = (checkouts.iter()).find_map(|c| {
                    let id: Arc<str> = c.id.as_str().into();
                    Some((id, c.branches.first()?.name.clone()))
                });
                if let Some((id, name)) = &first {
                    picks.insert(project.clone(), (id.clone(), Pick::Branch(name.clone())));
                }
                first
            }
        };
        let mut wanted: Vec<_> = (checkouts.iter())
            .flat_map(|checkout| {
                let id: Arc<str> = checkout.id.as_str().into();
                checkout
                    .branches
                    .iter()
                    .map(move |branch| (id.clone(), branch))
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
                if matches!(this.selection(cx), Some(Selection::Machines(p)) if p == key.0) {
                    this.refresh_diff(false, cx);
                }
                this.fetch_next(cx);
                cx.notify();
            })
            .ok();
        });
    }

    fn pick(&mut self, project: &ProjectId, id: &Arc<str>, pick: Pick, cx: &mut Context<Self>) {
        let picked = (id.clone(), pick);
        if self.mesh.picks.get(project) != Some(&picked) {
            self.mesh.picks.insert(project.clone(), picked);
            self.show_picked(cx);
        }
    }

    /// The checkout of `project`'s page and the branch picked there, or holding the commit
    /// picked there.
    pub(super) fn machine_picked_branch(&self, project: &ProjectId) -> Option<(Arc<str>, String)> {
        let (id, Pick::Branch(name) | Pick::Commit(name, _)) = self.mesh.picks.get(project)?;
        Some((id.clone(), name.clone()))
    }

    /// The pick on `project`'s page, if it's in checkout `id`.
    fn machine_pick(&self, project: &ProjectId, id: &Arc<str>) -> Option<&Pick> {
        let (picked_in, pick) = self.mesh.picks.get(project)?;
        (picked_in == id).then_some(pick)
    }

    fn branch_fetch(&self, project: &ProjectId, id: &Arc<str>, name: &str) -> Option<&Fetch> {
        let key = (project.clone(), id.clone(), name.to_owned());
        self.mesh.fetches.get(&key).map(|(_, fetch)| fetch)
    }

    /// The diff of what's picked on `project`'s page, once its branch is here.
    pub(super) fn machine_source(&self, project: &ProjectId) -> Option<DiffSource> {
        let (id, pick) = self.mesh.picks.get(project)?;
        let (name, commit) = match pick {
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

    /// What the diff pane says while the pick on `project`'s page has no diff yet.
    pub(super) fn machine_waiting(&self, project: &ProjectId, cx: &gpui::App) -> SharedString {
        let store = self.store.read(cx);
        let checkouts = store.mesh_checkouts(project);
        let Some((id, Pick::Branch(name) | Pick::Commit(name, _))) = self.mesh.picks.get(project)
        else {
            return match checkouts.iter().all(|c| c.branches.is_empty()) {
                // The list says why it's empty.
                _ if checkouts.is_empty() => "".into(),
                true => "Nothing committed there yet. Uncommitted files open on but.dev".into(),
                false => "Pick a branch or a commit".into(),
            };
        };
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return "".into();
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

    pub(super) fn render_machines_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let connection = store.mesh.connection();
        let header = side_header(theme)
            .child(title_text("Other machines"))
            .children(
                connection
                    .map(|c| faint(format!("Signed in as {} on {}", c.login, c.machine), theme)),
            );
        let checkouts = store.mesh_checkouts(project);
        if checkouts.is_empty() {
            let empty = match store.mesh.read(project) {
                true => empty_state(
                    Some(Illustration::Papers),
                    "Nothing unfinished elsewhere",
                    "Work on this project on your other machines shows here",
                    theme,
                    cx,
                ),
                false => empty_state(None, "Reading your other machines…", "", theme, cx),
            };
            return v_flex()
                .size_full()
                .child(header)
                .child(empty)
                .into_any_element();
        }
        let server = connection.map(|c| c.server.clone());
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
                    .children(checkouts.iter().enumerate().map(|(ix, checkout)| {
                        let id: Arc<str> = checkout.id.as_str().into();
                        // Its own id keeps its rows' ids apart from another checkout's.
                        v_flex()
                            .id(("checkout", ix))
                            .child(checkout_heading(checkout, server.as_deref(), theme))
                            .children(uncommitted_rows(checkout, server.as_deref(), theme))
                            .children(
                                self.machine_branches(project, &id, checkout, solid, theme, cx),
                            )
                    })),
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

/// Over a checkout's rows: the machine (and worktree) it's on, whether that's online, when it
/// last changed and where it is, with a link to it on but.dev.
fn checkout_heading(checkout: &Checkout, server: Option<&str>, theme: &Theme) -> AnyElement {
    let active = checkout.online && checkout.last_activity.is_some_and(just_now);
    let (color, status) = match (active, checkout.online) {
        (true, _) => (theme.safe, "Online"),
        (false, true) => (theme.text_3, "Online"),
        (false, false) => (theme.text_3.opacity(0.4), "Offline"),
    };
    let activity = checkout.last_activity.map(|at| match just_now(at) {
        true => "active now".to_owned(),
        false => format!("active {} ago", relative_time(at)),
    });
    v_flex()
        .px(px(14.))
        .pt(px(14.))
        .pb(px(2.))
        .gap(px(2.))
        .child(
            h_flex()
                .gap(px(8.))
                .child(icon(Icon::Globe).text_color(theme.text_2))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(checkout.label()),
                )
                .children(server.map(|server| {
                    let url = mesh::link(server, &checkout.id, None);
                    icon_button("machine-on-but-dev", Icon::Link)
                        .small()
                        .tooltip("Open on but.dev, with its uncommitted changes")
                        .on_click(move |_, _, cx| cx.open_url(&url))
                })),
        )
        .child(
            h_flex()
                .gap(px(5.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(dot(color))
                .child(match activity {
                    Some(activity) => format!("{status}, {activity}"),
                    None => status.to_owned(),
                }),
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
        }))
        .into_any_element()
}

/// A checkout's uncommitted files, under a heading. Their diffs are on that machine, so a click
/// opens the checkout on but.dev.
fn uncommitted_rows(checkout: &Checkout, server: Option<&str>, theme: &Theme) -> Vec<AnyElement> {
    if checkout.uncommitted.is_empty() {
        return Vec::new();
    }
    let heading = list_heading(
        plural(checkout.uncommitted.len(), "uncommitted file"),
        theme,
    );
    let url = server.map(|server| mesh::link(server, &checkout.id, None));
    let files = checkout.uncommitted.iter().enumerate().map(|(ix, file)| {
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
            .when_some(url.clone(), |el, url| {
                el.on_click(move |_, _, cx| cx.open_url(&url))
            })
            .into_any_element()
    });
    std::iter::once(heading).chain(files).collect()
}

/// What a branch on another machine shows as: merged, or still to finish.
fn branch_glyph(status: BranchStatus, theme: &Theme) -> (Icon, gpui::Hsla) {
    match status {
        BranchStatus::Integrated => (Icon::Tick, theme.safe),
        BranchStatus::Diverged => (Icon::Branch, theme.warn),
        BranchStatus::Pushed | BranchStatus::Unpushed | BranchStatus::Empty => {
            (Icon::Branch, theme.text_2)
        }
    }
}
