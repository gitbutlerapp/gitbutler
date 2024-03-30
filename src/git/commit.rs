use super::{Oid, Result, Signature, Tree};

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
        self.commit.tree().map(Into::into).map_err(Into::into)
    }

    pub fn tree_id(&self) -> Oid {
        self.commit.tree_id().into()
    }

    pub fn parents(&self) -> Result<Vec<Commit<'repo>>> {
        let mut parents = vec![];
        for i in 0..self.parent_count() {
            parents.push(self.parent(i)?);
        }
        Ok(parents)
    }

    pub fn parent(&self, n: usize) -> Result<Commit<'repo>> {
        self.commit.parent(n).map(Into::into).map_err(Into::into)
    }

    pub fn time(&self) -> git2::Time {
        self.commit.time()
    }

    pub fn author(&self) -> Signature<'_> {
        self.commit.author().into()
    }

    pub fn message(&self) -> Option<&str> {
        self.commit.message()
    }

    pub fn committer(&self) -> Signature<'_> {
        self.commit.committer().into()
    }

    pub fn raw_header(&self) -> Option<&str> {
        self.commit.raw_header()
    }
}
