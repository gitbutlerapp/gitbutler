//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darknes bind them.

mod creation;
pub mod rebase;
pub mod traverse;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, bail};
use but_core::ChangeId;
use but_core::commit::CommitIdentifiers;
use but_core::commit::{SignCommit, padding};
use but_graph::init::Overlay;
pub use creation::GraphEditorOptions;
use gix::{
    prelude::ObjectIdExt as _,
    refs::{
        Target,
        transaction::{Change, PreviousValue, RefEdit},
    },
};

use crate::graph_rebase::util::{OrderedParentKind, collect_ordered_parents};

use crate::graph_rebase::cherry_pick::{PickMode, TreeMergeMode};
mod branch_stacks;
pub use branch_stacks::BranchStacks;
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod merge_commit_changes;
mod merge_tree;
pub mod mutate;
pub mod ordering;
pub(crate) mod util;
pub mod workspace;
pub use workspace::{GraphWorkspace, Subgraph};

pub mod testing;

#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub id: gix::ObjectId,
    /// If we are dealing with a sub-graph with an incomplete history, we
    /// need to represent the bottom most commits in a way that we preserve
    /// their parents.
    ///
    /// If this is Some, the commit WILL NOT be picked onto the parents the
    /// graph implies but instead on to the parents listed here.
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    pub pick_mode: PickMode,
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
    pub tree_merge_mode: TreeMergeMode,
    /// The editor contains every commit in the workspace graph, but only those
    /// reachable from a mutable entrypoint (e.g. `HEAD`) should be rewritten.
    /// When `false`, the rebase copies the pick verbatim instead of
    /// cherry-picking it, preserving its id.
    pub mutable: bool,
}

impl Pick {
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

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Pick(Pick),
    /// Points at the commit found at its first parent
    Reference {
        refname: gix::refs::FullName,
        /// Only references reachable from a mutable entrypoint (e.g. `HEAD`)
        /// are updated during materialization. When `false`, the reference is
        /// kept in the graph for traversal but never written.
        mutable: bool,
    },
    /// Used as a placeholder after removing a pick or reference
    None,
}

impl Step {
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
    /// references only originate from non-`HEAD`-reachable segments during
    /// [`Editor::create`].
    pub fn new_reference(refname: gix::refs::FullName) -> Self {
        Self::Reference {
            refname,
            mutable: true,
        }
    }
}

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

/// `ToSelector` does _not_ normalize a selector.
pub trait ToSelector {
    /// Calling `to_selector` on an
    /// object asserts that the reciever was a object that is selectable in the
    /// graph.
    fn to_selector(&self, editor: &Editor<'_, '_>) -> Result<Selector>;
}

pub trait ToCommitSelector {
    /// Calling `to_commit_selector` on
    /// an object asserts that the reciever has a selectable pick step in the
    /// graph.
    fn to_commit_selector(&self, editor: &Editor<'_, '_>) -> Result<Selector>;
}

pub trait ToReferenceSelector {
    /// Calling `to_reference_selector` on
    /// an object asserts that the reciever has a selectable reference step in
    /// the graph.
    fn to_reference_selector(&self, editor: &Editor<'_, '_>) -> Result<Selector>;
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
    fn to_commit_selector(&self, editor: &Editor<'_, '_>) -> Result<Selector> {
        let selector = editor.history.normalize_selector(*self)?;
        let step = &editor.graph[selector.id];
        if !matches!(step, Step::Pick(_)) {
            bail!("Expected selector for {step:?} to refer to a commit");
        }

        Ok(selector)
    }
}

impl ToReferenceSelector for Selector {
    fn to_reference_selector(&self, editor: &Editor<'_, '_>) -> Result<Selector> {
        let selector = editor.history.normalize_selector(*self)?;
        let step = &editor.graph[selector.id];
        if !matches!(step, Step::Reference { .. }) {
            bail!("Expected selector for {step:?} to refer to a reference");
        }

        Ok(selector)
    }
}

