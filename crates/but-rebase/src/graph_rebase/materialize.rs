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

use crate::graph_rebase::{Checkout, MaterializeOutcome, SuccessfulRebase};

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

/// Options for [SuccessfulRebase::materialize].
#[derive(Default)]
pub struct MaterializeOptions {
    /// Materializes a rebase without checking out the editor's own worktree.
    ///
    /// Linked worktrees aren't checked out either, but their `HEAD`s still follow the
    /// rewrite, so what they had checked out surfaces as uncommitted changes there -
    /// exactly like the editor's own worktree.
    pub without_checkout: bool,
}

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    /// The linked worktrees this edit has to move, with where the rewrite put each
    /// one, validated against the shape recorded at editor creation.
    pub(super) fn linked_checkout_specs(&self) -> Result<Vec<LinkedCheckoutSpec>> {
        let mut specs = Vec::new();
        for checkout in &self.checkouts {
            let Checkout::Worktree {
                worktree_name,
                selector,
                ref_name: expected_ref,
                initial_head,
                merge_base_override,
            } = checkout
            else {
                continue;
            };
            let (target, actual_ref) = self
                .checkout_target(*selector)?
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
        let Some((selector, merge_base_override)) =
            self.checkouts.iter().find_map(|checkout| match checkout {
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => Some((*selector, *merge_base_override)),
                Checkout::Worktree { .. } => None,
            })
        else {
            return Ok(None);
        };
        let (target, ref_name) = self
            .checkout_target(selector)?
            .context("Checkout selector is pointing to none")?;
        Ok(Some(HeadCheckout {
            target,
            ref_name,
            merge_base_override,
        }))
    }

    /// Materializes a history rewrite.
    pub fn materialize(
        mut self,
        materialize_options: MaterializeOptions,
    ) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let specs = self.linked_checkout_specs()?;
        let detached_head_edits = detached_worktree_head_edits(&specs)?;

        let (head, checkout_conflict_occurred) = if !materialize_options.without_checkout {
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
            (head, checkout_conflict_occurred)
        } else {
            (None, false)
        };

        let mut ref_edits = self.ref_edits.clone();
        ref_edits.extend(detached_head_edits);

        if !materialize_options.without_checkout
            && let Some(refname) = head.and_then(|head| head.ref_name)
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
            checkout_conflict_occurred,
        })
    }

    /// Convenience for [Self::materialize] with
    /// [MaterializeOptions::without_checkout] set.
    pub fn materialize_without_checkout(self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        self.materialize(MaterializeOptions {
            without_checkout: true,
        })
    }
}
