//! What the file watcher saw written in each checkout lately, for a worktree's overview: how
//! busy it's been a quarter of an hour at a time over the last week, which a restart keeps.
//! And the commits made there, so those of a branch that left, as merged or unapplied, still
//! count.

use std::collections::{HashSet, VecDeque};

use gpui::Context;
use serde::{Deserialize, Serialize};

use super::{ProjectId, Store};
use crate::backend::{Branch, Checkout, CommitId, CommitState, ReviewState, Workspace};

/// How long one bar of activity covers, in seconds.
pub const QUARTER: i64 = 15 * 60;
/// How far back activity is kept, in seconds: a week, at most 672 quarters a checkout.
pub(super) const KEPT_FOR: i64 = 7 * 24 * 3600;
/// How long commits are remembered, in seconds: a day, a little more than the overview shows,
/// as the whole state is saved again with every few writes.
const COMMITS_KEPT_FOR: i64 = 24 * 3600;

/// One checkout's writes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Activity {
    /// How many files were written in each quarter of an hour, by when it began in seconds
    /// since the epoch, oldest first; quarters without any are left out.
    pub quarters: VecDeque<(i64, u32)>,
    /// When the last file was written, in seconds since the epoch.
    pub last: Option<i64>,
    /// The commits of the last day on the checkout's branches, as last seen there, newest
    /// first.
    #[serde(default)]
    pub commits: Vec<SeenCommit>,
}

/// A commit as a checkout last had it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeenCommit {
    pub id: CommitId,
    /// Its change ID, which a branch renamed or a commit moved to another keeps, and its
    /// [`authored`](crate::backend::Commit::authored), which a rebase by Git keeps too.
    pub key: String,
    pub authored: String,
    pub title: String,
    pub time: i64,
    /// Landed once its branch's pull request merged.
    pub state: CommitState,
    /// The branch it was on, by full ref name or `None` when unnamed, and that branch's pull
    /// request.
    pub branch: Option<String>,
    pub pull: Option<u64>,
}

impl Activity {
    /// The commits remembered that the checkout's `branches` don't have: those of a branch
    /// gone since, as merged or unapplied, as last seen, but for any one of `branches` has
    /// now by either of what it goes by, as one renamed does.
    pub fn gone(&self, branches: &[&Branch]) -> Vec<&SeenCommit> {
        let refs: HashSet<Option<String>> = (branches.iter())
            .map(|b| b.ref_name.as_ref().map(ToString::to_string))
            .collect();
        let keys: HashSet<&str> = (branches.iter().flat_map(|b| &b.commits))
            .flat_map(|c| [c.key.as_str(), c.authored.as_str()])
            .collect();
        let here = |c: &SeenCommit| keys.contains(&*c.key) || keys.contains(&*c.authored);
        (self.commits.iter())
            .filter(|c| !refs.contains(&c.branch) && !here(c))
            .collect()
    }
}

/// What to remember of a checkout as of `now`, with its `branches` as they are: their
/// commits, and what `kept` has of branches [gone](Activity::gone) since, of the last day.
fn remembered(kept: Option<&Activity>, now: i64, branches: &[&Branch]) -> Vec<SeenCommit> {
    let since = now - COMMITS_KEPT_FOR;
    let gone = kept.map_or(Vec::new(), |kept| kept.gone(branches));
    let mut commits: Vec<SeenCommit> = (gone.into_iter())
        .filter(|c| c.time > since)
        .cloned()
        .collect();
    commits.extend(branches.iter().flat_map(|branch| {
        let merged = (branch.review.as_ref()).is_some_and(|r| r.state == ReviewState::Merged);
        (branch.commits.iter().filter(|c| c.time > since)).map(move |c| SeenCommit {
            id: c.id,
            key: c.key.clone(),
            authored: c.authored.clone(),
            title: c.title.clone(),
            time: c.time,
            state: if merged {
                CommitState::Integrated
            } else {
                c.state
            },
            branch: branch.ref_name.as_ref().map(ToString::to_string),
            pull: branch.review.as_ref().map(|r| r.number),
        })
    }));
    commits.sort_by_key(|c| std::cmp::Reverse(c.time));
    commits
}

