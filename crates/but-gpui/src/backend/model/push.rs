use super::{Branch, CommitId, PushState, Target, Workspace};

impl Workspace {
    /// `ref_name`'s branch and the ones below it, down the lanes it rests on, which pushing it
    /// pushes too.
    pub(crate) fn pushed_with(&self, ref_name: &super::FullName) -> impl Iterator<Item = &Branch> {
        self.chain(ref_name).flat_map(|link| link.branches())
    }

    /// Whether pushing `ref_name` takes GitButler's workspace commit along, which merges every
    /// applied branch: the lanes it goes down end on a commit no lane holds, as one started
    /// on the main checkout's `HEAD` does.
    fn pushes_workspace(&self, ref_name: &super::FullName) -> bool {
        (self.chain(ref_name).last())
            .and_then(|link| link.lane.rests_on)
            .is_some_and(|id| self.lane_holding(id).is_none())
    }

    /// Whether pushing `ref_name` has commits to send.
    pub fn can_push(&self, ref_name: &super::FullName) -> bool {
        self.pushed_with(ref_name).any(Branch::can_push)
    }

    /// The commits only the remote has that pushing `ref_name` drops: a forced push replaces
    /// what's there.
    pub fn push_drops(&self, ref_name: &super::FullName) -> Vec<CommitId> {
        if !self.needs_force(ref_name) {
            return Vec::new();
        }
        (self.pushed_with(ref_name))
            .flat_map(|b| b.incoming.iter().map(|commit| commit.id))
            .collect()
    }

