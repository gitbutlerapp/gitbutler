//! Functions for materializing a rebase
use crate::graph_rebase::rebase::SuccessfulRebase;
use anyhow::Result;
use but_core::worktree::{
    checkout::{Options, UncommitedWorktreeChanges},
    safe_checkout,
};

impl SuccessfulRebase {
    /// Materializes a history rewrite
    pub fn materialize(self, repo: &gix::Repository) -> Result<()> {
        for checkout in self.checkouts {
            // TODO(CTO): improve safe_checkout to allow for speculation
            safe_checkout(
                checkout.old_head_id,
                checkout.head_id,
                repo,
                Options {
                    uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                    skip_head_update: true,
                },
            )?;
        }

        repo.edit_references(self.ref_edits.clone())?;

        Ok(())
    }
}
