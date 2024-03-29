use super::{Commit, Oid, Result, Tree};

pub struct Branch<'repo> {
    branch: git2::Branch<'repo>,
}

impl<'repo> From<git2::Branch<'repo>> for Branch<'repo> {
    fn from(branch: git2::Branch<'repo>) -> Self {
        Self { branch }
    }
}

impl<'repo> Branch<'repo> {
    pub fn name(&self) -> Option<&str> {
        self.branch.get().name()
    }

    pub fn refname(&self) -> Option<&str> {
        self.branch.get().name()
    }

    pub fn target(&self) -> Option<Oid> {
        self.branch.get().target().map(Into::into)
    }

    pub fn upstream(&self) -> Result<Branch<'repo>> {
        self.branch.upstream().map(Into::into).map_err(Into::into)
    }

    pub fn refname_bytes(&self) -> &[u8] {
        self.branch.get().name_bytes()
    }

    pub fn peel_to_tree(&self) -> Result<Tree<'repo>> {
        self.branch
            .get()
            .peel_to_tree()
            .map_err(Into::into)
            .map(Into::into)
    }

    pub fn peel_to_commit(&self) -> Result<Commit<'repo>> {
        self.branch
            .get()
            .peel_to_commit()
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn is_remote(&self) -> bool {
        self.branch.get().is_remote()
    }
}
