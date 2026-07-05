//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

use crate::graph_rebase::{Checkout, MaterializeOutcome, Pick, Step, SuccessfulRebase};

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let mut head_reference_update = None;
        for checkout in std::mem::take(&mut self.checkouts) {
            match checkout {
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => {
                    let step = self.graph.step_view(selector.id);

                    let (new_head, new_head_refname) = match step {
                        Step::None => bail!("Checkout selector is pointing to none"),
                        Step::Pick(Pick { id, .. }) => (id, None),
                        Step::Reference { refname, .. } => {
                            let parent_step_id = crate::graph_rebase::positions::resolve_to_pick(
                                &self.graph,
                                selector.id,
                            )
                            .context("No commit to reference")?;
                            let Some(id) = self.graph.commit_id(parent_step_id) else {
                                bail!("resolve_to_pick should always return a commit pick");
                            };
                            (id, Some(refname))
                        }
                    };
                    head_reference_update = new_head_refname;

                    // If the head has changed (which means it's in the
                    // commit mapping), perform a safe checkout.
                    safe_checkout_from_head(
                        new_head,
                        &self.repo,
                        Options {
                            skip_head_update: true,
                            merge_base_override,
                            allow_conflicted_commit_checkout: true,
                        },
                    )?;
                }
            }
        }

        let mut ref_edits = std::mem::take(&mut self.ref_edits);
        if let Some(refname) = head_reference_update
            && self.repo.head_name()?.as_ref() != Some(&refname)
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
        self.finish(ref_edits)
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
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let ref_edits = std::mem::take(&mut self.ref_edits);
        self.finish(ref_edits)
    }

    fn finish(self, ref_edits: Vec<RefEdit>) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        self.repo.edit_references(ref_edits)?;

        refresh_workspace_from_arena(&self.graph, self.workspace, &self.repo, &*self.meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}

/// The editor's mutated arena IS the next workspace — materialization projects it directly
/// instead of rewalking the repository. (Mutate-then-project == rewalk-then-project was
/// proven suite-wide on a field-exact projection fingerprint before the rewalk retired.)
///
/// Falls back to a rewalk when the arena has nothing to project: HEAD is unborn (e.g. its
/// referent was deleted without a repoint) or points outside the editor's graph.
fn refresh_workspace_from_arena<M: RefMetadata>(
    graph: &crate::graph_rebase::EditorGraph,
    workspace: &mut but_graph::Workspace,
    repo: &gix::Repository,
    meta: &M,
) -> anyhow::Result<()> {
    let project_meta = workspace.graph.project_meta.clone();
    let options = workspace.graph.options.clone();
    let Some(mutated) = but_graph::workspace_from_commit_graph(
        graph.arena().clone(),
        repo,
        meta,
        project_meta.clone(),
        options.clone(),
    )?
    else {
        return workspace.refresh_from_head(repo, meta, project_meta);
    };
    *workspace = mutated;
    Ok(())
}
