//! Perform the actual rebase: rewrite mutable commits in the in-memory
//! repository and seal the graph back into a validated [`NodeGraph`].

use std::{collections::BTreeMap, fmt::Write as _};

use anyhow::{Result, bail};

use crate::{
    NodeGraph, NodeGraphEntrypoint, NodeIndex, NodeKind,
    edit::{
        EditSession, MutableNodeGraph, NodePolicy,
        cherry_pick::{CherryPickOutcome, cherry_pick},
        recompute_annotations, resolve_reference_targets,
    },
    node::{collect_ordered_parents, is_commit_like, resolve_to_commit, topological_order},
};

/// A rewritten, sealed graph together with the edit state needed to preview,
/// keep editing, or materialize it.
///
/// The new commits it refers to exist only in the in-memory repository
/// ([`Self::repo`]) until [`Self::materialize_changes`] persists them.
#[derive(Debug)]
pub struct Rebased {
    /// The sealed post-rebase graph. Node indexes are identical to the mutable
    /// graph this was produced from.
    pub graph: NodeGraph,
    pub(crate) policy: Vec<NodePolicy>,
    pub(crate) session: EditSession,
}

impl Rebased {
    /// Returns the in-memory repository that backs this rebase preview.
    ///
    /// This repository may contain objects that have not been persisted yet.
    pub fn repo(&self) -> &gix::Repository {
        &self.session.repo
    }

    /// Synthesize the [`crate::edit::Pick`] stored at `index` in the rebased
    /// graph, or `None` if the node is not an addressable commit.
    pub fn pick_at(&self, index: NodeIndex) -> Option<crate::edit::Pick> {
        crate::edit::pick_of(self.graph.nodes(), &self.policy, index)
    }

    /// Read the mutability of the reference node at `index` in the rebased
    /// graph, or `None` if the node is not a reference.
    pub fn reference_mutability(&self, index: NodeIndex) -> Option<bool> {
        crate::edit::reference_mutability_of(self.graph.nodes(), &self.policy, index)
    }

    /// Provides a mapping between commits that were rewritten as part of the
    /// transformation, from original to rewritten id.
    pub fn commit_mappings(&self) -> BTreeMap<gix::ObjectId, gix::ObjectId> {
        self.session.commit_mappings.mappings()
    }

    /// Convert back into a mutable graph for multi-step operations.
    ///
    /// The edit session (in-memory repository, commit mappings, initial
    /// references) carries over, so chained rebases accumulate.
    pub fn into_mut(self) -> MutableNodeGraph {
        let Rebased {
            graph,
            policy,
            session,
        } = self;
        MutableNodeGraph {
            nodes: graph.nodes,
            context: graph.context,
            policy,
            session,
        }
    }

    /// Return the commit targeted by `ref_name` in the post-rebase graph.
    pub fn reference_target(&self, ref_name: &gix::refs::FullNameRef) -> Result<gix::ObjectId> {
        let Some((reference, _)) = self.graph.node_by_ref_name(ref_name) else {
            bail!("Could not find reference '{ref_name}' in rebase result");
        };
        resolve_to_commit(self.graph.nodes(), reference)
            .and_then(|target| self.graph.nodes()[target].kind().addressable_commit_id())
            .ok_or_else(|| anyhow::anyhow!("Reference has no target commit in rebase result"))
    }

    /// Returns a preview of what the workspace will look like after
    /// materialization, projected directly from the rebased graph.
    ///
    /// Any objects referenced in the projection must be accessed via the
    /// in-memory repository owned by this [`Rebased`] (`self.repo()`), since
    /// they might exist only in memory.
    pub fn workspace(&self) -> Result<crate::Workspace> {
        self.graph.clone().into_workspace()
    }
}

