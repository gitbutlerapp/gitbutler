#![deny(missing_docs)]
//! One graph based engine to rule them all,
//! one vector based to find them,
//! one mess of git2 code to bring them all,
//! and in the darkness bind them.
//!
//! ---
//!
//! A graph-based rebase engine. The workspace is loaded into an `Editor`: a graph of
//! commits, with each reference (branch) stored as a position on its commit.
//! Callers mutate the graph (insert/move/remove commits, create/move references), then
//! `Editor::rebase` replays it — cherry-picking every mutable commit onto its new parents
//! and producing the reference updates to write. A [`CommitSpec`](crate::graph_rebase::CommitSpec) is the currency at the
//! boundary: the description of a commit you hand in, or read back out.
//!
//! References are positions on commits, not entries in the graph — see the `positions`
//! module for the model. That model makes the replay contract one sentence: ids are
//! rewritten in place (entry identity, parent arrays and positions never move), so no
//! mutation maintains a ref target by hand — after the rewrite, every ref's update is
//! derived from its position in one loop. "Rewrote the graph but forgot to move the
//! branch" is not a bug this engine can have; there is no such step to forget.
//!
//! # Vanilla git and the extension
//!
//! The editor is a plain-git rebase engine with GitButler's workspace model layered
//! beside it, and the code keeps the two distinguishable everywhere:
//!
//! - `commits` — the vanilla half: the commit graph mounted for editing. It imports
//!   nothing from the ref side, so commit surgery cannot touch references.
//! - `positions` (reads and checks) and `ref_ops` (writes) — the extension: group order,
//!   carries, empty-lane slots; the facts git itself cannot represent. The split is the
//!   signature: every `positions` function takes `&EditorStore`, every `ref_ops`
//!   function takes `&mut`.
//! - the verbs (`mutate`) — deliberately both worlds: a mutation states its commit-side
//!   and ref-side consequences in one place, because the right ref-side consequence
//!   depends on what the mutation means. There is no fixup pass anywhere. Their lines
//!   classify themselves: `store.commits.…` is vanilla surgery, `positions::`/`ref_ops::`
//!   is the extension, and a bare `store.…` method does not self-classify — its own doc
//!   says what it serves or spans.
//!
//! Vanilla behaviors are the extension's degenerate cases: a plain ref is a singleton
//! group with carry `All` and nothing stacked above it, and a ref following a rebase is
//! implemented by doing nothing — positions stand still while ids rewrite underneath.
//! The seam between the worlds is `RefState::on`: the one fact git can say (name to
//! commit), written by the vanilla primitive `set_on` and annotated by the extension's
//! table, with the agreement checked by a debug assertion. A guided reading of this whole section,
//! built from code excerpts: `editor-worlds.md` at the repository root. The vanilla
//! reads of it — `resolve_to_commit`, `positioned_on` — live on the store with
//! the other methods that span both halves, so a plain-git question never consults the
//! extension's table at all.

mod commits;
mod creation;
mod positions;
mod ref_ops;
mod store;
pub(crate) mod util;

pub mod anchor;
pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod merge_commit_changes;
pub mod mutate;
pub mod ordering;
pub mod rebase;
/// Utilities for testing
pub mod testing;
pub mod traverse;
pub mod workspace;

pub use commits::CommitIndex;
pub use creation::EditorStoreOptions;
pub use store::{EditorIndex, RefIndex};
pub use workspace::{GraphWorkspace, Subgraph};

pub(crate) use store::EditorStore;

use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use but_core::commit::CommitIdentifiers;
use but_core::{RefMetadata, commit::SignCommit, ref_metadata::ProjectMeta};
use but_graph::walk::Overlay;
use gix::refs::transaction::RefEdit;

use crate::graph_rebase::cherry_pick::{PickMode, TreeMergeMode};

/// Used to manipulate a set of commits. Every mutation takes `&mut self` and the rebase
/// consumes `self`, so a shared `&Editor` is inherently a frozen view — which is exactly
/// how [`RebasedEditor`] exposes it.
#[derive(Debug)]
pub struct Editor<'meta, M: RefMetadata> {
    /// The editor's store — the state every mutation rewrites; see [`EditorStore`].
    pub(crate) store: EditorStore,
    /// Worktrees that we might need to perform `safe_checkout` on.
    pub(crate) checkouts: Vec<Checkout>,
    /// The in-memory repository that the rebase engine works with.
    pub(crate) repo: gix::Repository,
    /// Provides data about how the editor instance was transformed.
    pub(crate) history: RevisionHistory,
    /// The workspace target configuration the editor was created with.
    pub(crate) project_meta: ProjectMeta,
    /// A reference to the metadata that the editor was created for.
    pub(crate) meta: &'meta mut M,
}