impl ToSelector for Selector {
    fn to_selector(&self, _: &Editor<'_, '_>) -> Result<Selector> {
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
    /// A visible linked worktree whose `HEAD` should follow this edit.
    Worktree {
        /// The stable worktree name under `$GIT_COMMON_DIR/worktrees/`.
        worktree_name: gix::bstr::BString,
        /// The worktree's `HEAD` selector: a reference when attached, or a commit when detached.
        selector: Selector,
        /// The symbolic referent at editor creation, or `None` for a detached `HEAD`.
        ref_name: Option<gix::refs::FullName>,
        /// The peeled `HEAD` at editor creation, used to reject stale worktree state.
        initial_head: gix::ObjectId,
        /// Like [`Checkout::Head`]'s `merge_base_override`, but computed against this
        /// worktree's own `HEAD^{tree}`, so changes consumed *from this worktree*
        /// cancel out during its checkout.
        merge_base_override: Option<gix::ObjectId>,
    },
}

#[derive(Debug)]
pub struct Editor<'ws, 'db> {
    graph: StepGraph,
    /// Used to track any references that might need
    /// deleted.
    initial_references: Vec<gix::refs::FullName>,
    /// Worktrees that we might need to perform `safe_checkout` on.
    checkouts: Vec<Checkout>,
    /// The in-memory repository that the rebase engine works with.
    repo: gix::Repository,
    history: RevisionHistory,
    workspace: &'ws mut but_graph::Workspace,
    db: &'db mut but_db::DbHandle,
}

/// Represents a successful rebase, and any valid, but potentially conflicting scenarios it had.
#[derive(Debug)]
pub struct SuccessfulRebase<'ws, 'db> {
    pub(crate) repo: gix::Repository,
    pub(crate) initial_references: Vec<gix::refs::FullName>,
    pub(crate) ref_edits: Vec<RefEdit>,
    pub(crate) graph: StepGraph,
    pub(crate) checkouts: Vec<Checkout>,
    pub history: RevisionHistory,
    workspace: &'ws mut but_graph::Workspace,
    /// The database handle inherited from the [`Editor`], so [`Self::into_editor`]
    /// can hand it back.
    db: &'db mut but_db::DbHandle,
}

impl<'ws, 'db> SuccessfulRebase<'ws, 'db> {
    /// Returns the in-memory repository that backs this rebase preview.
    ///
    /// This repository may contain objects that have not been persisted yet,
    /// which makes it suitable for dry-run inspection of [`Self::overlayed_graph`].
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Returns the preview repository and the database the editor was created with.
    pub fn repo_and_db_mut(&mut self) -> (&gix::Repository, &mut but_db::DbHandle) {
        (&self.repo, self.db)
    }

    pub fn db(&self) -> &but_db::DbHandle {
        self.db
    }

    /// The project metadata the workspace is projected from, both by
    /// [`Self::overlayed_graph`] and when materializing. A rewrite that moved the
    /// target records the new tip here so the projection sits on it.
    pub fn project_meta_mut(&mut self) -> &mut but_core::ref_metadata::ProjectMeta {
        &mut self.workspace.graph.project_meta
    }

    fn checkout_target(
        &self,
        selector: Selector,
    ) -> Result<Option<(gix::ObjectId, Option<gix::refs::FullName>)>> {
        let selector = self.history.normalize_selector(selector)?;
        self.checkout_target_inner(selector.id)
    }

    fn checkout_target_inner(
        &self,
        idx: StepGraphIndex,
    ) -> Result<Option<(gix::ObjectId, Option<gix::refs::FullName>)>> {
        Ok(match &self.graph[idx] {
            Step::None => {
                collect_ordered_parents(&self.graph, idx, OrderedParentKind::CommitOrReference)
                    .first()
                    .map(|child_idx| self.checkout_target_inner(*child_idx))
                    .transpose()?
                    .flatten()
            }
            Step::Pick(Pick { id, .. }) => Some((*id, None)),
            Step::Reference { refname, .. } => {
                let parent =
                    collect_ordered_parents(&self.graph, idx, OrderedParentKind::CommitOnly)
                        .into_iter()
                        .next()
                        .context("No first parent to reference")?;
                let Step::Pick(Pick { id, .. }) = self.graph[parent] else {
                    bail!("collect_ordered_parents should always return a commit pick");
                };
                Some((id, Some(refname.clone())))
            }
        })
    }

