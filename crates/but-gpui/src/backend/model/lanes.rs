use std::collections::HashSet;

use super::{Branch, Commit, CommitId, Workspace};

/// The checkout a lane's branches are worked on in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum Checkout {
    #[default]
    Main,
    Linked(bstr::BString),
}

#[derive(Debug, Clone)]
pub struct Lane {
    /// Branches from the top of the lane down to its base. A linked worktree's run from the
    /// one checked out down to the workspace's target commit or another lane's commit.
    pub branches: Vec<Branch>,
    pub checkout: Checkout,
    /// The commit in another lane its bottom branch starts from: `None` for a stack, and for a
    /// linked worktree started outside the workspace.
    pub rests_on: Option<CommitId>,
}

/// A lane from its branch `at` down, entered at `from` when what rests on it starts below that
/// branch's tip.
#[derive(Debug, Clone, Copy)]
pub struct Link<'a> {
    pub lane: &'a Lane,
    pub at: usize,
    pub from: Option<CommitId>,
}

impl Checkout {
    pub fn linked(&self) -> Option<&bstr::BStr> {
        match self {
            Checkout::Main => None,
            Checkout::Linked(name) => Some(name.as_ref()),
        }
    }

    pub fn is_linked(&self) -> bool {
        matches!(self, Checkout::Linked(_))
    }
}

impl Lane {
    /// The last commit of its bottom named branch, which a branch moves below to go under the
    /// lane's branches. `None` when that branch has no commits of its own.
    pub fn end(&self) -> Option<CommitId> {
        let bottom = (self.branches.iter().rev()).find(|branch| branch.ref_name.is_some())?;
        bottom.commits.last().map(|commit| commit.id)
    }
}

impl<'a> Link<'a> {
    /// Its branches from `at` down, whole.
    pub fn branches(self) -> &'a [Branch] {
        self.lane.branches.get(self.at..).unwrap_or_default()
    }

    /// Its commits from `from` down, newest first.
    pub fn commits(self) -> impl Iterator<Item = &'a Commit> {
        let from = self.from;
        (self.branches().iter())
            .flat_map(|branch| &branch.commits)
            .skip_while(move |commit| from.is_some_and(|from| commit.id != from))
    }

    pub fn top(self) -> Option<(&'a Branch, Option<CommitId>)> {
        Some((self.branches().first()?, self.from))
    }
}

impl Workspace {
    /// The lane of the linked worktree `name`.
    pub fn worktree_lane(&self, name: &bstr::BStr) -> Option<&Lane> {
        (self.lanes.iter())
            .find(|lane| matches!(&lane.checkout, Checkout::Linked(linked) if linked == name))
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
    pub fn lane(&self, bottom: Option<&super::FullName>) -> Option<&Lane> {
        (self.lanes.iter()).find(|lane| {
            (lane.branches.last()).is_some_and(|branch| branch.ref_name.as_ref() == bottom)
        })
    }

    /// The lane holding the branch `ref_name`, a stack's or a linked worktree's, and the
    /// branch's place in it from the top.
    pub fn lane_of(&self, ref_name: Option<&super::FullName>) -> Option<(&Lane, usize)> {
        self.lanes.iter().find_map(|lane| {
            let at = (lane.branches.iter()).position(|b| b.ref_name.as_ref() == ref_name)?;
            Some((lane, at))
        })
    }

    /// The lane holding commit `id`, and the place from the top of the branch holding it.
    pub fn lane_holding(&self, id: CommitId) -> Option<(&Lane, usize)> {
        self.lanes.iter().find_map(|lane| {
            let at = (lane.branches.iter())
                .position(|b| b.commits.iter().any(|commit| commit.id == id))?;
            Some((lane, at))
        })
    }

    /// The lanes `lane` rests on, nearest first. A commit no lane holds ends it, and so does
    /// going down more lanes than there are.
    pub fn beneath<'a>(&'a self, lane: &'a Lane) -> impl Iterator<Item = Link<'a>> {
        let entered = move |rests_on: Option<CommitId>| {
            let id = rests_on?;
            let (lane, at) = self.lane_holding(id)?;
            let tip = lane.branches.get(at)?.commits.first()?.id;
            let from = (tip != id).then_some(id);
            Some(Link { lane, at, from })
        };
        let links = std::iter::successors(entered(lane.rests_on), move |link| {
            entered(link.lane.rests_on)
        });
        links.take(self.lanes.len())
    }

    /// The lane of the branch `ref_name` from it down, then the lanes beneath. Empty for one
    /// no lane holds.
    pub fn chain(&self, ref_name: &super::FullName) -> impl Iterator<Item = Link<'_>> {
        let found = self.lane_of(Some(ref_name));
        found
            .into_iter()
            .flat_map(|(lane, at)| self.chain_from(lane, at))
    }