    /// Why `ref_name` can't be pushed yet, though it has commits to send: pushing goes to
    /// the remote of the target's upstream, and a conflicted commit would go along.
    pub fn push_refused(&self, ref_name: &super::FullName) -> Option<&'static str> {
        let unpushed = (self.target.as_ref()).map_or(self.target_branch.is_some(), |t| t.local);
        if self.remoteless {
            Some("The project has no remote to push to: add one with `git remote add`")
        } else if (self.target.as_ref()).is_some_and(|t| t.untracked.is_some()) {
            Some(
                "The target branch doesn't track the remote's of its name: the project's page says how to",
            )
        } else if unpushed {
            Some("The target branch tracks no remote branch: push it once, with `git push -u`")
        } else if self.target.is_none() {
            Some("Pick the project's target branch first")
        } else if (self.pushed_with(ref_name)).any(|b| b.commits.iter().any(|c| c.conflicted)) {
            Some("Resolve the conflicted commits first")
        } else if self.pushes_workspace(ref_name) {
            Some(
                "It was started on GitButler's workspace commit, so pushing it would publish \
                 every applied branch",
            )
        } else {
            None
        }
    }

    /// The target, when pushing `ref_name` forces the target branch, which others' work builds
    /// on. What only its remote has counts as the target's incoming commits, not the branch's.
    pub fn forced_target(&self, ref_name: &super::FullName) -> Option<&Target> {
        (self.pushed_with(ref_name))
            .any(|b| {
                b.push == PushState::NeedsForce
                    && b.ref_name.is_some()
                    && self.target_branch == b.ref_name
            })
            .then_some(self.target.as_ref())
            .flatten()
    }

    /// Whether pushing `ref_name` has to force: it or a branch below it was rewritten since
    /// it was pushed.
    pub fn needs_force(&self, ref_name: &super::FullName) -> bool {
        (self.pushed_with(ref_name)).any(|branch| branch.push == PushState::NeedsForce)
    }

    /// Why `ref_name` can't be pushed now: it has no name, nothing to send, or
    /// [`Self::push_refused`] says why not.
    pub fn push_blocked(&self, ref_name: Option<&super::FullName>) -> Option<&'static str> {
        // A branch whose ref was deleted, as by `git branch -D`, has commits but no name.
        let Some(ref_name) = ref_name else {
            return Some("Name the branch first, then push it");
        };
        match self.can_push(ref_name) {
            true => self.push_refused(ref_name),
            false => Some("Nothing to push yet"),
        }
    }

    /// What pushing `ref_name` is called.
    pub fn push_label(&self, ref_name: &super::FullName) -> &'static str {
        match self.needs_force(ref_name) {
            true => "Force push",
            false => "Push",
        }
    }

    /// Whether `ref_name` can pull the commits only its remote has: it has some, it's the top
    /// of a stack, and no linked worktree rests on it. Pulling into a branch with others above
    /// moves its reference to the top of the stack, pulling under a worktree moves what it
    /// rests on, and pulling a worktree's own branch reaches past the worktree, so the app
    /// doesn't offer them.
    pub fn can_pull(&self, ref_name: &super::FullName) -> bool {
        self.stack_of(Some(ref_name)).is_some_and(|(stack, at)| {
            at == 0
                && (stack.branches.first()).is_some_and(|b| !b.incoming.is_empty())
                && !self.rested_on(ref_name)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::build::{branch, full, id, workspace, worktree};
    use super::super::{Branch, CommitState, PushState, RemoteCommit, Target, Workspace};

    #[test]
    fn a_worktree_branch_pushes_as_the_branches_beneath_it_need() {
        let standing = |name: &str, commits: &[u8], how: &str| -> Branch {
            let (push, state) = match how {
                "pushed" => (PushState::NothingToPush, CommitState::Pushed),
                "rewritten" => (PushState::NeedsForce, CommitState::Local),
                _ => (PushState::Unpushed, CommitState::Local),
            };
            let mut branch = branch(name, commits);
            branch.push = push;
            for commit in &mut branch.commits {
                commit.state = state;
                commit.conflicted = how == "conflicted";
            }
            branch
        };
        let pushes: String = [
            ("wt", ["pushed", "pushed", "pushed"]),
            ("wt", ["new", "pushed", "pushed"]),
            ("wt", ["pushed", "rewritten", "pushed"]),
            ("wt", ["pushed", "pushed", "new"]),
            ("wt", ["new", "conflicted", "pushed"]),
            ("middle", ["new", "pushed", "pushed"]),
        ]
        .map(|(pushed, [wt, middle, bottom])| {
            let ws = Workspace {
                target: Some(Target {
                    name: "origin/main".into(),
                    branch: "main".into(),
                    local: false,
                    untracked: None,
                    behind: 0,
                    commits_behind: 0,
                    needs_update: false,
                    incoming: Vec::new(),
                }),
                ..workspace(
                    vec![vec![
                        standing("top", &[1], "pushed"),
                        standing("middle", &[2, 3], middle),
                        standing("bottom", &[4], bottom),
                    ]],
                    vec![worktree("wt", Some(3), vec![standing("wt", &[5], wt)])],
                )
            };
            let ref_name = full(&format!("refs/heads/{pushed}"));
            let does = match (ws.can_push(&ref_name), ws.push_refused(&ref_name)) {
                (false, _) => "nothing to push",
                (true, Some(refused)) => refused,
                (true, None) if ws.needs_force(&ref_name) => "force push",
                (true, None) => "push",
            };
            format!("{pushed} with wt {wt}, middle {middle}, bottom {bottom}: {does}\n")
        })
        .concat();
        snapbox::assert_data_eq!(
            pushes,
            snapbox::str![[r#"
wt with wt pushed, middle pushed, bottom pushed: nothing to push
wt with wt new, middle pushed, bottom pushed: push
wt with wt pushed, middle rewritten, bottom pushed: force push
wt with wt pushed, middle pushed, bottom new: push
wt with wt new, middle conflicted, bottom pushed: Resolve the conflicted commits first
middle with wt new, middle pushed, bottom pushed: nothing to push

"#]]
        );
    }

    #[test]
    fn a_branch_pulls_only_from_the_top_of_a_stack_no_worktree_rests_on() {
        let behind = |mut branch: Branch| {
            branch.incoming = vec![RemoteCommit {
                id: id(99),
                key: String::new(),
                authored: String::new(),
                title: String::new(),
                body: String::new(),
                author: String::new(),
                email: String::new(),
                agent: None,
                time: 0,
                merge: false,
                review: None,
            }];
            branch
        };
        let ws = workspace(
            vec![
                vec![
                    behind(branch("top", &[1])),
                    behind(branch("middle", &[2, 3])),
                ],
                vec![behind(branch("under-tip", &[6]))],
            ],
            vec![
                worktree("wt", Some(3), vec![behind(branch("wt", &[5]))]),
                worktree("on-tip", Some(6), vec![branch("on-tip", &[8])]),
            ],
        );
        let pulls: String = ["top", "middle", "under-tip", "wt"]
            .map(|name| {
                let ref_name = full(&format!("refs/heads/{name}"));
                let (rested_on, can_pull) = (ws.rested_on(&ref_name), ws.can_pull(&ref_name));
                format!("{name}: rested_on {rested_on}, can_pull {can_pull}\n")
            })
            .concat();
        // Only `top` pulls: `middle` has `top` stacked on it and `wt` resting on its lower
        // commit, `under-tip` has `on-tip` resting on its tip, and `wt` is a linked
        // worktree's own branch.
        snapbox::assert_data_eq!(
            pulls,
            snapbox::str![[r#"
top: rested_on false, can_pull true
middle: rested_on true, can_pull false
under-tip: rested_on true, can_pull false
wt: rested_on false, can_pull false

"#]]
        );
    }
}