    /// Makes materializing leave the files of every checkout untouched while its `HEAD`
    /// and index still follow the rewrite, so what the rewrite took out of a checked out
    /// commit surfaces as uncommitted changes.
    pub fn keep_worktrees_unchanged(&mut self) -> Result<()> {
        let trees = self
            .padded_checkouts()?
            .into_iter()
            .map(|PaddedCheckout { tip, .. }| {
                tip.map(|tip| {
                    anyhow::Ok(tip.target().attach(&self.repo).object()?.peel_to_tree()?.id)
                })
                .transpose()
            })
            .collect::<Result<Vec<_>>>()?;
        for (checkout, tree) in self.checkouts.iter_mut().zip(trees) {
            let (Checkout::Head {
                merge_base_override,
                ..
            }
            | Checkout::Worktree {
                merge_base_override,
                ..
            }) = checkout;
            *merge_base_override = tree;
        }
        Ok(())
    }

    fn padded_checkouts(&self) -> Result<Vec<PaddedCheckout<'_>>> {
        self.checkouts
            .iter()
            .map(|checkout| {
                let (Checkout::Head { selector, .. } | Checkout::Worktree { selector, .. }) =
                    checkout;
                let tip = self
                    .checkout_target(*selector)?
                    .map(|(commit, ref_name)| {
                        let conflicted = but_core::Commit::from_id(commit.attach(&self.repo))?;
                        let padding = conflicted
                            .is_conflicted()
                            .then(|| padding::create(&conflicted))
                            .transpose()
                            .context(but_error::Code::ConflictedCommitCheckout)?;
                        anyhow::Ok(PaddedTip {
                            commit,
                            padding,
                            ref_name,
                        })
                    })
                    .transpose()?;
                Ok(PaddedCheckout { checkout, tip })
            })
            .collect()
    }

