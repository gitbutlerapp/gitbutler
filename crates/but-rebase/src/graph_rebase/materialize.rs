//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::{
    bstr::{BString, ByteSlice as _},
    refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    },
};

use crate::graph_rebase::{Checkout, RebasedEditor};

pub(super) struct LinkedCheckoutSpec {
    pub(super) name: BString,
    pub(super) initial_head: gix::ObjectId,
    pub(super) ref_name: Option<gix::refs::FullName>,
    pub(super) target: gix::ObjectId,
    pub(super) merge_base_override: Option<gix::ObjectId>,
}

struct HeadCheckout {
    target: gix::ObjectId,
    ref_name: Option<gix::refs::FullName>,
    merge_base_override: Option<gix::ObjectId>,
}

struct LinkedCheckoutRepo {
    repo: gix::Repository,
    target: gix::ObjectId,
    merge_base_override: Option<gix::ObjectId>,
}

/// Ref edits moving the `HEAD` of every detached linked worktree in `specs` to where
/// the rewrite put it.
///
/// Attached worktrees need nothing here - their symbolic `HEAD` follows the branch
/// edit that is already part of the transaction.
///
/// `worktrees/<name>/HEAD` addresses another worktree's `HEAD` from this repository,
/// so this rides along in the same transaction as the branch updates instead of
/// needing the worktree's own repository handle. Making `initial_head` the expected
/// value lets the transaction reject a worktree that moved under us, rather than
/// checking for it separately and racing.
fn detached_worktree_head_edits(specs: &[LinkedCheckoutSpec]) -> Result<Vec<RefEdit>> {
    specs
        .iter()
        .filter(|spec| spec.ref_name.is_none())
        .map(|spec| {
            let name: gix::refs::FullName = format!("worktrees/{}/HEAD", spec.name)
                .try_into()
                .with_context(|| {
                    format!(
                        "Worktree {} has a name that cannot address its HEAD",
                        spec.name
                    )
                })?;
            Ok(RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: gix::reference::log::message("rebase", "HEAD".into(), 1),
                    },
                    expected: PreviousValue::MustExistAndMatch(Target::Object(spec.initial_head)),
                    new: Target::Object(spec.target),
                },
                name,
                deref: false,
            })
        })
        .collect()
}

fn open_linked_checkout_repos(
    repo: &gix::Repository,
    specs: Vec<LinkedCheckoutSpec>,
) -> Result<Vec<LinkedCheckoutRepo>> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let proxies = repo.worktrees()?;
    specs
        .into_iter()
        .map(|spec| {
            let proxy = proxies
                .iter()
                .find(|proxy| proxy.id() == spec.name.as_bstr())
                .with_context(|| format!("Visible worktree {} no longer exists", spec.name))?;
            let worktree_repo = proxy.clone().into_repo()?;
            let actual_ref = worktree_repo.head_name()?;
            let actual_head = worktree_repo.head_id()?.detach();
            if actual_ref.as_ref() != spec.ref_name.as_ref() || actual_head != spec.initial_head {
                bail!(
                    "Visible worktree {} changed since the editor was created: \
                     expected {} at {}, got {} at {}",
                    spec.name,
                    spec.ref_name
                        .as_ref()
                        .map_or_else(|| "detached".into(), ToString::to_string),
                    spec.initial_head,
                    actual_ref
                        .as_ref()
                        .map_or_else(|| "detached".into(), ToString::to_string),
                    actual_head
                );
            }
            Ok(LinkedCheckoutRepo {
                repo: worktree_repo,
                target: spec.target,
                merge_base_override: spec.merge_base_override,
            })
        })
        .collect()
}

