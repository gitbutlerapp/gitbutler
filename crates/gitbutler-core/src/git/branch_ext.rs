use anyhow::{Context, Result};

use crate::types::ReferenceName;

pub trait BranchExt {
    fn reference_name(&self) -> Result<ReferenceName>;
}

impl<'repo> BranchExt for git2::Branch<'repo> {
    fn reference_name(&self) -> Result<ReferenceName> {
        let name = self.get().name().context("Failed to get branch name")?;

        Ok(name.into())
    }
}