/// Addressing and reads shared by every operation: anchor resolution and the typed
/// lookups. The mutation verbs live in [`mutate`], traversal in [`traverse`].
impl<M: RefMetadata> Editor<'_, M> {
    /// Resolve `anchor` to the entry it addresses: a held index passes through, a commit id
    /// or reference name is looked up in the graph — and fails here when absent.
    pub fn resolve_anchor(&self, anchor: impl Into<anchor::Anchor>) -> Result<EditorIndex> {
        Ok(match anchor.into() {
            anchor::Anchor::Commit(id) => self.select_commit(id)?.into(),
            anchor::Anchor::Reference(name) => self.select_reference(name.as_ref())?.into(),
            anchor::Anchor::Held(entry) => entry,
        })
    }

    /// The commit id the commit at `commit` holds; errors when it was removed.
    pub fn id_of(&self, commit: CommitIndex) -> Result<gix::ObjectId> {
        Ok(self.spec_of(commit)?.id)
    }

    /// The full commit at `commit` — id and per-commit options; errors when removed.
    pub fn spec_of(&self, commit: CommitIndex) -> Result<CommitSpec> {
        self.store
            .commits
            .commit_spec(commit)
            .context("The addressed commit was removed")
    }

    /// The name of the reference at `reference`; errors when it was deleted.
    pub fn name_of(&self, reference: RefIndex) -> Result<gix::refs::FullName> {
        match self.store.reference(reference.into()) {
            Some((refname, _)) => Ok(refname.clone()),
            None => bail!("The addressed reference was deleted"),
        }
    }

    /// Whether the entry at `index` was removed — matching the [`EditorIndex`] arms
    /// answers which kind statically; this answers the one dynamic fact, liveness.
    pub fn is_removed(&self, index: impl Into<EditorIndex>) -> bool {
        match index.into() {
            index @ EditorIndex::Commit(_) => self.store.commit_id(index).is_none(),
            EditorIndex::Ref(i) => !self.store.is_reference(i),
        }
    }

    /// Every commit the edits so far have rewritten, old id to new id.
    pub fn commit_mappings(&self) -> BTreeMap<gix::ObjectId, gix::ObjectId> {
        self.history.commit_mappings()
    }
}

/// The editor after its rebase: the replayed graph, and the reference edits to write.
/// Conflicting commits may be among the results — a rebase that ran is not necessarily one
/// you want to keep.
///
/// Holds the editor frozen by ownership: the field is private and only ever lent out
/// as `&Editor` (via `Deref`), through which no `&mut self` mutation is callable —
/// the borrow system carries the cannot-mutate-a-finished-rebase guarantee.
#[derive(Debug)]
pub struct RebasedEditor<'meta, M: RefMetadata> {
    pub(crate) editor: Editor<'meta, M>,
    /// Any reference edits that need to be committed as a result of the history
    /// rewrite
    pub(crate) ref_edits: Vec<RefEdit>,
}

impl<'meta, M: RefMetadata> std::ops::Deref for RebasedEditor<'meta, M> {
    type Target = Editor<'meta, M>;
    fn deref(&self) -> &Self::Target {
        &self.editor
    }
}

