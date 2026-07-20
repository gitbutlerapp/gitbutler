//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use bstr::{BString, ByteSlice as _};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

use crate::graph_rebase::{
    Checkout, MaterializeOutcome, Pick, Step, SuccessfulRebase, util::resolve_to_commit,
};

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    /// Materializes a history rewrite
    pub fn materialize(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(self.repo)?;
        }

        let mut head_update = None;
        for checkout in self.checkouts {
            match checkout {
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => {
                    let selector = self.history.normalize_selector(selector)?;
                    let (new_head, new_head_target) = match self.graph.step(selector.id) {
                        Step::None => bail!("Checkout selector is pointing to none"),
                        Step::Pick(Pick { id, .. }) => (id, Target::Object(id)),
                        Step::Reference { refname, .. } => {
                            let target = resolve_to_commit(&self.graph, selector.id)
                                .context("No target commit for checkout reference")?;
                            let Step::Pick(Pick { id, .. }) = self.graph.step(target) else {
                                bail!("resolve_to_commit should always land on a commit pick");
                            };
                            (id, Target::Symbolic(refname))
                        }
                    };
                    head_update = Some(new_head_target);

                    // If the head has changed (which means it's in the
                    // commit mapping), perform a safe checkout.
                    safe_checkout_from_head(
                        new_head,
                        &repo,
                        Options {
                            skip_head_update: true,
                            merge_base_override,
                            allow_conflicted_commit_checkout: true,
                        },
                    )?;
                }
            }
        }

        let mut ref_edits = self.ref_edits.clone();
        if let Some(target) = head_update {
            let unchanged = match &target {
                Target::Object(id) => {
                    repo.head_name()?.is_none() && repo.head_id()?.detach() == *id
                }
                Target::Symbolic(refname) => repo.head_name()?.as_ref() == Some(refname),
            };
            if !unchanged {
                let target_description: BString = match &target {
                    Target::Object(id) => id.to_string().into(),
                    Target::Symbolic(refname) => refname.shorten().to_owned(),
                };
                ref_edits.push(RefEdit {
                    change: Change::Update {
                        log: LogChange {
                            mode: RefLog::AndReference,
                            force_create_reflog: false,
                            message: gix::reference::log::message(
                                "safe checkout",
                                target_description.as_bstr(),
                                0,
                            ),
                        },
                        expected: PreviousValue::Any,
                        new: target,
                    },
                    name: "HEAD".try_into().expect("root refs are always valid"),
                    deref: false,
                });
            }
        }
        repo.edit_references(ref_edits)?;

        refresh_workspace_from_head(
            self.workspace,
            &repo,
            &*self.meta,
            self.project_meta.clone(),
        )?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            project_meta: self.project_meta,
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

        refresh_workspace_from_head(
            self.workspace,
            &repo,
            &*self.meta,
            self.project_meta.clone(),
        )?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            project_meta: self.project_meta,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}

fn refresh_workspace_from_head(
    workspace: &mut but_graph::Workspace,
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
) -> Result<()> {
    let graph = but_graph::Graph::from_repo(
        repo,
        meta,
        project_meta,
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    *workspace = graph.into_workspace()?;
    Ok(())
}
