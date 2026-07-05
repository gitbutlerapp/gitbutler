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

        refresh_workspace_from_arena(&self.graph, self.workspace, &repo, &*self.meta)?;

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

        refresh_workspace_from_arena(&self.graph, self.workspace, &repo, &*self.meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}

/// THE FLIP (dissolve stage D4d-b): the editor's mutated arena IS the next workspace —
/// materialization projects it directly instead of rewalking the repository. The rewalk
/// survives as an env-gated verifier (`BUT_REBASE_WRITE_THROUGH=assert`): the dissolve's
/// parity obligation, mutate-then-project == rewalk-then-project, compared on a field-exact
/// fingerprint of everything the projection derives except graph indices (independently
/// built graphs number segments differently).
///
/// Falls back to a rewalk when the arena has nothing to project: HEAD is unborn (e.g. its
/// referent was deleted without a repoint) or points outside the editor's graph.
fn refresh_workspace_from_arena<M: RefMetadata>(
    graph: &crate::graph_rebase::StepGraph,
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
    if std::env::var_os("BUT_REBASE_WRITE_THROUGH").is_some_and(|v| v == "assert") {
        let rewalked = but_graph::Workspace::from_head(repo, meta, project_meta, options)?;
        let (mutated_fp, rewalked_fp) = (
            projection_fingerprint(workspace),
            projection_fingerprint(&rewalked),
        );
        if mutated_fp != rewalked_fp {
            bail!(
                "WRITE-THROUGH DIVERGENCE\n--- mutate-then-project\n{mutated_fp}\n--- rewalk-then-project\n{rewalked_fp}"
            );
        }
    }
    Ok(())
}

/// The parity view: everything the projection derives that is NOT a graph index — kind,
/// bounds, target, stacks, segments, per-commit ids/parents/flags/refs, remote and outside
/// commit sets. Graph-index-dependent fields (segment indices, sibling links) are excluded
/// since independently built graphs number segments differently.
fn projection_fingerprint(ws: &but_graph::Workspace) -> String {
    use std::fmt::Write as _;
    let commit_line = |out: &mut String, prefix: &str, c: &but_graph::workspace::StackCommit| {
        writeln!(
            out,
            "{prefix}{} parents=[{}] flags={:?} refs=[{}]",
            c.id,
            c.parent_ids
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            c.flags,
            c.refs
                .iter()
                .map(|r| r.ref_name.as_bstr().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )
        .ok();
    };
    let mut out = String::new();
    let kind = match &ws.kind {
        but_graph::workspace::WorkspaceKind::Managed { ref_info } => {
            format!("Managed({})", ref_info.ref_name.as_bstr())
        }
        but_graph::workspace::WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
            format!("ManagedMissing({})", ref_info.ref_name.as_bstr())
        }
        but_graph::workspace::WorkspaceKind::AdHoc => "AdHoc".to_string(),
    };
    writeln!(
        out,
        "kind={kind} lower_bound={:?} target_ref={:?} target_commit={:?} metadata={}",
        ws.lower_bound,
        ws.target_ref
            .as_ref()
            .map(|t| (t.ref_name.as_bstr().to_string(), t.commits_ahead)),
        ws.target_commit.as_ref().map(|t| t.commit_id),
        ws.metadata.is_some(),
    )
    .ok();
    for stack in &ws.stacks {
        writeln!(out, "stack {:?}", stack.id).ok();
        for segment in &stack.segments {
            writeln!(
                out,
                "  {} base={:?} remote={:?} projected_name={} entrypoint={} metadata={}",
                segment
                    .ref_name()
                    .map_or_else(|| "<anon>".to_string(), |n| n.as_bstr().to_string()),
                segment.base,
                segment
                    .remote_tracking_ref_name
                    .as_ref()
                    .map(|n| n.as_bstr().to_string()),
                segment.name_projected_from_outside,
                segment.is_entrypoint,
                segment.metadata.is_some(),
            )
            .ok();
            for commit in &segment.commits {
                commit_line(&mut out, "    ", commit);
            }
            for commit in &segment.commits_on_remote {
                commit_line(&mut out, "    remote ", commit);
            }
            for commit in segment.commits_outside.iter().flatten() {
                commit_line(&mut out, "    outside ", commit);
            }
        }
    }
    out
}
