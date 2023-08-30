pub struct Reference<'a> {
    reference: git2::Reference<'a>,
}

impl<'a> From<git2::Reference<'a>> for Reference<'a> {
    fn from(reference: git2::Reference<'a>) -> Self {
        Reference { reference }
    }
}

impl<'a> Reference<'a> {
    pub fn name(&self) -> Option<&str> {
        self.reference.name()
    }

    pub fn target(&self) -> Option<git2::Oid> {
        self.reference.target()
    }

    pub fn peel_to_commit(&self) -> super::Result<git2::Commit<'a>> {
        self.reference.peel_to_commit()
    }

    pub fn peel_to_tree(&self) -> super::Result<git2::Tree<'a>> {
        self.reference.peel_to_tree()
    }
}
