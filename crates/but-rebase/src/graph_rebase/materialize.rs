//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _,
    worktree::{
        checkout::{Options, UncommitedWorktreeChanges},
        safe_checkout_from_head,
    },
};

use crate::graph_rebase::{
    Checkout, MaterializeOutcome, Pick, Step, SuccessfulRebase, util::collect_ordered_parents,
};

impl SuccessfulRebase {
    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        for checkout in self.checkouts {
            match checkout {
                Checkout::Head(selector) => {
                    let selector = self.history.normalize_selector(selector)?;
                    let step = self.graph[selector.id].clone();

                    let new_head = match step {
                        Step::None => bail!("Checkout selector is pointing to none"),
                        Step::Pick(Pick { id, .. }) => id,
                        Step::Reference { .. } => {
                            let parents = collect_ordered_parents(&self.graph, selector.id);
                            let parent_step_id =
                                parents.first().context("No first parent to reference")?;
                            let Step::Pick(Pick { id, .. }) = self.graph[*parent_step_id] else {
                                bail!("collect_ordered_parents should always return a commit pick");
                            };
                            id
                        }
                    };

                    // If the head has changed (which means it's in the
                    // commit mapping), perform a safe checkout.
                    safe_checkout_from_head(
                        new_head,
                        &repo,
                        Options {
                            uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                            skip_head_update: true,
                        },
                    )?;
                }
            }
        }

        repo.edit_references(self.ref_edits.clone())?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
        })
    }

    /// Materializes a rebase without performing a checkout.
    ///
    /// For the vast majority of operations you want to use
    /// [`Self::materialize`]. This is intended to be used in niche cases like
    /// `uncommit`.
    ///
    /// This has means that we don't "cherry pick" the uncommitted changes from
    /// the old head onto the new one.
    ///
    /// If I dropped a commit from the history,
    /// [`Self::materialize_without_checkout`] will now see those changes in
    /// your working directory.
    ///
    /// If I instead called [`Self::materialize`], the changes would instead be
    /// gone from disk.
    pub fn materialize_without_checkout(mut self) -> Result<MaterializeOutcome> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        repo.edit_references(self.ref_edits.clone())?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
        })
    }
}
