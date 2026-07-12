#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darkness bind them.
//!
//! ---
//!
//! A graph-based rebase engine. The workspace is loaded into an `Editor` as a `GraphEditor`: an
//! arena of `Step`s where a `Pick` is a commit to cherry-pick, while a `Reference` (a branch)
//! rides as positioned data on a pick.
//! Callers mutate the graph (insert/move/remove picks, create/move references), then
//! `Editor::rebase` replays it — cherry-picking every mutable pick onto its new parents and
//! producing the reference updates to write.
//!
//! References are POSITIONS, not entries with edges — see the `positions` module for the model.

mod creation;
mod graph_editor;
mod positions;
pub mod rebase;
mod ref_ops;
pub mod traverse;
use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use but_core::{RefMetadata, commit::SignCommit, ref_metadata::ProjectMeta};
use but_graph::walk::Overlay;
pub use creation::GraphEditorOptions;
use gix::refs::transaction::RefEdit;

use crate::graph_rebase::cherry_pick::{PickMode, TreeMergeMode};
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod merge_commit_changes;
pub mod mutate;
pub mod ordering;
pub mod selector;
pub(crate) mod util;
pub mod workspace;
pub use workspace::{GraphWorkspace, Subgraph};

/// Utilities for testing
pub mod testing;

/// Used to manipulate a set of picks.
#[derive(Debug)]
pub struct Editor<'meta, M: RefMetadata> {
    /// The internal graph of steps
    graph: GraphEditor,
    /// Initial references, used to spot references that need deleting.
    initial_references: Vec<gix::refs::FullName>,
    /// Worktrees that we might need to perform `safe_checkout` on.
    checkouts: Vec<Checkout>,
    /// The in-memory repository that the rebase engine works with.
    repo: gix::Repository,
    /// Provides data about how the editor instance was transformed.
    history: RevisionHistory,
    /// The workspace target configuration the editor was created with.
    project_meta: ProjectMeta,
    /// A reference to the metadata that the editor was created for.
    meta: &'meta mut M,
}

impl<'meta, M: RefMetadata> Editor<'meta, M> {
    pub(crate) fn new_selector(&self, id: impl Into<EditorIndex>) -> Selector {
        let id = id.into();
        Selector { id }
    }
}

/// Represents a successful rebase, and any valid, but potentially conflicting scenarios it had.
#[derive(Debug)]
pub struct SuccessfulRebase<'meta, M: RefMetadata> {
    pub(crate) repo: gix::Repository,
    pub(crate) initial_references: Vec<gix::refs::FullName>,
    /// Any reference edits that need to be committed as a result of the history
    /// rewrite
    pub(crate) ref_edits: Vec<RefEdit>,
    /// The new commit graph
    pub(crate) graph: GraphEditor,
    pub(crate) checkouts: Vec<Checkout>,
    /// Provides data about how the editor instance was transformed.
    pub history: RevisionHistory,
    /// The workspace target configuration the editor was created with.
    project_meta: ProjectMeta,
    /// A reference to the metadata that the editor was created for.
    meta: &'meta mut M,
}

impl<'meta, M: RefMetadata> SuccessfulRebase<'meta, M> {
    /// Returns the in-memory repository that backs this rebase preview.
    ///
    /// This repository may contain objects that have not been persisted yet,
    /// which makes it suitable for dry-run inspection of a [`Self::rebase_overlay`] redo.
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Returns the preview repository together with mutable access to the
    /// ref-metadata the editor was created with.
    ///
    /// Use this to build post-rebase projections that need both, like a
    /// workspace preview computed from [`Self::rebase_overlay`].
    pub fn repo_and_meta_mut(&mut self) -> (&gix::Repository, &mut M) {
        (&self.repo, self.meta)
    }

    /// Returns the ref-metadata the editor was created with.
    pub fn meta(&self) -> &M {
        self.meta
    }

