#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darknes bind them.

mod creation;
pub mod rebase;
use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail};
use but_core::{RefMetadata, commit::SignCommit};
use but_graph::init::Overlay;
pub use creation::GraphEditorOptions;
use gix::refs::transaction::RefEdit;

use crate::graph_rebase::util::collect_ordered_parents;

use crate::graph_rebase::cherry_pick::{PickMode, TreeMergeMode};
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod merge_commit_changes;
pub mod mutate;
pub mod ordering;
pub(crate) mod util;

/// Additional reference to include in an editor, with persistence behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraRef<'a> {
    /// The reference name to include in the editor graph.
    pub ref_name: &'a gix::refs::FullNameRef,
    /// Whether rebases/materialization may update this ref.
    pub mutability: ExtraRefMutability,
}

impl<'a> ExtraRef<'a> {
    /// Track an extra ref and allow the editor to update it.
    pub fn mutable(ref_name: &'a gix::refs::FullNameRef) -> Self {
        Self {
            ref_name,
            mutability: ExtraRefMutability::Mutable,
        }
    }

    /// Track an extra ref for traversal only, without persisting updates.
    pub fn immutable(ref_name: &'a gix::refs::FullNameRef) -> Self {
        Self {
            ref_name,
            mutability: ExtraRefMutability::Immutable,
        }
    }
}

/// Controls whether an extra ref may be updated by editor materialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraRefMutability {
    /// The ref may be rewritten and deleted as needed by the edited graph.
    Mutable,
    /// The ref is available for graph traversal but must not be persisted.
    Immutable,
}

/// Utilities for testing
pub mod testing;

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
    /// creating a new commit since the the mappings will be non-sensical to the
    /// frontend consumers.
    pub exclude_from_tracking: bool,
    /// If set to false, the rebase will fail if this commit results in a
    /// conflicted state. The cherry-pick still runs and creates the
    /// conflicted commit — this check happens afterwards in [`Editor::rebase`].
    pub conflictable: bool,
    /// Controls how parent trees are merged during cherry-pick.
    /// See [`TreeMergeMode`] for details.
    pub tree_merge_mode: TreeMergeMode,
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
        }
    }
}

/// Describes what action the engine should take
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Cherry picks the given commit into the new location in the graph
    Pick(Pick),
    /// Represents applying a reference to the commit found at it's first parent
    Reference {
        /// The refname
        refname: gix::refs::FullName,
    },
    /// Used as a placeholder after removing a pick or reference
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
}

/// Used to represent a connection between a given commit.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
    /// Represents in which order the `parent` fields should be written out
    ///
    /// A child commit should have edges that all have unique orders. In order
    /// to achive that we can employ the following semantics.
    order: usize,
}

type StepGraphIndex = petgraph::stable_graph::NodeIndex;
type StepGraph = petgraph::stable_graph::StableDiGraph<Step, Edge>;

/// Convert a structure to a selector for a particular editor.
///
/// `ToSelector` does _not_ normalize a selector.
pub trait ToSelector {
    /// Converts a given object into a selector. Calling `to_selector` on an
    /// object asserts that the reciever was a object that is selectable in the
    /// graph.
    fn to_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector>;
}

/// Convert a type to a selector, and ensures that it is type commit.
pub trait ToCommitSelector {
    /// Converts a given object into a selector. Calling `to_commit_selector` on
    /// an object asserts that the reciever has a selectable pick step in the
    /// graph.
    fn to_commit_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector>;
}

/// Convert a type to a selector, and ensures that it is type reference.
pub trait ToReferenceSelector {
    /// Converts a given object into a selector. Calling `to_reference_selector` on
    /// an object asserts that the reciever has a selectable reference step in
    /// the graph.
    fn to_reference_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector>;
}

/// Points to a step in the rebase editor.
///
/// Hash, PartialEq, and Eq are implemented for this struct. Because selectors
/// are a pointer to a node in a particular version of the Editor's internal
/// representation, it means that you can have two selectors that when
/// normalised point to the same node. If you want to ensure you have just one
/// selector to a given node, make sure you are working with selectors all
/// normalised to the latest revision of the Editor.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Selector {
    id: StepGraphIndex,
    revision: usize,
}

