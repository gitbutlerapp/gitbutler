//! Functions for materializing a rebase
use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{
        checkout::{Options, PreparedCheckout},
        prepare_safe_checkout_from_head,
    },
};
use gix::{
    bstr::{BString, ByteSlice as _},
    refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    },
};

use crate::graph_rebase::{Checkout, MaterializeOutcome, SuccessfulRebase};

struct LinkedCheckoutSpec {
    name: BString,
    initial_head: gix::ObjectId,
    ref_name: Option<gix::refs::FullName>,
    target: gix::ObjectId,
}

struct LinkedCheckoutRepo {
    name: BString,
    repo: gix::Repository,
    target: gix::ObjectId,
    detached: bool,
}

fn open_linked_checkout_repos(
    repo: &gix::Repository,
    specs: Vec<LinkedCheckoutSpec>,
) -> Result<Vec<LinkedCheckoutRepo>> {
    specs
        .into_iter()
        .map(|spec| {
            let proxy = repo
                .worktrees()?
                .into_iter()
                .find(|proxy| proxy.id() == spec.name.as_bstr())
                .with_context(|| format!("Visible worktree {} no longer exists", spec.name))?;
            let worktree_repo = proxy.into_repo()?;
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
                name: spec.name,
                repo: worktree_repo,
                target: spec.target,
                detached: spec.ref_name.is_none(),
            })
        })
        .collect()
}

fn prepare_linked_checkouts(repos: &[LinkedCheckoutRepo]) -> Result<Vec<PreparedCheckout<'_>>> {
    repos
        .iter()
        .map(|checkout| {
            prepare_safe_checkout_from_head(
                checkout.target,
                &checkout.repo,
                Options {
                    skip_head_update: !checkout.detached,
                    merge_base_override: None,
                    allow_conflicted_commit_checkout: false,
                },
            )
            .with_context(|| format!("Cannot update linked worktree {}", checkout.name))
        })
        .collect()
}

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    fn linked_checkout_specs(&self) -> Result<Vec<LinkedCheckoutSpec>> {
        self.checkouts
            .iter()
            .filter_map(|checkout| {
                let Checkout::Worktree {
                    worktree_name,
                    selector,
                    ref_name,
                    initial_head,
                } = checkout
                else {
                    return None;
                };
                Some((worktree_name, selector, ref_name, initial_head))
            })
            .map(|(worktree_name, selector, expected_ref, initial_head)| {
                let (target, actual_ref) = self.checkout_target(*selector)?.with_context(|| {
                    format!("Visible worktree {worktree_name} HEAD was removed")
                })?;
                if actual_ref.as_ref() != expected_ref.as_ref() {
                    bail!("Visible worktree {worktree_name} HEAD changed shape during the edit");
                }
                Ok(LinkedCheckoutSpec {
                    name: worktree_name.clone(),
                    initial_head: *initial_head,
                    ref_name: expected_ref.clone(),
                    target,
                })
            })
            .collect()
    }

    fn head_checkout(
        &self,
    ) -> Result<(
        gix::ObjectId,
        Option<gix::refs::FullName>,
        Option<gix::ObjectId>,
    )> {
        let (selector, merge_base_override) = self
            .checkouts
            .iter()
            .find_map(|checkout| match checkout {
                Checkout::Head {
                    selector,
                    merge_base_override,
                } => Some((*selector, *merge_base_override)),
                Checkout::Worktree { .. } => None,
            })
            .context("Editor has no HEAD checkout")?;
        let (target, ref_name) = self
            .checkout_target(selector)?
            .context("Checkout selector is pointing to none")?;
        Ok((target, ref_name, merge_base_override))
    }

    /// Materializes a history rewrite.
    pub fn materialize(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        let linked_repos = open_linked_checkout_repos(&repo, self.linked_checkout_specs()?)?;
        let prepared_linked = prepare_linked_checkouts(&linked_repos)?;
        let (new_head, new_head_refname, merge_base_override) = self.head_checkout()?;
        let prepared_head = prepare_safe_checkout_from_head(
            new_head,
            &repo,
            Options {
                skip_head_update: true,
                merge_base_override,
                allow_conflicted_commit_checkout: true,
            },
        )?;

        let mut ref_edits = self.ref_edits.clone();
        if let Some(refname) = new_head_refname
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
        for (prepared, checkout) in prepared_linked.into_iter().zip(&linked_repos) {
            prepared
                .execute()
                .with_context(|| format!("Failed to update linked worktree {}", checkout.name))?;
        }
        prepared_head.execute()?;

        repo.edit_references(ref_edits)?;

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }

    /// Materializes a rebase without checking out the editor's own worktree.
    pub fn materialize_without_checkout(mut self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let repo = self.repo.clone();
        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        repo.edit_references(self.ref_edits.clone())?;

        let project_meta = self.workspace.graph.project_meta.clone();
        self.workspace
            .refresh_from_head(&repo, &*self.meta, project_meta)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}