    fn padded_ref_edits(&self, checkouts: &[PaddedCheckout<'_>]) -> Result<Vec<RefEdit>> {
        let mut edits = self.ref_edits.clone();
        let padded_refs = checkouts.iter().filter_map(|checkout| match &checkout.tip {
            Some(PaddedTip {
                commit,
                padding: Some(padding),
                ref_name: Some(ref_name),
            }) => Some((*commit, *padding, ref_name)),
            _ => None,
        });
        for (commit, padding, ref_name) in padded_refs {
            match edits.iter_mut().find(|edit| &edit.name == ref_name) {
                Some(RefEdit {
                    change: Change::Update { new, .. },
                    ..
                }) => *new = Target::Object(padding),
                Some(RefEdit {
                    change: Change::Delete { .. },
                    ..
                }) => bail!("BUG: the checked out reference {ref_name} is deleted"),
                None => edits.push(RefEdit::update(
                    ref_name.clone(),
                    padding,
                    PreviousValue::MustExistAndMatch(Target::Object(commit)),
                    "",
                )),
            }
        }
        edits.retain(|edit| {
            !matches!(
                &edit.change,
                Change::Update {
                    expected: PreviousValue::MustExistAndMatch(previous),
                    new,
                    ..
                } if previous == new
            )
        });
        Ok(edits)
    }

    fn worktree_tips_after_rebase(&self) -> Result<Vec<but_graph::init::WorktreeTip>> {
        self.checkouts
            .iter()
            .filter_map(|checkout| match checkout {
                Checkout::Worktree {
                    worktree_name,
                    selector,
                    ..
                } => Some((worktree_name, selector)),
                Checkout::Head { .. } => None,
            })
            .map(|(worktree_name, selector)| {
                let (id, ref_name) = self.checkout_target(*selector)?.with_context(|| {
                    format!("Visible worktree {worktree_name} HEAD was removed")
                })?;
                Ok(but_graph::init::WorktreeTip {
                    name: worktree_name.clone(),
                    ref_name,
                    id,
                })
            })
            .collect()
    }

    /// Return the commit targeted by `ref_name` in the post-rebase step graph.
    pub fn reference_target(&self, ref_name: &gix::refs::FullNameRef) -> Result<gix::ObjectId> {
        let reference = self
            .graph
            .node_indices()
            .find(|node| {
                matches!(
                    &self.graph[*node],
                    Step::Reference { refname, .. } if refname == ref_name
                )
            })
            .with_context(|| format!("Could not find reference '{ref_name}' in rebase result"))?;
        let parent = collect_ordered_parents(&self.graph, reference, OrderedParentKind::CommitOnly)
            .into_iter()
            .next()
            .context("Reference has no target commit in rebase result")?;
        match self.graph[parent] {
            Step::Pick(Pick { id, .. }) => Ok(id),
            _ => bail!("Reference target in rebase result is not a commit"),
        }
    }

    /// Returns a preview of what the but-graph will look like after
    /// materialization.
    ///
    /// Any objects referenced in the resulting graph must be accessed via the
    /// in-memory repository owned by this [`SuccessfulRebase`] (`self.repo`),
    /// since they might exist only in memory.
    pub fn overlayed_graph(&self) -> Result<but_graph::Graph> {
        self.overlayed_graph_with_workspace_overrides(None, None)
    }

    /// Return the post-rebase graph with optional ad-hoc workspace projection overrides.
    ///
    /// This is useful for dry-run operations whose graph rewrite is accompanied by metadata or
    /// checkout changes that are deliberately not persisted. The override entrypoint must name the
    /// commit and local reference that would be checked out, while `branch_stack_order` supplies the
    /// tip-to-base order that would be written to ref metadata.
    pub fn overlayed_graph_with_workspace_overrides(
        &self,
        entrypoint: Option<(gix::ObjectId, gix::refs::FullName)>,
        branch_stack_order: Option<&[gix::refs::FullName]>,
    ) -> Result<but_graph::Graph> {
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
                Checkout::Head { selector, .. } => self.checkout_target(*selector).ok().flatten(),
                Checkout::Worktree { .. } => None,
            })
            .next()
        else {
            bail!("BUG: Tried to construct rebase engine graph overlay with no entrypoints");
        };

