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
        transaction::{PreviousValue, RefEdit},
    },
};

use crate::graph_rebase::{Checkout, MaterializeOutcome, SuccessfulRebase};

pub(super) struct LinkedCheckoutSpec {
    pub(super) name: BString,
    pub(super) initial_head: gix::ObjectId,
    pub(super) initial_ref_name: Option<gix::refs::FullName>,
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

fn head_target(id: gix::ObjectId, ref_name: Option<gix::refs::FullName>) -> Target {
    ref_name.map_or(Target::Object(id), Target::Symbolic)
}

/// Ref edits moving the `HEAD` of every linked worktree in `specs` to where the
/// rewrite put it: attached to a branch, or detached at a commit.
///
/// A `HEAD` that stays attached to the same branch needs nothing here - it follows
/// the branch edit that is already part of the transaction.
///
/// `worktrees/<name>/HEAD` addresses another worktree's `HEAD` from this repository,
/// so this rides along in the same transaction as the branch updates instead of
/// needing the worktree's own repository handle. Making the initial `HEAD` the
/// expected value lets the transaction reject a worktree that moved under us, rather
/// than checking for it separately and racing.
fn worktree_head_edits(specs: &[LinkedCheckoutSpec]) -> Result<Vec<RefEdit>> {
    specs
        .iter()
        .filter_map(|spec| {
            let initial = head_target(spec.initial_head, spec.initial_ref_name.clone());
            let target = head_target(spec.target, spec.ref_name.clone());
            (initial != target).then_some((spec, initial, target))
        })
        .map(|(spec, initial, target)| {
            let name: gix::refs::FullName = format!("worktrees/{}/HEAD", spec.name)
                .try_into()
                .with_context(|| {
                    format!(
                        "Worktree {} has a name that cannot address its HEAD",
                        spec.name
                    )
                })?;
            Ok(RefEdit::update(
                name,
                target,
                PreviousValue::MustExistAndMatch(initial),
                gix::reference::log::message("rebase", "HEAD".into(), 1),
            ))
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
            if actual_ref != spec.initial_ref_name || actual_head != spec.initial_head {
                bail!(
                    "Visible worktree {} changed since the editor was created: \
                     expected {} at {}, got {} at {}",
                    spec.name,
                    spec.initial_ref_name
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
    /// The linked worktrees this edit has to move, with where the rewrite put each one.
    pub(super) fn linked_checkout_specs(&self) -> Result<Vec<LinkedCheckoutSpec>> {
        let mut specs = Vec::new();
        for checkout in &self.checkouts {
            let Checkout::Worktree {
                worktree_name,
                selector,
                ref_name: initial_ref_name,
                initial_head,
                merge_base_override,
            } = checkout
            else {
                continue;
            };
            let (target, ref_name) = self
                .checkout_target(*selector)?
                .with_context(|| format!("Visible worktree {worktree_name} HEAD was removed"))?;
            specs.push(LinkedCheckoutSpec {
                name: worktree_name.clone(),
                initial_head: *initial_head,
                initial_ref_name: initial_ref_name.clone(),
                ref_name,
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
        if !self.references_updated()? {
            return Ok(MaterializeOutcome {
                graph: self.graph,
                history: self.history,
                workspace: self.workspace,
                meta: self.meta,
                db: self.db,
                checkout_conflict_occurred: false,
            });
        }

        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let specs = self.linked_checkout_specs()?;
        let worktree_head_edits = worktree_head_edits(&specs)?;

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
        ref_edits.extend(worktree_head_edits);

        if !materialize_options.without_checkout
            && let Some(head) = head
        {
            let target = head_target(head.target, head.ref_name);
            if repo.find_reference("HEAD")?.inner.target != target {
                let checked_out = match &target {
                    Target::Symbolic(refname) => refname.shorten().to_owned(),
                    Target::Object(id) => id.to_string().into(),
                };
                ref_edits.push(RefEdit::update(
                    "HEAD".try_into().expect("root refs are always valid"),
                    target,
                    PreviousValue::Any,
                    gix::reference::log::message("safe checkout", checked_out.as_ref(), 0),
                ));
            }
        }

        repo.edit_references(ref_edits)?;

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta, &mut *self.db)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
            db: self.db,
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
