use anyhow::{Context, Result};
use gitbutler_reference::ReferenceName;

pub trait BranchExt {
    /// Returns the full name of the reference.
    ///
    /// ### Panics
    ///
    /// If the reference name is not UTF-8
    fn reference_name(&self) -> Result<ReferenceName>;
}

impl BranchExt for git2::Branch<'_> {
    fn reference_name(&self) -> Result<ReferenceName> {
        let full_name = self.get().name().context("Failed to get branch name")?;
        Ok(full_name.into())
    }
}