    /// The mutated commit graph this rebase will materialize — already the next workspace
    /// state, since materializing only persists objects and applies ref edits. Project it
    /// (with [`Self::repo`] and [`Self::rebase_overlay`]) to preview without a rewalk.
    pub fn arena(&self) -> &but_graph::CommitGraph {
        self.graph.arena()
    }

    /// Return the commit targeted by `ref_name` in the post-rebase graph.
    pub fn reference_target(&self, ref_name: &gix::refs::FullNameRef) -> Result<gix::ObjectId> {
        let (ref_idx, ..) = self
            .graph
            .references()
            .find(|(_, refname, _)| refname.as_ref() == ref_name)
            .with_context(|| format!("Could not find reference '{ref_name}' in rebase result"))?;
        crate::graph_rebase::positions::resolve_to_pick(&self.graph, ref_idx)
            .and_then(|pick| self.graph.commit_id(pick))
            .context("Reference has no target commit in rebase result")
    }

    /// The overlay describing this rebase's outcome: updated/dropped refs plus the requested
    /// checkout as the entrypoint.
    ///
    /// Feed it to a graph or workspace redo (with [`Self::repo`], since rewritten objects may
    /// exist only in memory) to preview the post-rebase state without materializing.
    pub fn rebase_overlay(&self) -> Result<Overlay> {
        self.rebase_overlay_with_workspace_overrides(None, None)
    }

    /// Like [`Self::rebase_overlay`], with ad-hoc workspace projection overrides.
    ///
    /// For dry-run operations whose graph rewrite is accompanied by metadata or checkout
    /// changes that are deliberately not persisted: the override `entrypoint` names the
    /// commit and local reference that would be checked out, while `branch_stack_order`
    /// supplies the tip-to-base order that would be written to ref metadata.
    pub fn rebase_overlay_with_workspace_overrides(
        &self,
        entrypoint: Option<(gix::ObjectId, gix::refs::FullName)>,
        branch_stack_order: Option<&[gix::refs::FullName]>,
    ) -> Result<Overlay> {
        let dropped_refs = self.ref_edits.iter().filter_map(|edit| match &edit.change {
            gix::refs::transaction::Change::Delete { .. } => Some(edit.name.clone()),
            _ => None,
        });
        let updated_refs = self.ref_edits.iter().filter_map(|edit| match &edit.change {
            gix::refs::transaction::Change::Update { new, .. } => Some(gix::refs::Reference {
                name: edit.name.clone(),
                target: new.clone(),
                // TODO(CTO): Peeled is only relevant for symbolic refs?
                peeled: None,
            }),
            _ => None,
        });

        let Some((entrypoint_id, entrypoint_refname)) = self
            .checkouts
            .iter()
            .filter_map(|checkout| match checkout {
                Checkout::Head { selector, .. } => match self.graph.step_view(selector.id) {
                    Step::None => None,
                    Step::Pick(Pick { id, .. }) => Some((id, None)),
                    Step::Reference { refname, .. } => {
                        if let Some(to_reference) = crate::graph_rebase::positions::resolve_to_pick(
                            &self.graph,
                            selector.id,
                        ) && let Some(id) = self.graph.commit_id(to_reference)
                        {
                            Some((id, Some(refname)))
                        } else {
                            None
                        }
                    }
                },
            })
            .next()
        else {
            bail!("BUG: Tried to construct rebase engine graph overlay with no entrypoints");
        };

        let (entrypoint_id, entrypoint_refname) = entrypoint
            .map_or((entrypoint_id, entrypoint_refname), |(id, ref_name)| {
                (id, Some(ref_name))
            });
        let mut overlay = Overlay::default()
            .with_references(updated_refs)
            .with_dropped_references(dropped_refs)
            .with_entrypoint(entrypoint_id, entrypoint_refname);
        if let Some(branch_stack_order) = branch_stack_order {
            overlay = overlay.with_branch_stack_order_override(branch_stack_order.iter().cloned());
        }
        Ok(overlay)
    }
}