impl Store {
    /// When something other than the app last wrote to `checkout` of `project`: what the file
    /// watcher saw, or an uncommitted file's time, as of one written before the app started.
    pub fn last_write(&self, project: &ProjectId, checkout: &Checkout) -> Option<i64> {
        let files = (self.workspace(project)).and_then(|ws| ws.uncommitted_in(checkout));
        let written = (files.unwrap_or_default().iter())
            .filter_map(|file| file.modified.map(|ms| ms / 1000))
            .filter(|at| !self.own_write(project, *at))
            .max();
        let activity = self.activity.get(&(project.clone(), checkout.clone()));
        activity.and_then(|a| a.last).max(written)
    }

    /// Whether `checkout` of `project` is being written: written to just now, and since its
    /// newest commit, a branch's that left since too, as committing makes it quiet again.
    pub fn writing(&self, project: &ProjectId, checkout: &Checkout) -> bool {
        let Some(workspace) = self.workspace(project) else {
            return false;
        };
        let gone = (self.activity.get(&(project.clone(), checkout.clone())))
            .and_then(|activity| activity.commits.first())
            .map(|commit| commit.time);
        let newest = (workspace.branches_in(checkout))
            .flat_map(|branch| &branch.commits)
            .map(|commit| commit.time)
            .max()
            .max(gone);
        (self.last_write(project, checkout))
            .is_some_and(|at| crate::ui::just_now(at) && newest < Some(at))
    }

    /// The checkouts of `project` being [written](Store::writing): the main one first, then
    /// the linked worktrees.
    pub fn being_written(&self, project: &ProjectId) -> Vec<Checkout> {
        let Some(workspace) = self.workspace(project) else {
            return Vec::new();
        };
        let linked = (workspace.worktrees.iter()).map(|w| Checkout::Linked(w.name.clone()));
        (std::iter::once(Checkout::Main).chain(linked))
            .filter(|checkout| self.writing(project, checkout))
            .collect()
    }

    /// Remember the commits each checkout of `workspace` has, as [`remembered`] keeps them.
    pub(super) fn remember_commits(
        &mut self,
        id: &ProjectId,
        workspace: &Workspace,
        cx: &mut Context<Self>,
    ) {
        let now = crate::ui::now();
        let linked = (workspace.worktrees.iter()).map(|w| Checkout::Linked(w.name.clone()));
        let mut changed = false;
        for checkout in std::iter::once(Checkout::Main).chain(linked) {
            let branches: Vec<&Branch> = workspace.branches_in(&checkout).collect();
            let checkout = (id.clone(), checkout);
            let kept = self.activity.get(&checkout);
            let commits = remembered(kept, now, &branches);
            if commits != kept.map_or(&[][..], |a| &a.commits) {
                self.activity.entry(checkout).or_default().commits = commits;
                changed = true;
            }
        }
        if changed {
            self.save_soon(cx);
        }
    }
}

impl Activity {
    /// Count `n` files written at `now`, and forget what's older than is kept.
    pub fn record(&mut self, now: i64, n: u32) {
        let quarter = now - now.rem_euclid(QUARTER);
        if n == 0 {
            return;
        }
        match self.quarters.back_mut() {
            Some((at, count)) if *at == quarter => *count += n,
            _ => self.quarters.push_back((quarter, n)),
        }
        self.last = Some(now);
        while (self.quarters.front()).is_some_and(|(at, _)| *at <= now - KEPT_FOR) {
            self.quarters.pop_front();
        }
    }

    /// How many files were written in each of the `n` quarters up to the one `now` is in,
    /// oldest first.
    pub fn bars(&self, now: i64, n: usize) -> Vec<u32> {
        let last = now - now.rem_euclid(QUARTER);
        let first = last - QUARTER * (n as i64 - 1);
        let mut bars = vec![0; n];
        for (at, count) in self.quarters.iter().filter(|(at, _)| *at >= first) {
            if let Some(bar) = bars.get_mut(((at - first) / QUARTER) as usize) {
                *bar += count;
            }
        }
        bars
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, COMMITS_KEPT_FOR, KEPT_FOR, QUARTER, SeenCommit, remembered};
    use crate::backend::{Branch, Commit, CommitState, PushState, Review, ReviewState};

