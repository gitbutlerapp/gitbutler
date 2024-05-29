// use anyhow::Result;
use super::Result;
use bstr::BStr;
use git2::Commit;

/// Extension trait for `git2::Commit`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Commit`
pub trait CommitExt {
    /// Obtain the commit-message as bytes, but without assuming any encoding.
    fn message_bstr(&self) -> &BStr;
    fn change_id(&self) -> Option<String>;
    fn parents_gb(&self) -> Result<Vec<Commit<'_>>>;
    fn is_signed(&self) -> bool;
    fn tree_gb(&self) -> Result<git2::Tree<'_>>;
}

impl<'repo> CommitExt for git2::Commit<'repo> {
    fn message_bstr(&self) -> &BStr {
        self.message_bytes().as_ref()
    }
    fn change_id(&self) -> Option<String> {
        let cid = self.header_field_bytes("change-id").ok()?;
        if cid.is_empty() {
            None
        } else {
            // convert the Buf to a string
            let ch_id = std::str::from_utf8(&cid).ok()?.to_owned();
            Some(ch_id)
        }
    }
    fn parents_gb(&self) -> Result<Vec<Commit<'repo>>> {
        let mut parents = vec![];
        for i in 0..self.parent_count() {
            parents.push(self.parent(i)?);
        }
        Ok(parents)
    }
    fn is_signed(&self) -> bool {
        let cid = self.header_field_bytes("gpgsig").ok();
        cid.is_some()
    }
    fn tree_gb(&self) -> Result<git2::Tree<'repo>> {
        self.tree().map_err(Into::into)
    }
}