impl MutableNodeGraph {
    /// Perform the rebase.
    ///
    /// Walks the graph parents-first, cherry-picking every mutable commit onto
    /// its (possibly rewritten) parents in the in-memory repository, then seals
    /// the graph: reference targets are refreshed from topology, annotations
    /// recomputed, and the result validated.
    ///
    /// References are not written here — [`Rebased::materialize_changes`]
    /// computes the reference transaction by diffing the sealed graph against
    /// the on-disk repository.
    pub fn rebase(self) -> Result<Rebased> {
        let MutableNodeGraph {
            mut nodes,
            mut context,
            mut policy,
            mut session,
        } = self;

        // Lanes that gained commits during the edit join the workspace merge.
        crate::edit::adopt_workspace_lanes(
            &mut nodes,
            context.managed_workspace_commit_id,
            &session.unmerged_lanes,
        );

        // Process parents before children so every pick lands on already
        // rewritten parents. Commit ids are swapped in place, which is safe
        // because a node's own id is only read at its position in the order.
        for node_idx in topological_order(&nodes)? {
            let NodeKind::Commit { id } = *nodes[node_idx].kind() else {
                continue;
            };
            let NodePolicy::Pick(settings) = policy[node_idx].clone() else {
                bail!("BUG: commit node {node_idx} without pick settings");
            };
            let pick = settings.with_id(id);
            // Immutable picks are copied verbatim: the commit keeps its id, so
            // there's no cherry-pick to run and nothing to record in the
            // mapping.
            if !pick.mutable {
                continue;
            }

            let ontos = match pick.preserved_parents.clone() {
                Some(ontos) => ontos,
                None => collect_ordered_parents(&nodes, node_idx)
                    .into_iter()
                    .map(|parent| {
                        nodes[parent].kind().addressable_commit_id().ok_or_else(|| {
                            anyhow::anyhow!("A parent in the output graph is not a pick")
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            };

            let outcome = cherry_pick(
                &session.repo,
                pick.id,
                &ontos,
                pick.pick_mode,
                pick.tree_merge_mode,
                pick.sign_commit,
            )?;

            if matches!(outcome, CherryPickOutcome::ConflictedCommit(_)) && !pick.conflictable {
                bail!(
                    "Commit {} was marked as not conflictable, but resulted in a conflicted state",
                    pick.id
                );
            }

            match outcome {
                CherryPickOutcome::Commit(new_id)
                | CherryPickOutcome::ConflictedCommit(new_id)
                | CherryPickOutcome::Identity(new_id) => {
                    nodes[node_idx].set_kind(NodeKind::Commit { id: new_id });
                    if !pick.exclude_from_tracking {
                        session.commit_mappings.update(pick.id, new_id);
                    }
                    if context.managed_workspace_commit_id == Some(pick.id) {
                        context.managed_workspace_commit_id = Some(new_id);
                    }
                }
                CherryPickOutcome::FailedToMergeBases {
                    base_merge_failed,
                    bases,
                    onto_merge_failed,
                    ontos,
                } => {
                    // Exit early - the rebase failed because it encountered a commit it couldn't pick
                    bail!(format_base_merge_error(
                        pick.id,
                        base_merge_failed,
                        bases,
                        onto_merge_failed,
                        ontos
                    ));
                }
            }
        }

        // Seal-time normalization: edits can leave duplicate parent edges on
        // reference nodes, and a workspace reference's overlay parents can
        // point at nodes that are no longer references (e.g. integrated stack
        // tips replaced by tombstones). Neither carries meaning in a sealed
        // graph, so both are dropped before validation.
        for node_idx in 0..nodes.len() {
            let NodeKind::Reference(reference) = nodes[node_idx].kind() else {
                continue;
            };
            let is_workspace = matches!(
                reference.metadata,
                Some(crate::ReferenceMetadata::Workspace(_))
            );
            let parents = nodes[node_idx].parents().to_vec();
            let mut kept: Vec<NodeIndex> = Vec::with_capacity(parents.len());
            if is_workspace {
                let Some((&own_target, overlays)) = parents.split_last() else {
                    continue;
                };
                for &overlay in overlays {
                    if matches!(nodes[overlay].kind(), NodeKind::Reference(_))
                        && overlay != own_target
                        && !kept.contains(&overlay)
                    {
                        kept.push(overlay);
                    }
                }
                kept.push(own_target);
            } else {
                for &parent in &parents {
                    if !kept.contains(&parent) {
                        kept.push(parent);
                    }
                }
            }
            if kept != parents {
                *nodes[node_idx].parents_mut() = kept;
            }
        }

        // Seal: refresh reference targets from topology. References whose
        // commit chain died are left behind on disk untouched, with a trace.
        resolve_reference_targets(&mut nodes, &mut policy, &mut session.left_behind);

        // Re-resolve the entrypoint. The checked-out reference is
        // authoritative when `HEAD` was symbolic: edits may have moved that
        // reference onto a different commit, or removed the node the
        // entrypoint pointed at.
        match &context.entrypoint {
            NodeGraphEntrypoint::Node(index) => {
                let symbolic_target = context
                    .entrypoint_ref
                    .as_ref()
                    .and_then(|name| crate::edit::node_index_by_ref_name(&nodes, name.as_ref()))
                    .and_then(|ref_index| resolve_to_commit(&nodes, ref_index))
                    .filter(|target| is_commit_like(&nodes, *target));
                if let Some(target) = symbolic_target {
                    context.entrypoint = NodeGraphEntrypoint::Node(target);
                } else if !is_commit_like(&nodes, *index) {
                    let resolved = resolve_to_commit(&nodes, *index)
                        .filter(|target| is_commit_like(&nodes, *target));
                    match resolved {
                        Some(target) => context.entrypoint = NodeGraphEntrypoint::Node(target),
                        None => {
                            bail!("The edit removed every commit the entrypoint could resolve to")
                        }
                    }
                }
            }
            NodeGraphEntrypoint::Unborn(reference) => {
                // A formerly unborn reference that gained history becomes a
                // born entrypoint.
                let ref_name = reference.ref_info.ref_name.clone();
                if let Some(index) = crate::edit::node_index_by_ref_name(&nodes, ref_name.as_ref())
                    && let Some(target) = resolve_to_commit(&nodes, index)
                    && matches!(nodes[target].kind(), NodeKind::Commit { .. })
                {
                    context.entrypoint = NodeGraphEntrypoint::Node(target);
                    context.entrypoint_ref = Some(ref_name);
                }
            }
        }

        // The managed workspace commit may have been removed outright.
        if let Some(managed_id) = context.managed_workspace_commit_id
            && !nodes
                .iter()
                .any(|node| matches!(node.kind(), NodeKind::Commit { id } if *id == managed_id))
        {
            context.managed_workspace_commit_id = None;
        }

        let annotations = recompute_annotations(&nodes, &context);

        let graph = NodeGraph {
            nodes,
            annotations,
            context,
        }
        .validated()?;

        Ok(Rebased {
            graph,
            policy,
            session,
        })
    }
}

impl NodeGraph {
    /// The node the entrypoint stands on for display and checkout purposes,
    /// preferring the symbolic entrypoint reference's node.
    pub fn head_index(&self) -> Option<NodeIndex> {
        let symbolic = self
            .entrypoint_ref()
            .and_then(|name| self.node_by_ref_name(name).map(|(index, _)| index));
        match (self.entrypoint(), symbolic) {
            (_, Some(symbolic)) => Some(symbolic),
            (NodeGraphEntrypoint::Node(index), None) => Some(*index),
            (NodeGraphEntrypoint::Unborn(_), None) => None,
        }
    }

    /// Resolve the entrypoint to the commit `HEAD` must point at after
    /// materialization, alongside the symbolic reference name if `HEAD`
    /// should stay symbolic.
    ///
    /// `preferred` is the stable node index `HEAD` was attached to when
    /// editing started; if that node still exists, HEAD follows it even when
    /// the reference it holds was replaced with a differently-named one.
    pub(crate) fn resolve_head(
        &self,
        preferred: Option<NodeIndex>,
    ) -> Result<(gix::ObjectId, Option<gix::refs::FullName>)> {
        let preferred = preferred.filter(|&index| {
            !matches!(
                self.nodes()[index].kind(),
                NodeKind::None | NodeKind::Boundary { .. }
            )
        });
        let Some(head) = preferred.or_else(|| self.head_index()) else {
            bail!("Cannot resolve HEAD for an unborn workspace");
        };
        let target = if is_commit_like(self.nodes(), head) {
            head
        } else {
            resolve_to_commit(self.nodes(), head)
                .ok_or_else(|| anyhow::anyhow!("No target commit for checkout reference"))?
        };
        let id = self.nodes()[target]
            .kind()
            .addressable_commit_id()
            .ok_or_else(|| anyhow::anyhow!("HEAD does not resolve to an addressable commit"))?;
        let symbolic = match self.nodes()[head].kind() {
            NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.clone()),
            _ => None,
        };
        Ok((id, symbolic))
    }
}

fn format_base_merge_error(
    target: gix::ObjectId,
    base_merge_failed: bool,
    bases: Option<Vec<gix::ObjectId>>,
    onto_merge_failed: bool,
    ontos: Option<Vec<gix::ObjectId>>,
) -> String {
    fn fmt_side(out: &mut String, kind: &str, failed: bool, shas: Option<Vec<gix::ObjectId>>) {
        if failed {
            if let Some(shas) = shas {
                writeln!(
                    out,
                    "Encountered a conflict while merging the commit's {kind}: {}.",
                    shas.iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .ok();
            } else {
                writeln!(
                    out,
                    "Encountered a conflict while merging the commit's {kind}."
                )
                .ok();
            }
        }
    }

    let mut out = "".to_string();
    writeln!(
        &mut out,
        "Failed to merge bases while cherry picking commit {target}."
    )
    .ok();
    fmt_side(&mut out, "original bases", base_merge_failed, bases);
    fmt_side(&mut out, "new bases", onto_merge_failed, ontos);
    writeln!(
        &mut out,
        "Any ids mentioned may be in-memory and inaccessible through the git CLI."
    )
    .ok();
    out
}