    fn branch(name: &str, commits: &[(u8, i64)], review: Option<ReviewState>) -> Branch {
        Branch {
            name: name.to_owned(),
            ref_name: Some(crate::backend::model::build::full(&format!(
                "refs/heads/{name}"
            ))),
            commits: (commits.iter())
                .map(|&(n, time)| Commit {
                    key: format!("change-{n}"),
                    authored: format!("authored-{n}"),
                    time,
                    ..Commit::bare(gix::ObjectId::from([n; 20]))
                })
                .collect(),
            incoming: Vec::new(),
            remote: None,
            push: PushState::NothingToPush,
            review: review.map(|state| Review {
                state,
                ..Review::default()
            }),
        }
    }

    fn activity(commits: Vec<SeenCommit>) -> Activity {
        Activity {
            commits,
            ..Activity::default()
        }
    }

    fn said(commits: &[SeenCommit]) -> Vec<(&str, &str, CommitState)> {
        (commits.iter())
            .map(|c| {
                (
                    &*c.key,
                    c.branch.as_deref().expect("every branch here is named"),
                    c.state,
                )
            })
            .collect()
    }

    #[test]
    fn commits_of_a_branch_that_left_stay_as_last_seen() {
        let now = COMMITS_KEPT_FOR * 2;
        let (a, b) = (
            branch("a", &[(2, now - 20), (1, now - 30)], None),
            branch("b", &[(3, now - 10), (9, now - COMMITS_KEPT_FOR - 1)], None),
        );
        let kept = remembered(None, now, &[&a, &b]);
        assert_eq!(
            said(&kept),
            [
                ("change-3", "refs/heads/b", CommitState::Local),
                ("change-2", "refs/heads/a", CommitState::Local),
                ("change-1", "refs/heads/a", CommitState::Local),
            ],
            "the branches' commits of the last day, newest first"
        );

        let a = branch("a", &[(1, now - 30)], Some(ReviewState::Merged));
        let kept = remembered(Some(&activity(kept)), now, &[&a]);
        assert_eq!(
            said(&kept),
            [
                ("change-3", "refs/heads/b", CommitState::Local),
                ("change-1", "refs/heads/a", CommitState::Integrated),
            ],
            "b left with its commit; a lost one, as squashed away, and landed once merged"
        );

        let renamed = branch("b2", &[(3, now - 10)], None);
        let kept = remembered(Some(&activity(kept)), now, &[&renamed]);
        assert_eq!(
            said(&kept),
            [
                ("change-3", "refs/heads/b2", CommitState::Local),
                ("change-1", "refs/heads/a", CommitState::Integrated),
            ],
            "a branch that has a gone one's commit takes it, so it isn't listed twice"
        );

        let mut rebased = branch("b3", &[(3, now - 10)], None);
        rebased.commits[0].key = "rebased-3".to_owned();
        let kept = remembered(Some(&activity(kept)), now, &[&rebased]);
        assert_eq!(
            said(&kept),
            [
                ("rebased-3", "refs/heads/b3", CommitState::Local),
                ("change-1", "refs/heads/a", CommitState::Integrated),
            ],
            "renamed again and rebased by Git, which drops the change ID, it's still one commit"
        );

        assert_eq!(
            remembered(Some(&activity(kept)), now + COMMITS_KEPT_FOR, &[]),
            [],
            "commits older than a day are forgotten"
        );
    }

    #[test]
    fn writes_count_by_quarter_for_a_week() {
        let mut activity = Activity::default();
        let start = 100 * QUARTER;
        activity.record(start, 2);
        activity.record(start + 60, 1);
        activity.record(start + QUARTER, 1);
        assert_eq!(
            activity.bars(start + QUARTER, 3),
            [0, 3, 1],
            "a quarter's writes, oldest first, up to the quarter now is in"
        );
        let later = start + KEPT_FOR + 1;
        activity.record(later, 1);
        assert_eq!(
            activity.bars(later, 1 + KEPT_FOR as usize / QUARTER as usize)[..2],
            [0, 1],
            "the first quarter's forgotten after a week, the next one kept"
        );
        assert_eq!(activity.last, Some(later));
    }
}
