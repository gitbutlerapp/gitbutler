use anyhow::{Context, Result};
use gitbutler_reference::ReferenceName;

pub trait BranchExt {
    fn reference_name(&self) -> Result<ReferenceName>;
}

impl BranchExt for git2::Branch<'_> {
    fn reference_name(&self) -> Result<ReferenceName> {
        let name = self.get().name().context("Failed to get branch name")?;

        Ok(name.into())
    }
}