impl<'meta, M: RefMetadata> RebasedEditor<'meta, M> {
    /// Resolve `commit` to the identifiers of its commit, including the change id.
    pub fn identifiers_of(&self, commit: CommitIndex) -> Result<CommitIdentifiers> {
        let id = self.id_of(commit)?;
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

    /// Returns the preview repository together with mutable access to the
    /// ref-metadata the editor was created with.
    ///
    /// Use this to build post-rebase projections that need both, like a
    /// workspace preview computed from [`Self::overlay`].
    pub fn repo_and_meta_mut(&mut self) -> (&gix::Repository, &mut M) {
        (&self.editor.repo, self.editor.meta)
    }

    /// Returns the ref-metadata the editor was created with.
    pub fn meta(&self) -> &M {
        self.meta
    }

    /// The mutated commit graph this rebase will materialize — already the next workspace
    /// state, since materializing only persists objects and applies ref edits. Project it
    /// (with [`Editor::repo`] and [`Self::overlay`]) to preview without a rewalk.
    pub fn commit_graph(&self) -> &but_graph::CommitGraph {
        self.store.commits.graph()
    }

    /// Return the commit targeted by `ref_name` in the post-rebase graph.
    pub fn reference_target(&self, ref_name: &gix::refs::FullNameRef) -> Result<gix::ObjectId> {
        let (ref_idx, ..) = self
            .store
            .references()
            .find(|(_, refname, _)| refname.as_ref() == ref_name)
            .with_context(|| format!("Could not find reference '{ref_name}' in rebase result"))?;
        self.store
            .resolve_to_commit(ref_idx)
            .and_then(|commit| self.store.commit_id(commit))
            .context("Reference has no target commit in rebase result")
    }

    /// Where `entry` ended up after the rewrite: the commit, and the reference naming
    /// it when the entry is a reference rather than a commit.
    ///
    /// `None` when the entry resolves to nothing, which for a checkout means what it
    /// followed was removed by the edit.
    pub(crate) fn checkout_target(
        &self,
        entry: EditorIndex,
    ) -> Result<Option<(gix::ObjectId, Option<gix::refs::FullName>)>> {
        Ok(match entry {
            EditorIndex::Commit(_) => self.store.commit_id(entry).map(|id| (id, None)),
            EditorIndex::Ref(_) => match self.store.reference(entry) {
                None => None,
                Some((refname, _)) => {
                    let refname = refname.clone();
                    let commit = self
                        .store
                        .resolve_to_commit(entry)
                        .context("No commit to reference")?;
                    let id = self
                        .store
                        .commit_id(commit)
                        .context("resolve_to_commit always resolves to a commit")?;
                    Some((id, Some(refname)))
                }
            },
        })
    }

    /// The overlay describing this rebase's outcome: updated/dropped refs plus the requested
    /// checkout as the entrypoint.
    ///
    /// Feed it to a graph or workspace re-derivation (with [`Editor::repo`], since rewritten objects may
    /// exist only in memory) to preview the post-rebase state without materializing.
    pub fn overlay(&self) -> Result<Overlay> {
        self.overlay_with(None, None)
    }

    /// Like [`Self::overlay`], with ad-hoc workspace projection overrides.
    ///
    /// For dry-run operations whose graph rewrite is accompanied by metadata or checkout
    /// changes that are deliberately not persisted: the override `entrypoint` names the
    /// commit and local reference that would be checked out, while `branch_stack_order`
    /// supplies the tip-to-base order that would be written to ref metadata.
    pub fn overlay_with(
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
                Checkout::Head { entry, .. } => self.checkout_target(*entry).ok().flatten(),
                // A linked worktree has its own `HEAD`; it never names the entrypoint.
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

/// Represents a commit to be cherry-picked in a rebase operation.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitSpec {
    /// The ID of the commit getting cherry-picked
    pub id: gix::ObjectId,
    /// If we are dealing with a sub-graph with an incomplete history, we
    /// need to represent the bottom most commits in a way that we preserve
    /// their parents.
    ///
    /// If this is Some, the commit will not be cherry-picked onto the parents the
    /// graph implies but instead on to the parents listed here.
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    /// Controls under what circumstances the commit is cherry-picked.
    pub pick_mode: PickMode,
    /// Controls whether the resulting commit is signed.
    ///
    /// Note that signing a parent commit only causes descendants to be signed if those descendants
    /// are also cherry-picked with a `sign_commit` value that enables signing (e.g. [`SignCommit::Yes`]
    /// or [`SignCommit::IfSignCommitsEnabled`] with config enabled).
    pub sign_commit: SignCommit,
    /// Exclude the commit from being included in the
    /// [`Editor::commit_mappings()`]. This is helpful if we are
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
    /// When `false`, the rebase copies the commit verbatim instead of
    /// cherry-picking it, preserving its id.
    pub mutable: bool,
}

impl CommitSpec {
    /// Creates a commit with the expected defaults
    pub fn new(id: gix::ObjectId) -> Self {
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

    /// Creates a commit with the expected defaults, but is excluded from being
    /// included from the [`Editor::commit_mappings()`] output. This is
    /// often preferable if you are doing something like an
    /// `insert_blank_commit` operation.
    pub fn untracked(id: gix::ObjectId) -> Self {
        let mut spec = Self::new(id);
        spec.exclude_from_tracking = true;
        spec
    }

    /// Creates a commit with the defaults set for a workspace commit
    pub fn workspace(id: gix::ObjectId) -> Self {
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

/// Represents places where `safe_checkout` should be called from
#[derive(Debug, Clone)]
pub(crate) enum Checkout {
    /// The HEAD of the `repo` the editor was created for.
    Head {
        entry: EditorIndex,
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
        /// The worktree's `HEAD` entry: a reference when attached, or a commit when detached.
        entry: EditorIndex,
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

/// How commit ids moved as the editor transformed the graph.
#[derive(Debug, Clone, Default)]
pub(crate) struct RevisionHistory {
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
    pub(crate) fn commit_mappings(&self) -> BTreeMap<gix::ObjectId, gix::ObjectId> {
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
        CommitSpec,
        cherry_pick::{PickMode, TreeMergeMode},
    };

    #[test]
    fn workspace_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            CommitSpec::workspace(object_id),
            CommitSpec {
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
            CommitSpec::new(object_id),
            CommitSpec {
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