impl<'meta, M: RefMetadata> RebasedEditor<'meta, M> {
    /// The linked worktrees this edit has to move, with where the rewrite put each
    /// one, validated against the shape recorded at editor creation.
    pub(super) fn linked_checkout_specs(&self) -> Result<Vec<LinkedCheckoutSpec>> {
        let mut specs = Vec::new();
        for checkout in &self.checkouts {
            let Checkout::Worktree {
                worktree_name,
                entry,
                ref_name: expected_ref,
                initial_head,
                merge_base_override,
            } = checkout
            else {
                continue;
            };
            let (target, actual_ref) = self
                .checkout_target(*entry)?
                .with_context(|| format!("Visible worktree {worktree_name} HEAD was removed"))?;
            if actual_ref.as_ref() != expected_ref.as_ref() {
                bail!(
                    "Visible worktree {worktree_name} HEAD changed shape during the edit: \
                     expected {}, got {}",
                    expected_ref
                        .as_ref()
                        .map_or_else(|| "detached".into(), ToString::to_string),
                    actual_ref
                        .as_ref()
                        .map_or_else(|| "detached".into(), ToString::to_string)
                );
            }
            specs.push(LinkedCheckoutSpec {
                name: worktree_name.clone(),
                initial_head: *initial_head,
                ref_name: expected_ref.clone(),
                target,
                merge_base_override: *merge_base_override,
            });
        }
        Ok(specs)
    }

    /// The editor's own `HEAD` checkout, or `None` when `HEAD` wasn't on a ref
    /// at editor creation and thus has nothing to follow.
    fn head_checkout(&self) -> Result<Option<HeadCheckout>> {
        let Some((entry, merge_base_override)) =
            self.checkouts.iter().find_map(|checkout| match checkout {
                Checkout::Head {
                    entry,
                    merge_base_override,
                } => Some((*entry, *merge_base_override)),
                Checkout::Worktree { .. } => None,
            })
        else {
            return Ok(None);
        };
        let (target, ref_name) = self
            .checkout_target(entry)?
            .context("Checkout entry resolves to nothing")?;
        Ok(Some(HeadCheckout {
            target,
            ref_name,
            merge_base_override,
        }))
    }

