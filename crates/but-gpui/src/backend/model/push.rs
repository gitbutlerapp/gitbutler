use super::{Branch, CommitId, PushState, Target, Workspace};

impl Workspace {
    /// `ref_name`'s branch and the ones below it in its stack, which pushing it pushes too.
    pub(crate) fn pushed_with(&self, ref_name: &str) -> &[Branch] {
        (self.stack_of(ref_name))
            .and_then(|(stack, at)| stack.branches.get(at..))
            .unwrap_or_default()
    }

    /// Whether pushing `ref_name` has commits to send. An unnamed branch has no ref to push.
    pub fn can_push(&self, ref_name: &str) -> bool {
        !ref_name.is_empty() && self.pushed_with(ref_name).iter().any(Branch::can_push)
    }

    /// The commits only the remote has that pushing `ref_name` drops: a forced push replaces
    /// what's there.
    pub fn push_drops(&self, ref_name: &str) -> Vec<CommitId> {
        if !self.needs_force(ref_name) {
            return Vec::new();
        }
        (self.pushed_with(ref_name).iter())
            .flat_map(|b| b.incoming.iter().map(|commit| commit.id))
            .collect()
    }

    /// Why `ref_name` can't be pushed yet, though it has commits to send: pushing goes to
    /// the remote of the target's upstream, and a conflicted commit would go along.
    pub fn push_refused(&self, ref_name: &str) -> Option<&'static str> {
        let pushed = self.pushed_with(ref_name);
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
        } else if (pushed.iter()).any(|b| b.commits.iter().any(|c| c.conflicted)) {
            Some("Resolve the conflicted commits first")
        } else {
            None
        }
    }

    /// The target, when pushing `ref_name` forces the target branch, which others' work builds
    /// on. What only its remote has counts as the target's incoming commits, not the branch's.
    pub fn forced_target(&self, ref_name: &str) -> Option<&Target> {
        (self.pushed_with(ref_name).iter())
            .any(|b| {
                b.push == PushState::NeedsForce && self.target_branch.as_ref() == Some(&b.ref_name)
            })
            .then_some(self.target.as_ref())
            .flatten()
    }

    /// Whether pushing `ref_name` has to force: it or a branch below it was rewritten since
    /// it was pushed.
    pub fn needs_force(&self, ref_name: &str) -> bool {
        (self.pushed_with(ref_name).iter()).any(|branch| branch.push == PushState::NeedsForce)
    }

    /// Whether `ref_name` can pull the commits only its remote has: it has some, and no
    /// branches stacked on it. Pulling into a branch with others above moves its reference
    /// to the top of the stack, so the app doesn't offer it.
    pub fn can_pull(&self, ref_name: &str) -> bool {
        self.stack_of(ref_name).is_some_and(|(stack, at)| {
            at == 0 && (stack.branches.first()).is_some_and(|b| !b.incoming.is_empty())
        })
    }
}