    /// `lane` from its branch `at` down, then the lanes beneath.
    pub fn chain_from<'a>(&'a self, lane: &'a Lane, at: usize) -> impl Iterator<Item = Link<'a>> {
        let own = Link {
            lane,
            at,
            from: None,
        };
        std::iter::once(own).chain(self.beneath(lane))
    }

    /// Whether another lane rests on a commit of the branch `ref_name`.
    pub fn rested_on(&self, ref_name: &super::FullName) -> bool {
        let Some(branch) = self.find_branch(Some(ref_name)) else {
            return false;
        };
        (self.lanes.iter())
            .filter_map(|lane| lane.rests_on)
            .any(|id| branch.commits.iter().any(|commit| commit.id == id))
    }

    /// The checkouts rewriting commit `id` moves: the one whose lane holds it, and those of the
    /// linked worktrees resting on it or on a commit above it, through any lanes between.
    pub fn moved_by(&self, id: CommitId) -> HashSet<Checkout> {
        (self.lanes.iter())
            .filter(|lane| {
                (self.chain_from(lane, 0).flat_map(Link::commits)).any(|commit| commit.id == id)
            })
            .map(|lane| lane.checkout.clone())
            .collect()
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
    pub fn stack_of(&self, ref_name: Option<&super::FullName>) -> Option<(&Lane, usize)> {
        self.lanes_in(&Checkout::Main).find_map(|stack| {
            let at = (stack.branches.iter()).position(|b| b.ref_name.as_ref() == ref_name)?;
            Some((stack, at))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::build::{branch, describe_chain, full, workspace, worktree};

    #[test]
    fn a_chain_follows_each_lane_down_to_what_it_rests_on() {
        let ws = workspace(
            vec![vec![
                branch("top", &[1]),
                branch("middle", &[2, 3]),
                branch("bottom", &[4]),
            ]],
            vec![
                worktree("wt", Some(3), vec![branch("wt", &[5])]),
                worktree("wt-stacked", Some(5), vec![branch("wt-stacked", &[6])]),
                worktree("outside", None, vec![branch("outside", &[7])]),
                worktree("dangling", Some(42), vec![branch("dangling", &[8])]),
            ],
        );
        let cycle = workspace(
            vec![],
            vec![
                worktree("a", Some(2), vec![branch("a", &[1])]),
                worktree("b", Some(1), vec![branch("b", &[2])]),
            ],
        );
        let chains: String = [
            (&ws, "wt"),
            (&ws, "wt-stacked"),
            (&ws, "outside"),
            (&ws, "dangling"),
            (&ws, "middle"),
            (&cycle, "a"),
        ]
        .map(|(ws, name)| {
            let chain = describe_chain(ws, &full(&format!("refs/heads/{name}")));
            format!("chain({name})\n{chain}\n")
        })
        .concat();
        snapbox::assert_data_eq!(
            chains,
            snapbox::str![[r#"
chain(wt)
wt: [wt]
Main: [middle, bottom] from middle~1
history: [wt, middle~1, bottom]

chain(wt-stacked)
wt-stacked: [wt-stacked]
wt: [wt]
Main: [middle, bottom] from middle~1
history: [wt-stacked, wt, middle~1, bottom]

chain(outside)
outside: [outside]
history: [outside]

chain(dangling)
dangling: [dangling]
history: [dangling]

chain(middle)
Main: [middle, bottom]
history: [middle, middle~1, bottom]

chain(a)
a: [a]
b: [b]
a: [a]
history: [a, b, a]


"#]]
        );
    }
}
