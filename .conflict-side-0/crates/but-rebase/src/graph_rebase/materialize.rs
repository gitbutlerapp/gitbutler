//! Functions for materializing a rebase
use std::collections::HashMap;

use crate::graph_rebase::{Checkouts, rebase::SuccessfulRebase};
use anyhow::Result;
use but_core::{
    ObjectStorageExt as _,
    worktree::{
        checkout::{Options, UncommitedWorktreeChanges},
        safe_checkout_from_head,
    },
};

/// The outcome of a materialize
#[derive(Debug, Clone)]
pub struct MaterializeOutcome {
    /// A mapping of any commits that were rewritten as part of the rebase
    pub commit_mapping: HashMap<gix::ObjectId, gix::ObjectId>,
}

impl SuccessfulRebase {
    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        for checkout in self.checkouts {
            match checkout {
                Checkouts::Head => {
                    let head_oid = repo.head_commit()?.id;
                    if let Some(new_head) = self.commit_mapping.get(&head_oid) {
                        // If the head has changed (which means it's in the
                        // commit mapping), perform a safe checkout.
                        safe_checkout_from_head(
                            *new_head,
                            &repo,
                            Options {
                                uncommitted_changes:
                                    UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                                skip_head_update: true,
                            },
                        )?;
                    }
                }
            }
        }

        repo.edit_references(self.ref_edits.clone())?;

        Ok(MaterializeOutcome {
            commit_mapping: self.commit_mapping,
        })
    }
}
