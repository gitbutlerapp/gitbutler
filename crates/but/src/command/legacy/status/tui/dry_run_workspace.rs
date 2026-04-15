use std::collections::HashSet;

#[derive(Debug, Default)]
pub(super) struct DryRunWorkspace {
    conflicted_commits: HashSet<gix::ObjectId>,
}

impl DryRunWorkspace {
    pub(super) fn clear(&mut self) {
        let Self { conflicted_commits } = self;
        conflicted_commits.clear();
    }

    pub(super) fn insert_conflicted_commit(&mut self, commit: gix::ObjectId) {
        self.conflicted_commits.insert(commit);
    }

    pub(super) fn is_commit_conflicted(&self, commit: gix::ObjectId) -> bool {
        self.conflicted_commits.contains(&commit)
    }
}
