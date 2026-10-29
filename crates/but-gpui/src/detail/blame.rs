//! Going from a hunk or line of a workspace branch's diff to the commit that last changed it.

use gpui::Context;

use super::Detail;
use crate::backend::{self, CommitId, DiffSource, HunkKey, LineKind};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Selection};

/// What a menu's way to the last commit looks for: the lines a hunk adds, as its header's menu
/// has it, or the one added line right-clicked, by its number. Hunks close together are one,
/// so a line's own commit can be older than its hunk's.
#[derive(Clone, Copy)]
pub(super) enum Blamed {
    Hunk(HunkKey),
    Line(u32),
}

impl Detail {
    /// What the menu of `blamed` in file `path` offers in a workspace branch's diff: the
    /// commit that last changed its added lines, opened at that file.
    pub(super) fn last_commit_item(
        &self,
        path: &str,
        blamed: Blamed,
        cx: &mut Context<Self>,
    ) -> Option<MenuItem> {
        let diff = self.diff.as_ref()?;
        let DiffSource::Range { newest, .. } = diff.key.source else {
            return None;
        };
        let file = diff.files.iter().find(|f| f.file.path == path)?;
        let (label, lines) = match blamed {
            Blamed::Line(line) => ("Show line's last commit", vec![line]),
            Blamed::Hunk(key) => {
                let hunk = file.hunks().iter().find(|h| h.key == key)?;
                let added = (hunk.lines.iter())
                    .filter(|line| line.kind == LineKind::Added)
                    .filter_map(|line| line.new);
                ("Show hunk's last commit", added.collect::<Vec<u32>>())
            }
        };
        if lines.is_empty() {
            return None;
        }
        let (project, path, entity) = (
            diff.key.project.clone(),
            file.file.change.path.clone(),
            cx.entity().downgrade(),
        );
        Some(MenuItem::new(label, move |_, _, cx| {
            let (project, path, lines) = (project.clone(), path.clone(), lines.clone());
            (entity.update(cx, |detail, cx| {
                detail.show_last_commit(project, newest, path, lines, cx)
            }))
            .ok();
        }))
    }

    /// Select the newest of the workspace's commits that last changed `lines` of `path` as
    /// commit `tip` has it, at the file as that commit names it. Nothing happens once
    /// something else is selected meanwhile: the jump would take the user back. Once an agent
    /// rewrote `tip`, the commits blamed are the old ones, and it refuses.
    fn show_last_commit(
        &mut self,
        project: ProjectId,
        tip: CommitId,
        path: bstr::BString,
        lines: Vec<u32>,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.store.read(cx).path(&project) else {
            return;
        };
        let asked = self.selection(cx);
        let blamed = backend::run(move || backend::blame_lines(&repo, tip, path.as_ref(), &lines));
        cx.spawn(async move |this, cx| {
            let blamed = blamed.await;
            this.update(cx, |this, cx| {
                if this.selection(cx) != asked {
                    return;
                }
                let paths = match blamed {
                    Ok(paths) => paths,
                    Err(err) => {
                        return this.store.update(cx, |store, cx| {
                            store.failed("Couldn't find the hunk's last commit", &err, cx)
                        });
                    }
                };
                let ws = this.store.read(cx).workspace(&project);
                let current = ws.is_some_and(|ws| ws.find_commit(tip).is_some());
                // Each branch lists its commits newest first, and the hunk's are all in one lane.
                let newest = ws.and_then(|ws| {
                    ws.commits()
                        .find_map(|commit| Some((commit.id, paths.get(&commit.id)?.clone())))
                });
                if !current {
                    let gone = "The branch was rewritten meanwhile, so try again";
                    return crate::state::snack(&this.store, gone, cx);
                }
                let Some((id, path)) = newest else {
                    return crate::state::snack(
                        &this.store,
                        "Last changed outside the workspace",
                        cx,
                    );
                };
                this.pending_file = Some(path);
                this.store.update(cx, |store, cx| {
                    store.select(Some(Selection::Commit(project, id)), cx)
                });
            })
            .ok();
        })
        .detach();
    }
}