        let (entrypoint_id, entrypoint_refname) = entrypoint
            .map_or((entrypoint_id, entrypoint_refname), |(id, ref_name)| {
                (id, Some(ref_name))
            });
        let branch_stacks = self.branch_stacks()?;
        let workspace_metadata = branch_stacks.workspace_metadata(&self.db.meta())?;
        let branch_metadata = workspace_metadata
            .iter()
            .flat_map(|workspace| &workspace.stacks)
            .flat_map(|stack| &stack.branches)
            .map(|branch| {
                let metadata = self.db.meta().branch(branch.ref_name.as_ref())?;
                Ok((branch.ref_name.clone(), (*metadata).clone()))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut overlay = Overlay::default()
            .with_references(updated_refs)
            .with_dropped_references(dropped_refs)
            .with_entrypoint(entrypoint_id, entrypoint_refname)
            .with_branch_metadata_override(branch_metadata)
            .with_workspace_metadata_override(
                workspace_metadata
                    .map(|workspace| (workspace.as_ref().to_owned(), (*workspace).clone())),
            );
        if let Some(branch_stack_order) = branch_stack_order {
            overlay = overlay.with_branch_stack_order_override(branch_stack_order.iter().cloned());
        }
        for stack in branch_stacks.ad_hoc {
            overlay = overlay.with_branch_stack_order_override(stack);
        }
        let mut graph = self.workspace.graph.clone();
        graph.worktree_tips = self.worktree_tips_after_rebase()?;
        graph.redo_traversal_with_overlay(&self.repo, &self.db.meta(), overlay)
    }

    /// Resolve `selector` to the identifiers of its commit pick including the change id.
    pub fn lookup_commit(&self, selector: Selector) -> Result<CommitIdentifiers> {
        let id = self.lookup_pick(selector)?;
        let commit = self.repo.find_commit(id)?;
        let commit = commit.decode()?;

        let change_id =
            but_core::commit::Headers::try_from_commit_headers(|| commit.extra_headers())
                .unwrap_or_default()
                .ensure_change_id(id)
                .change_id
                .expect("change ID is ensured");

        Ok(CommitIdentifiers { id, change_id })
    }

    /// If the successful rebase is materialized, will any references be updated
    ///
    /// Materialization will be a no-op
    pub fn references_updated(&self) -> Result<bool> {
        let checkouts = self.padded_checkouts()?;
        let ref_edits = self.padded_ref_edits(&checkouts)?;
        Ok(self.updates_references(&checkouts, &ref_edits))
    }

    fn updates_references(&self, checkouts: &[PaddedCheckout<'_>], ref_edits: &[RefEdit]) -> bool {
        !ref_edits.is_empty()
            || checkouts.iter().any(|PaddedCheckout { checkout, tip }| {
                let target = tip.as_ref().map(|tip| (tip.target(), tip.ref_name.clone()));
                match checkout {
                    Checkout::Head { .. } => {
                        let head = self
                            .repo()
                            .head_id()
                            .ok()
                            .map(|id| (id.detach(), self.repo().head_name().ok().flatten()));
                        target != head
                    }
                    Checkout::Worktree {
                        ref_name,
                        initial_head,
                        ..
                    } => target != Some((*initial_head, ref_name.clone())),
                }
            })
    }
}

struct PaddedCheckout<'a> {
    checkout: &'a Checkout,
    tip: Option<PaddedTip>,
}

struct PaddedTip {
    commit: gix::ObjectId,
    padding: Option<gix::ObjectId>,
    ref_name: Option<gix::refs::FullName>,
}

impl PaddedTip {
    fn target(&self) -> gix::ObjectId {
        self.padding.unwrap_or(self.commit)
    }
}

#[derive(Debug)]
pub struct MaterializeOutcome<'ws, 'db> {
    pub(crate) graph: StepGraph,
    pub history: RevisionHistory,
    pub workspace: &'ws mut but_graph::Workspace,
    pub db: &'db mut but_db::DbHandle,
    /// The checkouts whose uncommitted changes now carry conflict markers.
    pub conflicted_checkouts: Vec<ConflictedCheckout>,
}

/// A checkout whose uncommitted changes conflicted with the commit it moved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictedCheckout {
    /// The editor's own worktree.
    Head,
    /// The linked worktree `name`, checked out at `path`.
    Worktree {
        /// The directory name under `$GIT_COMMON_DIR/worktrees/`.
        name: gix::bstr::BString,
        path: std::path::PathBuf,
    },
}

pub trait LookupStep {
    fn lookup_step(&self, selector: Selector) -> Result<Step>;

    fn lookup_pick(&self, selector: Selector) -> Result<gix::ObjectId> {
        match self.lookup_step(selector)? {
            Step::Pick(Pick { id, .. }) => Ok(id),
            _ => bail!("Expected selector to point to a pick"),
        }
    }

    fn lookup_reference(&self, selector: Selector) -> Result<gix::refs::FullName> {
        match self.lookup_step(selector)? {
            Step::Reference { refname, .. } => Ok(refname),
            _ => bail!("Expected selector to point to a reference"),
        }
    }
}

impl LookupStep for Editor<'_, '_> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

impl LookupStep for SuccessfulRebase<'_, '_> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

impl LookupStep for MaterializeOutcome<'_, '_> {
    fn lookup_step(&self, selector: Selector) -> Result<Step> {
        lookup_step(&self.graph, &self.history, selector)
    }
}

fn lookup_step(graph: &StepGraph, history: &RevisionHistory, selector: Selector) -> Result<Step> {
    let normalized = history.normalize_selector(selector)?;
    Ok(graph[normalized.id].clone())
}

