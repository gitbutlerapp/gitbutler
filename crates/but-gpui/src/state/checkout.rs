use std::collections::HashSet;
use std::path::PathBuf;

use gpui::SharedString;

use super::{ProjectId, Section, Selection, Store};
use crate::backend::{Checkout, CommitId, Lane, Worktree};

impl Store {
    /// The lane `selection` is in, and the place from the top of the branch it's on: a
    /// branch's, a commit's, a remote commit's of a branch, or a stack's by its bottom branch.
    pub fn lane_for(&self, selection: &Selection) -> Option<(&Lane, usize)> {
        let workspace = self.workspace(selection.project())?;
        match selection {
            Selection::Branch(_, ref_name) | Selection::Stack(_, ref_name) => {
                workspace.lane_of(ref_name.as_ref())
            }
            Selection::Upstream(_, Some(ref_name), _)
            | Selection::Section(_, Section::Upstream(Some(ref_name))) => {
                workspace.lane_of(Some(ref_name))
            }
            Selection::Commit(_, id) => workspace.lane_holding(*id),
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Graph(_)
            | Selection::Worktree(..)
            | Selection::Mesh(..)
            | Selection::Upstream(_, None, _)
            | Selection::Section(..)
            | Selection::Terminal(..) => None,
        }
    }

    /// The checkout `selection` is in: whose uncommitted changes it is, the one a terminal is
    /// open on, or that of the lane it's in.
    pub fn checkout_of(&self, selection: &Selection) -> Option<Checkout> {
        match selection {
            Selection::Uncommitted(_) => Some(Checkout::Main),
            Selection::Worktree(_, name) => Some(Checkout::Linked(name.clone())),
            Selection::Terminal(..) => self.terminals.checkout_of(selection).cloned(),
            _ => Some(self.lane_for(selection)?.0.checkout.clone()),
        }
    }

    /// Whether `selection` is in a linked worktree's lane.
    pub fn in_linked(&self, selection: &Selection) -> bool {
        (self.checkout_of(selection).as_ref()).is_some_and(Checkout::is_linked)
    }

    /// The checkouts rewriting commits `ids` of `project` moves.
    pub fn moved_by(&self, project: &ProjectId, ids: &[CommitId]) -> HashSet<Checkout> {
        (self.workspace(project).into_iter())
            .flat_map(|ws| ids.iter().flat_map(|id| ws.moved_by(*id)))
            .collect()
    }

    /// Whether GitButler's history can take back what moves `checkouts`: it records only the
    /// main checkout, and nothing while the project is unrecorded.
    pub fn undoable(
        &self,
        project: &ProjectId,
        checkouts: impl IntoIterator<Item = Checkout>,
    ) -> bool {
        !(self.workspace(project)).is_some_and(|ws| ws.unrecorded)
            && checkouts
                .into_iter()
                .all(|checkout| checkout == Checkout::Main)
    }

    /// The linked worktree `selection` is in.
    pub fn worktree_for(&self, selection: &Selection) -> Option<&Worktree> {
        let checkout = self.checkout_of(selection)?;
        let name = checkout.linked()?;
        let workspace = self.workspace(selection.project())?;
        workspace.worktrees.iter().find(|wt| *wt.name == *name)
    }

    /// The folder of the checkout `selection` is in: a linked worktree's, or the main one's.
    /// `None` for another machine's checkout.
    pub fn checkout_dir(&self, selection: &Selection) -> Option<PathBuf> {
        let project = selection.project();
        match selection {
            Selection::Worktree(_, name) => {
                let worktree = self.workspace(project)?.find_worktree(name.as_ref())?;
                Some(worktree.path.clone())
            }
            Selection::Mesh(..) => None,
            _ => match self.worktree_for(selection) {
                Some(worktree) => Some(worktree.path.clone()),
                None => self.path(project),
            },
        }
    }

    /// Where an action on `at` happens, for a prompt or a confirmation to say, named as the
    /// breadcrumbs name it: the project, then the linked worktree or other machine's
    /// checkout when it isn't the main one, as "shop › agent-x".
    pub fn place(&self, at: &Selection) -> SharedString {
        let project = at.project();
        let name = self
            .project(project)
            .map_or("", |entry| &entry.project.name);
        let elsewhere = match at {
            Selection::Worktree(_, worktree) => Some(worktree.to_string()),
            Selection::Mesh(_, id, _) => self.mesh_checkout(project, id).map(|c| c.label()),
            _ => self
                .worktree_for(at)
                .map(|worktree| worktree.name.to_string()),
        };
        match elsewhere {
            Some(elsewhere) => format!("{name} › {elsewhere}").into(),
            None => name.to_owned().into(),
        }
    }

    /// Project `id`'s linked worktrees with nothing going on lately, but those on their way
    /// out.
    pub fn inactive_worktrees(&self, id: &ProjectId) -> Vec<&Worktree> {
        (self.workspace(id).into_iter().flat_map(|ws| &ws.worktrees))
            .filter(|wt| !self.hides_worktree(id, wt.name.as_ref()) && wt.is_inactive())
            .collect()
    }
}