impl ToCommitSelector for Selector {
    fn to_commit_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        let selector = editor.history.normalize_selector(*self)?;
        let step = &editor.graph[selector.id];
        if !matches!(step, Step::Pick(_)) {
            bail!("Expected selector for {step:?} to refer to a commit");
        }

        Ok(selector)
    }
}

impl ToReferenceSelector for Selector {
    fn to_reference_selector(&self, editor: &Editor<impl RefMetadata>) -> Result<Selector> {
        let selector = editor.history.normalize_selector(*self)?;
        let step = &editor.graph[selector.id];
        if !matches!(step, Step::Reference { .. }) {
            bail!("Expected selector for {step:?} to refer to a reference");
        }

        Ok(selector)
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

/// Used to manipulate a set of picks.
#[derive(Debug)]
pub struct Editor<'ws, 'meta, M: RefMetadata> {
    /// The internal graph of steps
    graph: StepGraph,
    /// Initial references. This is used to track any references that might need
    /// deleted.
    initial_references: Vec<gix::refs::FullName>,
    /// Worktrees that we might need to perform `safe_checkout` on.
    checkouts: Vec<Checkout>,
    /// The in-memory repository that the rebase engine works with.
    repo: gix::Repository,
    /// Provides data about how the editor instance was transformed.
    history: RevisionHistory,
    /// References that should remain selectable in the graph but must never be
    /// updated or deleted during materialization.
    immutable_references: std::collections::HashSet<gix::refs::FullName>,
    /// A reference to the workspace that the editor was created for.
    workspace: &'ws mut but_graph::Workspace,
    /// A reference to the metadata that the editor was created for.
    meta: &'meta mut M,
}

/// Represents a successful rebase, and any valid, but potentially conflicting scenarios it had.
#[derive(Debug)]
pub struct SuccessfulRebase<'ws, 'meta, M: RefMetadata> {
    pub(crate) repo: gix::Repository,
    pub(crate) initial_references: Vec<gix::refs::FullName>,
    /// Any reference edits that need to be committed as a result of the history
    /// rewrite
    pub(crate) ref_edits: Vec<RefEdit>,
    /// The new step graph
    pub(crate) graph: StepGraph,
    pub(crate) checkouts: Vec<Checkout>,
    /// Provides data about how the editor instance was transformed.
    pub history: RevisionHistory,
    pub(crate) immutable_references: std::collections::HashSet<gix::refs::FullName>,
    /// A reference to the workspace that the editor was created for.
    workspace: &'ws mut but_graph::Workspace,
    /// A reference to the metadata that the editor was created for.
    meta: &'meta mut M,
}

impl<'ws, 'meta, M: RefMetadata> SuccessfulRebase<'ws, 'meta, M> {
    /// Returns the in-memory repository that backs this rebase preview.
    ///
    /// This repository may contain objects that have not been persisted yet,
    /// which makes it suitable for dry-run inspection of [`Self::overlayed_graph`].
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Returns a preview of what the but-graph will look like after
    /// materialization.
    ///
    /// Any objects referenced in the resulting graph must be accessed via the
    /// in-memory repository owned by this [`SuccessfulRebase`] (`self.repo`),
    /// since they might exist only in memory.
    pub fn overlayed_graph(&self) -> Result<but_graph::Graph> {
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
                Checkout::Head { selector, .. } => {
                    let selector = self.history.normalize_selector(*selector).ok()?;
                    let step = &self.graph[selector.id];

                    match step {
                        Step::None => None,
                        Step::Pick(Pick { id, .. }) => Some((*id, None)),
                        Step::Reference { refname } => {
                            let parents = collect_ordered_parents(&self.graph, selector.id);

                            if let Some(to_reference) = parents.first()
                                && let Step::Pick(Pick { id, .. }) = self.graph[*to_reference]
                            {
                                Some((id, Some(refname.clone())))
                            } else {
                                None
                            }
                        }
                    }
                }
            })
            .next()
        else {
            bail!("BUG: Tried to construct rebase engine graph overlay with no entrypoints");
        };

        let overlay = Overlay::default()
            .with_references(updated_refs)
            .with_dropped_references(dropped_refs)
            .with_entrypoint(entrypoint_id, entrypoint_refname);
        self.workspace
            .graph
            .redo_traversal_with_overlay(&self.repo, self.meta, overlay)
    }
}

