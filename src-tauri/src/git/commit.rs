use super::{Oid, Result, Tree};

pub struct Commit<'repo> {
    commit: git2::Commit<'repo>,
}

impl<'repo> From<git2::Commit<'repo>> for Commit<'repo> {
    fn from(commit: git2::Commit<'repo>) -> Self {
        Self { commit }
    }
}

impl<'repo> From<&'repo git2::Commit<'repo>> for Commit<'repo> {
    fn from(commit: &'repo git2::Commit<'repo>) -> Self {
        Self {
            commit: commit.clone(),
        }
    }
}

impl<'repo> From<&'repo Commit<'repo>> for &'repo git2::Commit<'repo> {
    fn from(val: &'repo Commit<'repo>) -> Self {
        &val.commit
    }
}

impl<'repo> Commit<'repo> {
    pub fn id(&self) -> Oid {
        self.commit.id().into()
    }

    pub fn parent_count(&self) -> usize {
        self.commit.parent_count()
    }

    pub fn tree(&self) -> Result<Tree<'repo>> {
        self.commit.tree().map(Into::into)
    }

    pub fn tree_id(&self) -> Oid {
        self.commit.tree_id().into()
    }

    pub fn parent(&self, n: usize) -> Result<Commit<'repo>> {
        self.commit.parent(n).map(Into::into)
    }

    pub fn time(&self) -> git2::Time {
        self.commit.time()
    }

    pub fn author(&self) -> git2::Signature<'_> {
        self.commit.author()
    }

    pub fn message(&self) -> Option<&str> {
        self.commit.message()
    }

    pub fn committer(&self) -> git2::Signature<'_> {
        self.commit.committer()
    }
}

pub struct AnnotatedCommit<'repo> {
    annotated_commit: git2::AnnotatedCommit<'repo>,
}

impl<'repo> From<git2::AnnotatedCommit<'repo>> for AnnotatedCommit<'repo> {
    fn from(annotated_commit: git2::AnnotatedCommit<'repo>) -> Self {
        Self { annotated_commit }
    }
}

impl<'repo> From<&'repo AnnotatedCommit<'repo>> for &'repo git2::AnnotatedCommit<'repo> {
    fn from(val: &'repo AnnotatedCommit<'repo>) -> Self {
        &val.annotated_commit
    }
}
