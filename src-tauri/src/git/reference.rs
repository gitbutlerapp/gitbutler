use super::{Commit, Result, Tree};

pub struct Reference<'repo> {
    reference: git2::Reference<'repo>,
}

impl<'repo> From<git2::Reference<'repo>> for Reference<'repo> {
    fn from(reference: git2::Reference<'repo>) -> Self {
        Reference { reference }
    }
}

impl<'repo> Reference<'repo> {
    pub fn name(&self) -> Option<&str> {
        self.reference.name()
    }

    pub fn target(&self) -> Option<git2::Oid> {
        self.reference.target()
    }

    pub fn peel_to_commit(&self) -> Result<Commit<'repo>> {
        self.reference.peel_to_commit().map(Commit::from)
    }

    pub fn peel_to_tree(&self) -> Result<Tree<'repo>> {
        self.reference.peel_to_tree().map(Tree::from)
    }

    pub fn rename(
        &mut self,
        new_name: &str,
        force: bool,
        log_message: &str,
    ) -> Result<Reference<'repo>> {
        self.reference
            .rename(new_name, force, log_message)
            .map(Reference::from)
    }

    pub fn delete(&mut self) -> Result<()> {
        self.reference.delete()
    }
}
