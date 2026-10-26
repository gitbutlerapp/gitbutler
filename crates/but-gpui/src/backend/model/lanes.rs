use std::sync::Arc;

use super::{Branch, CommitId, Workspace};

/// The checkout a lane's branches are worked on in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Checkout {
    Main,
    Linked(Arc<str>),
}

#[derive(Debug, Clone)]
pub struct Lane {
    /// Branches from the top of the lane down to its base. A linked worktree's run from the
    /// one checked out down to where it started, without the commits the target already has:
    /// what was committed in it.
    pub branches: Vec<Branch>,
    pub checkout: Checkout,
}

impl Checkout {
    pub fn linked(&self) -> Option<&Arc<str>> {
        match self {
            Checkout::Main => None,
            Checkout::Linked(name) => Some(name),
        }
    }
}

impl From<Option<Arc<str>>> for Checkout {
    fn from(linked: Option<Arc<str>>) -> Self {
        linked.map_or(Checkout::Main, Checkout::Linked)
    }
}

impl Lane {
    /// The last commit of its bottom named branch, which a branch moves below to go under the
    /// lane's branches. `None` when that branch has no commits of its own.
    pub fn end(&self) -> Option<CommitId> {
        let bottom = (self.branches.iter().rev()).find(|branch| !branch.ref_name.is_empty())?;
        bottom.commits.last().map(|commit| commit.id)
    }
}

impl Workspace {
    /// The lane of the linked worktree `name`.
    pub fn worktree_lane(&self, name: &str) -> Option<&Lane> {
        (self.lanes.iter())
            .find(|lane| matches!(&lane.checkout, Checkout::Linked(linked) if **linked == *name))
    }

    /// The commits of the lane holding commit `id`, across its branches, newest first.
    pub fn lane_commits(&self, id: CommitId) -> Option<Vec<CommitId>> {
        (self.lanes.iter())
            .map(|lane| lane.branches.iter().flat_map(|b| &b.commits).map(|c| c.id))
            .map(Vec::from_iter)
            .find(|lane| lane.contains(&id))
    }

    /// The newest and oldest of `ids`, when they're two or more and one run of commits in one
    /// lane, across its branches too, so their changes together are a diff from the oldest's
    /// parent.
    pub fn run_of(&self, ids: &[CommitId]) -> Option<(CommitId, CommitId)> {
        let n = ids.len();
        let lane = self.lane_commits(*ids.first()?)?;
        let from = lane.iter().position(|id| ids.contains(id))?;
        let run = lane.get(from..from + n)?;
        let [newest, .., oldest] = run else {
            return None;
        };
        (run.iter().all(|id| ids.contains(id))).then_some((*newest, *oldest))
    }

    /// The stack, or linked worktree's lane, whose bottom branch is `bottom`.
    pub fn lane(&self, bottom: &str) -> Option<&Lane> {
        (self.lanes.iter())
            .find(|lane| (lane.branches.last()).is_some_and(|branch| branch.ref_name == bottom))
    }

    /// The lane holding the branch `ref_name`, a stack's or a linked worktree's, and the
    /// branch's place in it from the top.
    pub fn lane_of(&self, ref_name: &str) -> Option<(&Lane, usize)> {
        self.lanes.iter().find_map(|lane| {
            let at = lane.branches.iter().position(|b| b.ref_name == ref_name)?;
            Some((lane, at))
        })
    }

    /// The lanes worked on in `checkout`.
    pub fn lanes_in(&self, checkout: &Checkout) -> impl Iterator<Item = &Lane> + Clone + use<'_> {
        let checkout = checkout.clone();
        (self.lanes.iter()).filter(move |lane| lane.checkout == checkout)
    }

    /// The branches of the lanes worked on in `checkout`, top first.
    pub fn branches_in(
        &self,
        checkout: &Checkout,
    ) -> impl Iterator<Item = &Branch> + Clone + use<'_> {
        self.lanes_in(checkout).flat_map(|lane| &lane.branches)
    }

    /// Every branch shown: the stacks', then the linked worktrees'.
    pub fn all_branches(&self) -> impl Iterator<Item = &Branch> {
        self.lanes.iter().flat_map(|lane| &lane.branches)
    }

    /// The stack holding the branch `ref_name`, and the branch's place in it from the top.
    pub fn stack_of(&self, ref_name: &str) -> Option<(&Lane, usize)> {
        self.lanes_in(&Checkout::Main).find_map(|stack| {
            let at = stack.branches.iter().position(|b| b.ref_name == ref_name)?;
            Some((stack, at))
        })
    }
}
