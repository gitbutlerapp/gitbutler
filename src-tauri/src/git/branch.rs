use super::Reference;

pub struct Branch<'repo> {
    reference: Reference<'repo>,
}

impl<'repo> From<git2::Branch<'repo>> for Branch<'repo> {
    fn from(branch: git2::Branch<'repo>) -> Self {
        Self {
            reference: branch.into_reference().into(),
        }
    }
}

impl<'repo> Branch<'repo> {
    pub fn name(&self) -> Option<String> {
        self.reference.name().map(String::from)
    }

    pub fn get(&self) -> &Reference<'repo> {
        &self.reference
    }

    pub fn into_reference(self) -> Reference<'repo> {
        self.reference
    }
}
