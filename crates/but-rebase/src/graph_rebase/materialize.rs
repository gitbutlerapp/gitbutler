//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::{
    bstr::BStr,
    refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    },
};

use crate::graph_rebase::{
    Checkout, MaterializeOutcome, Pick, Selector, Step, SuccessfulRebase,
    util::collect_ordered_parents,
};

/// Check out `new_tip` in the linked worktree named `worktree_name`, doing nothing
/// when the worktree is already there.
///
/// The worktree repository is opened from disk, so all objects the rebase created
/// must be persisted before calling this. The worktree's `HEAD` stays symbolic -
/// the branch it points to is moved by the shared ref-store edits afterwards.
///
/// `merge_base_override` is passed through to the checkout's snapshot merge so
/// changes consumed from this worktree cancel out instead of reappearing as
/// uncommitted changes. See [`Checkout::Worktree`].
fn checkout_worktree(
    repo: &gix::Repository,
    worktree_name: &BStr,
    new_tip: gix::ObjectId,
    merge_base_override: Option<gix::ObjectId>,
) -> Result<()> {
    let proxy = repo
        .worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == worktree_name)
        .with_context(|| format!("Worktree {worktree_name} no longer exists"))?;
    let wt_repo = proxy.into_repo()?;
    if wt_repo.head_id().ok().map(|id| id.detach()) == Some(new_tip) {
        return Ok(());
    }
    safe_checkout_from_head(
        new_tip,
        &wt_repo,
        Options {
            skip_head_update: true,
            merge_base_override,
            // Never write conflict-encoded trees into a plain linked worktree -
            // a conflicted tip leaves the worktree stale instead.
            allow_conflicted_commit_checkout: false,
        },
    )?;
    Ok(())
}

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    /// Resolve the tip a worktree checkout `selector` points to after the rebase,
    /// or `None` when the branch step was removed from the graph.
    fn resolve_worktree_checkout_tip(&self, selector: Selector) -> Result<Option<gix::ObjectId>> {
        let selector = self.history.normalize_selector(selector)?;
        Ok(match &self.graph[selector.id] {
            Step::None => None,
            Step::Pick(Pick { id, .. }) => Some(*id),
            Step::Reference { .. } => {
                let parents = collect_ordered_parents(&self.graph, selector.id);
                let parent_step_id = parents.first().context("No first parent to reference")?;
                let Step::Pick(Pick { id, .. }) = self.graph[*parent_step_id] else {
                    bail!("collect_ordered_parents should always return a commit pick");
                };
                Some(id)
            }
        })
    }

    /// The tip each linked-worktree checkout will point to once this rebase is
    /// materialized, as `(worktree_name, commit_id)` pairs.
    ///
    /// Worktrees whose checked-out branch was removed from the graph are skipped.
    /// Objects referenced by the returned ids may exist only in the in-memory
    /// repository, see [`Self::repo()`].
    pub fn worktree_checkout_tips(&self) -> Result<Vec<(gix::bstr::BString, gix::ObjectId)>> {
        let mut tips = Vec::new();
        for checkout in &self.checkouts {
            let Checkout::Worktree {
                selector,
                worktree_name,
                ..
            } = checkout
            else {
                continue;
            };
            if let Some(tip) = self.resolve_worktree_checkout_tip(*selector)? {
                tips.push((worktree_name.clone(), tip));
            }
        }
        Ok(tips)
    }

    /// Run the checkout of every linked worktree whose branch this rebase moves.
    ///
    /// All objects must be persisted beforehand - the worktree repositories are
    /// opened from disk. A broken linked worktree degrades to today's
    /// stale-checkout behavior with a warning instead of failing the whole
    /// operation.
    fn checkout_worktrees(&self, repo: &gix::Repository) -> Result<()> {
        for checkout in &self.checkouts {
            let Checkout::Worktree {
                selector,
                worktree_name,
                merge_base_override,
            } = checkout
            else {
                continue;
            };
            let Some(new_tip) = self.resolve_worktree_checkout_tip(*selector)? else {
                tracing::warn!(
                    worktree = %worktree_name,
                    "the branch this worktree checks out was removed - leaving its checkout as is"
                );
                continue;
            };
            if let Err(err) = checkout_worktree(
                repo,
                worktree_name.as_ref(),
                new_tip,
                *merge_base_override,
            ) {
                tracing::warn!(
                    worktree = %worktree_name,
                    err = %err,
                    "failed to check out linked worktree - its branch still moves"
                );
            }
        }
        Ok(())
    }

    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        self.checkout_worktrees(&repo)?;

        let mut head_reference_update = None;
        for checkout in &self.checkouts {
            match checkout {
                Checkout::Worktree { .. } => {}
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => {
                    let selector = self.history.normalize_selector(*selector)?;
                    let step = self.graph[selector.id].clone();

                    let (new_head, new_head_refname) = match step {
                        Step::None => bail!("Checkout selector is pointing to none"),
                        Step::Pick(Pick { id, .. }) => (id, None),
                        Step::Reference { refname, .. } => {
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
                            skip_head_update: true,
                            merge_base_override: *merge_base_override,
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

    /// Materializes a rebase without performing a checkout of the editor's own
    /// (`HEAD`) worktree.
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
    ///
    /// Note that linked worktrees whose branches the rebase moves are still
    /// checked out - "without checkout" is strictly about the editor's own
    /// worktree; skipping the linked ones would leave their checkouts stale
    /// behind their moved branches.
    pub fn materialize_without_checkout(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        self.checkout_worktrees(&repo)?;

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
