mod refname;
pub use refname::{LocalRefname, Refname, RemoteRefname, VirtualRefname};

use super::{Oid, Result};

pub struct Reference<'repo> {
    reference: git2::Reference<'repo>,
}

impl<'repo> From<git2::Reference<'repo>> for Reference<'repo> {
    fn from(reference: git2::Reference<'repo>) -> Self {
        Reference { reference }
    }
}

impl<'repo> Reference<'repo> {
    pub fn name(&self) -> Option<Refname> {
        self.reference
            .name()
            .map(|name| name.parse().expect("libgit2 provides valid refnames"))
    }

    pub fn name_bytes(&self) -> &[u8] {
        self.reference.name_bytes()
    }

    pub fn target(&self) -> Option<Oid> {
        self.reference.target().map(Into::into)
    }

    pub fn peel_to_commit(&self) -> Result<git2::Commit<'repo>> {
        self.reference
            .peel_to_commit()
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn rename(
        &mut self,
        new_name: &Refname,
        force: bool,
        log_message: &str,
    ) -> Result<Reference<'repo>> {
        self.reference
            .rename(&new_name.to_string(), force, log_message)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn delete(&mut self) -> Result<()> {
        self.reference.delete().map_err(Into::into)
    }

    pub fn is_remote(&self) -> bool {
        self.reference.is_remote()
    }
}
