//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{
        checkout::{Options, UncommitedWorktreeChanges},
        safe_checkout_from_head,
    },
};
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

use crate::graph_rebase::{
    Checkout, MaterializeOutcome, Pick, Step, SuccessfulRebase, util::collect_ordered_parents,
};

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        let mut head_reference_update = None;
        for checkout in self.checkouts {
            match checkout {
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => {
                    let selector = self.history.normalize_selector(selector)?;
                    let step = self.graph[selector.id].clone();

                    let (new_head, new_head_refname) = match step {
                        Step::None => bail!("Checkout selector is pointing to none"),
                        Step::Pick(Pick { id, .. }) => (id, None),
                        Step::Reference { refname } => {
                            let parents = collect_ordered_parents(&self.graph, selector.id);
                            let parent_step_id =
                                parents.first().context("No first parent to reference")?;
                            let Step::Pick(Pick { id, .. }) = self.graph[*parent_step_id] else {
                                bail!("collect_ordered_parents should always return a commit pick");
                            };
                            (id, Some(refname))
                        }
                    };
                    head_reference_update = new_head_refname;

                    // If the head has changed (which means it's in the
                    // commit mapping), perform a safe checkout.
                    safe_checkout_from_head(
                        new_head,
                        &repo,
                        Options {
                            uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                            skip_head_update: true,
                            merge_base_override,
                            allow_conflicted_commit_checkout: true,
                        },
                    )?;
                }
            }
        }

        let mut ref_edits = self.ref_edits.clone();
        if let Some(refname) = head_reference_update
            && repo.head_name()?.as_ref() != Some(&refname)
        {
            let ref_short_name = refname.shorten().to_owned();
            ref_edits.push(RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: gix::reference::log::message(
                            "safe checkout",
                            ref_short_name.as_ref(),
                            0,
                        ),
                    },
                    expected: PreviousValue::Any,
                    new: Target::Symbolic(refname),
                },
                name: "HEAD".try_into().expect("root refs are always valid"),
                deref: false,
            });
        }
        repo.edit_references(ref_edits)?;

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
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
    pub fn materialize_without_checkout(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        repo.edit_references(self.ref_edits.clone())?;

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}