/// The outcome of a materialize
#[derive(Debug)]
pub struct MaterializeOutcome<'ws, 'meta, M: RefMetadata> {
    pub(crate) graph: StepGraph,
    /// Provides data about how the editor instance was transformed.
    pub history: RevisionHistory,
    /// A reference to the workspace that the editor was created for.
    pub workspace: &'ws mut but_graph::Workspace,
    /// A reference to the metadata that the editor was created for.
    pub meta: &'meta mut M,
}

/// Provides lookup for different steps that a selector might point to.
pub trait LookupStep {
    /// Look up the step that a given selector corresponds to.
    fn lookup_step(&self, selector: Selector) -> Result<Step>;

    /// Look up the step a given selector and assert it's a pick.
    fn lookup_pick(&self, selector: Selector) -> Result<gix::ObjectId> {
        match self.lookup_step(selector)? {
            Step::Pick(Pick { id, .. }) => Ok(id),
            _ => bail!("Expected selector to point to a pick"),
        }
    }

    /// Look up the step a given selector and assert it's a pick.
    fn lookup_reference(&self, selector: Selector) -> Result<gix::refs::FullName> {
        match self.lookup_step(selector)? {
            Step::Reference { refname } => Ok(refname),
            _ => bail!("Expected selector to point to a reference"),
        }
    }
}

impl<M: RefMetadata> LookupStep for Editor<'_, '_, M> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

impl<M: RefMetadata> LookupStep for SuccessfulRebase<'_, '_, M> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

impl<M: RefMetadata> LookupStep for MaterializeOutcome<'_, '_, M> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

fn lookup_step(graph: &StepGraph, history: &RevisionHistory, selector: Selector) -> Result<Step> {
    let normalized = history.normalize_selector(selector)?;
    Ok(graph[normalized.id].clone())
}

/// Provides data about how the editor instance was transformed.
#[derive(Debug, Clone, Default)]
pub struct RevisionHistory {
    mappings: Vec<HashMap<StepGraphIndex, StepGraphIndex>>,
    /// A mapping from any commits that were in the original mapping to a
    /// rewritten version.
    ///
    /// Unintuatively, the values are the original values, and the keys are the
    /// _new_ values that they have been mapped to.
    commit_mappings: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

impl<'ws, 'meta, M: RefMetadata> Editor<'ws, 'meta, M> {
    pub(crate) fn new_selector(&self, id: StepGraphIndex) -> Selector {
        Selector {
            id,
            revision: self.history.current_revision(),
        }
    }
}

impl RevisionHistory {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    pub(crate) fn current_revision(&self) -> usize {
        self.mappings.len()
    }

    /// The commit mappings starts empty, and gets updated when we perform a cherry pick.
    /// If there is no entry whose old `to` that cooresponds with the new
    /// `from`, then we just add a `to <- from` entry.
    /// If there is an entry whose old `to` that cooresponds with the new
    /// `from`, then we replace `old_to <- old_from` with `new_to <- old_from`
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

    pub(crate) fn normalize_selector(&self, mut selector: Selector) -> Result<Selector> {
        while selector.revision < self.current_revision() {
            selector.id = *self.mappings[selector.revision]
                .get(&selector.id)
                .context("Failed to normalize selector, selector was missing from the mapping")?;
            selector.revision += 1;
        }
        Ok(selector)
    }

    pub(crate) fn add_revision(&mut self, mapping: HashMap<StepGraphIndex, StepGraphIndex>) {
        self.mappings.push(mapping);
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
            }
        );

        Ok(())
    }
}