/// The outcome of a materialize
#[derive(Debug)]
pub struct MaterializeOutcome<'meta, M: RefMetadata> {
    pub(crate) graph: GraphEditor,
    /// Provides data about how the editor instance was transformed.
    pub history: RevisionHistory,
    /// A reference to the metadata that the editor was created for.
    pub meta: &'meta mut M,
}

impl<'meta, M: RefMetadata> MaterializeOutcome<'meta, M> {
    /// The mutated commit graph the rebase materialized — the next workspace state.
    /// Feed it to `Workspace::refresh_from_commit_graph` to bring a workspace up to date.
    pub fn arena(&self) -> &but_graph::CommitGraph {
        self.graph.arena()
    }
}

/// Provides lookup for different steps that a selector might point to.
pub trait LookupStep {
    /// Look up the step that a given selector corresponds to.
    fn lookup_step(&self, selector: Selector) -> Result<Step>;

    /// Look up the step for a given selector and assert it's a pick.
    fn lookup_pick(&self, selector: Selector) -> Result<gix::ObjectId> {
        match self.lookup_step(selector)? {
            Step::Pick(Pick { id, .. }) => Ok(id),
            _ => bail!("Expected selector to point to a pick"),
        }
    }

    /// Look up the step for a given selector and assert it's a reference.
    fn lookup_reference(&self, selector: Selector) -> Result<gix::refs::FullName> {
        match self.lookup_step(selector)? {
            Step::Reference { refname, .. } => Ok(refname),
            _ => bail!("Expected selector to point to a reference"),
        }
    }
}

impl<M: RefMetadata> LookupStep for Editor<'_, M> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        Ok(self.graph.step_view(selector.id))
    }
}

impl<M: RefMetadata> LookupStep for SuccessfulRebase<'_, M> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        Ok(self.graph.step_view(selector.id))
    }
}

impl<M: RefMetadata> LookupStep for MaterializeOutcome<'_, M> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        Ok(self.graph.step_view(selector.id))
    }
}

/// Represents a commit to be cherry-picked in a rebase operation.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    /// The ID of the commit getting picked
    pub id: gix::ObjectId,
    /// If we are dealing with a sub-graph with an incomplete history, we
    /// need to represent the bottom most commits in a way that we preserve
    /// their parents.
    ///
    /// If this is Some, the commit WILL NOT be picked onto the parents the
    /// graph implies but instead on to the parents listed here.
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    /// Controls under what circumstances the commit is cherry-picked.
    pub pick_mode: PickMode,
    /// Controls whether the resulting commit is signed.
    ///
    /// Note that signing a parent commit only causes descendants to be signed if those descendants
    /// are also picked with a `sign_commit` value that enables signing (e.g. [`SignCommit::Yes`]
    /// or [`SignCommit::IfSignCommitsEnabled`] with config enabled).
    pub sign_commit: SignCommit,
    /// Exclude the commit from being included in the
    /// [`RevisionHistory::commit_mappings()`]. This is helpful if we are
    /// creating a new commit since the mappings will be non-sensical to the
    /// frontend consumers.
    pub exclude_from_tracking: bool,
    /// If set to false, the rebase will fail if this commit results in a
    /// conflicted state. The cherry-pick still runs and creates the
    /// conflicted commit — this check happens afterwards in [`Editor::rebase`].
    pub conflictable: bool,
    /// Controls how parent trees are merged during cherry-pick.
    /// See [`TreeMergeMode`] for details.
    pub tree_merge_mode: TreeMergeMode,
    /// Whether the editor may rewrite this commit.
    ///
    /// The editor contains every commit in the workspace graph, but only those
    /// reachable from a mutable entrypoint (e.g. `HEAD`) should be rewritten.
    /// When `false`, the rebase copies the pick verbatim instead of
    /// cherry-picking it, preserving its id.
    pub mutable: bool,
}