    /// If the rebase is materialized, will any references be updated? If not,
    /// materialization is a no-op.
    pub fn references_updated(&self) -> Result<bool> {
        if !self.ref_edits.is_empty() {
            return Ok(true);
        }
        for checkout in &self.checkouts {
            match checkout {
                Checkout::Head { entry, .. } => {
                    let target = self.checkout_target(*entry)?.map(|t| t.0);
                    if target != self.repo().head_id().ok().map(|id| id.detach()) {
                        return Ok(true);
                    }
                }
                Checkout::Worktree {
                    entry,
                    initial_head,
                    ..
                } => {
                    let target = self.checkout_target(*entry)?.map(|t| t.0);
                    if target != Some(*initial_head) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    /// Materializes a history rewrite.
    ///
    /// See [`Self::materialize_with_outcome`] for a version that also reports whether the
    /// checkout of the editor's own worktree conflicted with uncommitted changes.
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    pub fn materialize(self) -> Result<(but_graph::CommitGraph, &'meta mut M)> {
        let outcome = self.materialize_with_outcome()?;
        Ok((outcome.commit_graph, outcome.meta))
    }

    /// Materializes a history rewrite, reporting on the checkout as well.
    ///
    /// A rewrite that [updates no reference](Self::references_updated) touches nothing.
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    pub fn materialize_with_outcome(mut self) -> Result<MaterializeOutcome<'meta, M>> {
        if !self.references_updated()? {
            return Ok(MaterializeOutcome {
                commit_graph: self.editor.store.commits.into_graph(),
                meta: self.editor.meta,
                checkout_conflict_occurred: false,
            });
        }

        let repo = self.repo.clone();
        if let Some(memory) = self.editor.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let specs = self.linked_checkout_specs()?;
        let detached_head_edits = detached_worktree_head_edits(&specs)?;
        let linked_repos = open_linked_checkout_repos(&repo, specs)?;
        for linked_repo in linked_repos {
            safe_checkout_from_head(
                linked_repo.target,
                &linked_repo.repo,
                Options {
                    skip_head_update: true,
                    merge_base_override: linked_repo.merge_base_override,
                    allow_conflicted_commit_checkout: false,
                    // Don't allow for linked worktrees.
                    allow_uncommitted_changes_to_conflict_with_new_head: false,
                },
            )?;
        }

        let head = self.head_checkout()?;
        let checkout_conflict_occurred = if let Some(head) = &head {
            let outcome = safe_checkout_from_head(
                head.target,
                &repo,
                Options {
                    skip_head_update: true,
                    merge_base_override: head.merge_base_override,
                    allow_conflicted_commit_checkout: true,
                    // Allow for our worktree.
                    allow_uncommitted_changes_to_conflict_with_new_head: true,
                },
            )?;
            outcome.conflict_occurred
        } else {
            false
        };

        let mut ref_edits = std::mem::take(&mut self.ref_edits);
        ref_edits.extend(detached_head_edits);
        if let Some(refname) = head.and_then(|head| head.ref_name)
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
        let (commit_graph, meta) = self.finish(ref_edits)?;
        Ok(MaterializeOutcome {
            commit_graph,
            meta,
            checkout_conflict_occurred,
        })
    }

    /// Materializes a rebase without checking out the editor's own worktree.
    ///
    /// For the vast majority of operations you want to use
    /// [`Self::materialize`]. This is intended to be used in niche cases like
    /// `uncommit`.
    ///
    /// Skipping the checkout means the uncommitted changes are not carried from
    /// the old head to the new one. If you drop a commit from history, this
    /// leaves its changes in your working directory; [`Self::materialize`]
    /// would remove them from disk.
    ///
    /// Linked worktrees aren't checked out either, but their `HEAD`s still follow the
    /// rewrite, so what they had checked out surfaces as uncommitted changes there -
    /// exactly like the editor's own worktree.
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    pub fn materialize_without_checkout(
        mut self,
    ) -> Result<(but_graph::CommitGraph, &'meta mut M)> {
        if let Some(memory) = self.editor.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let mut ref_edits = std::mem::take(&mut self.ref_edits);
        ref_edits.extend(detached_worktree_head_edits(
            &self.linked_checkout_specs()?,
        )?);
        self.finish(ref_edits)
    }

    /// Apply the reference edits and surrender the materialized commit graph — the next
    /// workspace state. Materialization adds no information (ids and mappings were
    /// computed at rebase time and are readable on [`RebasedEditor`] before this call);
    /// its products are the side effects, plus this graph for
    /// `Workspace::refresh_from_commit_graph`. The metadata handle rides along because
    /// consuming the editor is what releases the borrow it took at construction —
    /// callers that persist metadata after the refs land reclaim it here.
    fn finish(self, ref_edits: Vec<RefEdit>) -> Result<(but_graph::CommitGraph, &'meta mut M)> {
        let deleted: Vec<_> = ref_edits
            .iter()
            .filter(|edit| matches!(edit.change, Change::Delete { .. }))
            .map(|edit| edit.name.clone())
            .collect();
        self.repo.edit_references(ref_edits)?;

        // Metadata retirement is derived from the edits git received: a name git no
        // longer has keeps no per-branch metadata, so a later branch reusing the name
        // starts clean instead of inheriting a dead PR association. The workspace ref
        // is exempt — backends treat removing it as deleting the whole workspace record.
        for name in deleted {
            if !but_core::is_workspace_ref_name(name.as_ref()) {
                self.editor.meta.remove(name.as_ref())?;
            }
        }

        Ok((self.editor.store.commits.into_graph(), self.editor.meta))
    }
}

/// What [`RebasedEditor::materialize_with_outcome`] surrenders: the materialized commit graph
/// (the next workspace state), the metadata handle, and how the checkout went.
pub struct MaterializeOutcome<'meta, M: RefMetadata> {
    /// The mutated commit graph, for `Workspace::refresh_from_commit_graph`.
    pub commit_graph: but_graph::CommitGraph,
    /// The metadata handle the editor was created with.
    pub meta: &'meta mut M,
    /// True if the checkout of the editor's own worktree had to deal with uncommitted
    /// changes conflicting with the new head. Linked worktrees never allow that.
    pub checkout_conflict_occurred: bool,
}