/// How commits were rewritten, as pairs of an original commit and a commit that succeeds it.
///
/// An original can have several successors, and several originals can share one. A pair is
/// held once however often it is given.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CommitMappings(BTreeSet<(gix::ObjectId, gix::ObjectId)>);

impl FromIterator<(gix::ObjectId, gix::ObjectId)> for CommitMappings {
    fn from_iter<T: IntoIterator<Item = (gix::ObjectId, gix::ObjectId)>>(pairs: T) -> Self {
        Self(pairs.into_iter().collect())
    }
}

impl std::fmt::Debug for CommitMappings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl CommitMappings {
    /// Every pair of an original and one of its successors, ordered by original.
    pub fn iter(&self) -> impl Iterator<Item = (gix::ObjectId, gix::ObjectId)> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The commits that succeed `original`, none if it wasn't rewritten.
    pub fn successors(&self, original: gix::ObjectId) -> impl Iterator<Item = gix::ObjectId> + '_ {
        self.0
            .range((original, gix::ObjectId::null(original.kind()))..)
            .take_while(move |(from, _)| *from == original)
            .map(|(_, successor)| *successor)
    }

    /// The commit `original` is now: itself if it wasn't rewritten, else its successor.
    ///
    /// Fails if it has several successors, as none of them is `original` more than another.
    pub fn current(&self, original: gix::ObjectId) -> Result<gix::ObjectId> {
        let mut successors = self.successors(original);
        let current = successors.next().unwrap_or(original);
        if successors.next().is_some() {
            bail!("Commit {original} was rewritten into several commits");
        }
        Ok(current)
    }

    /// Each original that has exactly one successor, with that successor.
    pub fn sole_successors(&self) -> BTreeMap<gix::ObjectId, gix::ObjectId> {
        let mut sole = BTreeMap::new();
        let mut shared = BTreeSet::new();
        for (original, successor) in self.iter() {
            if sole.insert(original, successor).is_some() {
                shared.insert(original);
            }
        }
        sole.retain(|original, _| !shared.contains(original));
        sole
    }
}

#[derive(Debug, Clone, Default)]
pub struct RevisionHistory {
    mappings: Vec<HashMap<StepGraphIndex, StepGraphIndex>>,
    /// A mapping from any commits that were in the original mapping to a
    /// rewritten version.
    ///
    /// Unintuatively, the values are the original values, and the keys are the
    /// _new_ values that they have been mapped to.
    commit_mappings: BTreeMap<gix::ObjectId, gix::ObjectId>,
    pub conflicted_commits: Vec<(gix::ObjectId, ChangeId)>,
}

impl<'ws, 'db> Editor<'ws, 'db> {
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

    pub fn commit_mappings(&self) -> CommitMappings {
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
mod commit_mappings {
    use super::CommitMappings;

    fn id(hex: char) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.to_string().repeat(40).as_bytes()).expect("valid hex")
    }

    #[test]
    fn many_to_many() -> anyhow::Result<()> {
        let (a, b, c, d) = (id('a'), id('b'), id('c'), id('d'));
        let mappings: CommitMappings = [(a, c), (a, c), (b, c), (b, d)].into_iter().collect();

        // The repeated `a -> c` is held once, and `c` is shared by both originals.
        snapbox::assert_data_eq!(
            format!("{mappings:#?}"),
            snapbox::str![[r#"
{
    Sha1(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa): Sha1(cccccccccccccccccccccccccccccccccccccccc),
    Sha1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb): Sha1(cccccccccccccccccccccccccccccccccccccccc),
    Sha1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb): Sha1(dddddddddddddddddddddddddddddddddddddddd),
}
"#]]
        );
        assert_eq!(
            mappings.current(a)?,
            c,
            "a single successor is what `a` is now"
        );
        assert_eq!(
            mappings.current(d)?,
            d,
            "a commit that wasn't rewritten is itself"
        );
        assert!(
            mappings.current(b).is_err(),
            "neither of two successors is `b` more than the other"
        );
        assert_eq!(
            mappings.sole_successors(),
            [(a, c)].into_iter().collect(),
            "only `a` has exactly one successor"
        );
        Ok(())
    }
}

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