impl Pick {
    /// Creates a pick with the expected defaults
    pub fn new_pick(id: gix::ObjectId) -> Self {
        Self {
            id,
            preserved_parents: None,
            pick_mode: PickMode::IfChanged,
            sign_commit: SignCommit::IfSignCommitsEnabled,
            exclude_from_tracking: false,
            conflictable: true,
            tree_merge_mode: TreeMergeMode::WithRenames,
            mutable: true,
        }
    }

    /// Creates a pick with the expected defaults, but is excluded from being
    /// included from the [`RevisionHistory::commit_mappings()`] output. This is
    /// often preferable if you are doing something like an
    /// `insert_blank_commit` operation.
    pub fn new_untracked_pick(id: gix::ObjectId) -> Self {
        let mut pick = Self::new_pick(id);
        pick.exclude_from_tracking = true;
        pick
    }

    /// Creates a pick with the defaults set for a workspace commit
    pub fn new_workspace_pick(id: gix::ObjectId) -> Self {
        Self {
            id,
            preserved_parents: None,
            pick_mode: PickMode::IfChanged,
            sign_commit: SignCommit::No,
            exclude_from_tracking: false,
            conflictable: false,
            tree_merge_mode: TreeMergeMode::WithoutRenames,
            mutable: true,
        }
    }
}

/// Describes what action the engine should take
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Cherry picks the given commit into the new location in the graph
    Pick(Pick),
    /// A reference command, routed into the ref table as positioned data — never a node.
    Reference {
        /// The refname
        refname: gix::refs::FullName,
        /// Whether the editor may move or delete this reference.
        ///
        /// Only references reachable from a mutable entrypoint (e.g. `HEAD`)
        /// are updated during materialization. When `false`, the reference is
        /// kept in the graph for traversal but never written.
        mutable: bool,
    },
    /// A tombstone left behind when a pick or reference is removed.
    ///
    /// The entry is never deleted from the arena — that would invalidate every entry id after it.
    /// Instead the parent number becomes `None`, keeping ids stable. It retains its first
    /// outgoing edge so that resolving a reference downward walks THROUGH it to the next live
    /// pick. A tombstone must not survive into materialized output.
    None,
}

impl Step {
    /// Creates a pick with the expected defaults
    pub fn new_pick(id: gix::ObjectId) -> Self {
        Self::Pick(Pick::new_pick(id))
    }

    /// Creates a pick with the expected defaults, but is excluded from being
    /// included from the [`RevisionHistory::commit_mappings()`] output. This is
    /// often preferable if you are doing something like an
    /// `insert_blank_commit` operation.
    pub fn new_untracked_pick(id: gix::ObjectId) -> Self {
        Self::Pick(Pick::new_untracked_pick(id))
    }

    /// Creates a mutable reference step.
    ///
    /// References constructed by edit operations are mutable; immutable
    /// references only originate from non-`HEAD`-reachable territory during
    /// [`Editor::create`].
    pub fn new_reference(refname: gix::refs::FullName) -> Self {
        Self::Reference {
            refname,
            mutable: true,
        }
    }
}

pub(crate) use graph_editor::{EditorIndex, GraphEditor, PickIndex};

/// Convert a structure to a selector for a particular editor.
///
pub trait ToSelector {
    /// Converts a given object into a selector. Calling `to_selector` on an
    /// object asserts that the receiver was a object that is selectable in the
    /// graph.
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector>;
}

/// Convert a type to a selector, and ensures that it is type commit.
pub trait ToCommitSelector {
    /// Converts a given object into a selector. Calling `to_commit_selector` on
    /// an object asserts that the receiver has a selectable pick step in the
    /// graph.
    fn to_commit_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector>;
}

/// Convert a type to a selector, and ensures that it is type reference.
pub trait ToReferenceSelector {
    /// Converts a given object into a selector. Calling `to_reference_selector` on
    /// an object asserts that the receiver has a selectable reference step in
    /// the graph.
    fn to_reference_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector>;
}

