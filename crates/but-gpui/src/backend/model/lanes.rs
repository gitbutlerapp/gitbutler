use super::{Branch, CommitId, Workspace};

/// The checkout a lane's branches are worked on in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Checkout {
    Main,
}

#[derive(Debug, Clone)]
pub struct Lane {
    /// Branches from the top of the lane down to its base.
    pub branches: Vec<Branch>,
    pub checkout: Checkout,
}

impl Workspace {
    /// The branches of each stack, then of each linked worktree's lane, top first.
    pub fn lanes(&self) -> impl Iterator<Item = &[Branch]> {
        (self.lanes.iter().map(|lane| &lane.branches[..]))
            .chain(self.worktrees.iter().map(|wt| &wt.branches[..]))
    }

    /// The commits of the lane holding commit `id`, across its branches, newest first.
    pub fn lane_commits(&self, id: CommitId) -> Option<Vec<CommitId>> {
        (self.lanes())
            .map(|lane| lane.iter().flat_map(|b| &b.commits).map(|c| c.id))
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

    /// The branches of the stack, or linked worktree's lane, whose bottom branch is `bottom`.
    pub fn lane(&self, bottom: &str) -> Option<&[Branch]> {
        self.lanes().find(|branches| {
            branches
                .last()
                .is_some_and(|branch| branch.ref_name == bottom)
        })
    }

    /// The lane holding the branch `ref_name`, a stack's or a linked worktree's, and the
    /// branch's place in it from the top.
    pub fn lane_of(&self, ref_name: &str) -> Option<(&[Branch], usize)> {
        self.lanes().find_map(|lane| {
            let at = lane.iter().position(|b| b.ref_name == ref_name)?;
            Some((lane, at))
        })
    }

    /// The lanes worked on in `checkout`.
    pub fn lanes_in<'a>(
        &'a self,
        checkout: &'a Checkout,
    ) -> impl Iterator<Item = &'a Lane> + Clone {
        (self.lanes.iter()).filter(move |lane| lane.checkout == *checkout)
    }

    /// The stack holding the branch `ref_name`, and the branch's place in it from the top.
    pub fn stack_of(&self, ref_name: &str) -> Option<(&Lane, usize)> {
        self.lanes_in(&Checkout::Main).find_map(|stack| {
            let at = stack.branches.iter().position(|b| b.ref_name == ref_name)?;
            Some((stack, at))
        })
    }
}