/// Points to a step in the rebase editor.
///
/// Step indices are stable across mutation and rebase — a selector taken at
/// any point remains valid for the lifetime of the editor. Deleted steps
/// become tombstones ([`Step::None`]) rather than being removed, so a selector
/// never dangles, though it may point at a tombstone.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Selector {
    id: EditorIndex,
}

impl ToCommitSelector for Selector {
    fn to_commit_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        let step = editor.graph.step_view(self.id);
        if !matches!(step, Step::Pick(_)) {
            bail!("Expected selector for {step:?} to refer to a commit");
        }

        Ok(*self)
    }
}

impl ToReferenceSelector for Selector {
    fn to_reference_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        if !editor.graph.is_reference(self.id) {
            let step = editor.graph.step_view(self.id);
            bail!("Expected selector for {step:?} to refer to a reference");
        }

        Ok(*self)
    }
}

impl ToSelector for Selector {
    fn to_selector(&self, _: &Editor<impl RefMetadata>) -> Result<Selector> {
        Ok(*self)
    }
}

/// Represents places where `safe_checkout` should be called from
#[derive(Debug, Clone)]
pub(crate) enum Checkout {
    /// The HEAD of the `repo` the editor was created for.
    Head {
        selector: Selector,
        /// A pre-computed merge base tree (`HEAD^{tree}` + consumed changes,
        /// additive-only) to pass through to `safe_checkout`. When set, the
        /// 3-way snapshot merge uses this as the base so consumed hunks cancel
        /// and don't reappear as uncommitted changes.
        merge_base_override: Option<gix::ObjectId>,
    },
}

/// How commit ids moved as the editor transformed the graph.
#[derive(Debug, Clone, Default)]
pub struct RevisionHistory {
    /// A mapping from any commits that were in the original mapping to a
    /// rewritten version.
    ///
    /// Unintuitively, the values are the original values, and the keys are the
    /// _new_ values that they have been mapped to.
    commit_mappings: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

impl RevisionHistory {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    /// Record that `from` was rewritten to `to`. If `from` was itself the result of an
    /// earlier rewrite, the chain collapses: the original id now maps straight to `to`.
    pub(crate) fn update_mapping(&mut self, from: gix::ObjectId, to: gix::ObjectId) {
        if let Some(value) = self.commit_mappings.remove(&from) {
            self.commit_mappings.insert(to, value);
        } else {
            self.commit_mappings.insert(to, from);
        };
    }

    /// Provides a mapping between commits that were rewritten as part of the transformation.
    pub fn commit_mappings(&self) -> BTreeMap<gix::ObjectId, gix::ObjectId> {
        self.commit_mappings
            .iter()
            .filter_map(|(k, v)| if k == v { None } else { Some((*v, *k)) })
            .collect()
    }
}

/// I wanted to assert _somewhere_ the defaults for non-workspace & workspace commits. It doesn't feel like the right place to do it in integration tests because we should assert behaviour rather than details there.
#[cfg(test)]
mod test {
    use std::str::FromStr;

    use but_core::commit::SignCommit;

    use crate::graph_rebase::{
        Pick,
        cherry_pick::{PickMode, TreeMergeMode},
    };

    #[test]
    fn workspace_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            Pick::new_workspace_pick(object_id),
            Pick {
                id: object_id,
                preserved_parents: None,
                pick_mode: PickMode::IfChanged,
                sign_commit: SignCommit::No,
                exclude_from_tracking: false,
                conflictable: false,
                tree_merge_mode: TreeMergeMode::WithoutRenames,
                mutable: true,
            }
        );

        Ok(())
    }

    #[test]
    fn regular_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            Pick::new_pick(object_id),
            Pick {
                id: object_id,
                preserved_parents: None,
                pick_mode: PickMode::IfChanged,
                sign_commit: SignCommit::IfSignCommitsEnabled,
                exclude_from_tracking: false,
                conflictable: true,
                tree_merge_mode: TreeMergeMode::WithRenames,
                mutable: true,
            }
        );

        Ok(())
    }
}
