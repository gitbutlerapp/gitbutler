//! Write a sealed, rebased graph back to disk.
//!
//! Unlike the previous editor, the reference transaction is computed *here* by
//! diffing the sealed graph against the live on-disk references, not at rebase
//! time: the optimistic-lock snapshot is taken as late as possible and chained
//! rebases never compute edits that get thrown away.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use bstr::{BString, ByteSlice as _};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::refs::{
    Category, Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

use crate::{
    NodeGraph, NodeKind,
    edit::{NodePolicy, Rebased},
    node::resolve_to_commit,
};

/// Options for [`Rebased::materialize_changes`].
#[derive(Debug, Clone)]
pub struct MaterializeOptions {
    /// Whether to perform a `safe_checkout` so the worktree follows the
    /// rewritten `HEAD`.
    ///
    /// For the vast majority of operations you want `true`. Skipping the
    /// checkout is intended for niche cases like `uncommit`: if a commit was
    /// dropped from history, skipping the checkout leaves its changes in the
    /// working directory instead of removing them from disk.
    pub checkout: bool,
}

impl Default for MaterializeOptions {
    fn default() -> Self {
        Self { checkout: true }
    }
}

/// The outcome of a materialize.
#[derive(Debug)]
pub struct MaterializeOutcome {
    /// A fresh workspace projection, re-traversed from the on-disk repository
    /// after the write. This is the canonical post-materialize state.
    pub workspace: crate::Workspace,
    /// The sealed rebased graph the write was computed from. Node indexes are
    /// still valid against pre-materialize indexes.
    pub graph: NodeGraph,
    /// A mapping between original and rewritten commit ids.
    pub commit_mappings: BTreeMap<gix::ObjectId, gix::ObjectId>,
    /// The reference edits that were applied, for observability.
    pub ref_edits: Vec<RefEdit>,
}

impl Rebased {
    /// Materializes the history rewrite: persists in-memory objects, updates
    /// references in one transaction, checks out the rewritten `HEAD` (per
    /// `options`), and re-traverses the repository into a fresh workspace.
    pub fn materialize_changes<M: RefMetadata>(
        self,
        meta: &M,
        options: MaterializeOptions,
    ) -> Result<MaterializeOutcome> {
        let Rebased {
            graph,
            policy,
            mut session,
        } = self;

        let repo = session.repo.clone();
        if let Some(memory) = session.repo.objects.take_object_memory() {
            memory.persist(session.repo)?;
        }

        // Compute the reference transaction by diffing the sealed graph
        // against the live on-disk references.
        let mut ref_edits = Vec::new();
        let mut present_references = std::collections::HashSet::new();
        for (index, node) in graph.nodes().iter().enumerate() {
            let NodeKind::Reference(reference) = node.kind() else {
                continue;
            };
            let refname = reference.ref_info.ref_name.clone();
            present_references.insert(refname.clone());
            if !matches!(
                policy.get(index),
                Some(NodePolicy::Reference { mutable: true })
            ) {
                continue;
            }
            if refname.category() != Some(Category::LocalBranch) {
                bail!(
                    "BUG: only local branches may be moved or created, but {refname} is marked mutable"
                );
            }
            let target = resolve_to_commit(graph.nodes(), index)
                .and_then(|target| graph.nodes()[target].kind().addressable_commit_id())
                .context("References should have at least one parent")?;

            match repo.try_find_reference(&refname)? {
                Some(disk_reference) => match disk_reference.target() {
                    gix::refs::TargetRef::Object(id) => {
                        if id != target {
                            ref_edits.push(RefEdit {
                                name: refname,
                                change: Change::Update {
                                    log: LogChange::default(),
                                    expected: PreviousValue::MustExistAndMatch(
                                        disk_reference.target().into(),
                                    ),
                                    new: Target::Object(target),
                                },
                                deref: false,
                            });
                        }
                    }
                    gix::refs::TargetRef::Symbolic(name) => {
                        bail!("Attempted to update the symbolic reference {name}");
                    }
                },
                None => {
                    ref_edits.push(RefEdit {
                        name: refname,
                        change: Change::Update {
                            log: LogChange::default(),
                            expected: PreviousValue::MustNotExist,
                            new: Target::Object(target),
                        },
                        deref: false,
                    });
                }
            }
        }

        // Mutable references present when editing started but gone from the
        // sealed graph are deleted — unless their chain died during the edit,
        // in which case they are deliberately left behind.
        for reference in &session.initial_references {
            if !present_references.contains(reference) && !session.left_behind.contains(reference) {
                ref_edits.push(RefEdit {
                    name: reference.clone(),
                    change: Change::Delete {
                        log: gix::refs::transaction::RefLog::AndReference,
                        expected: PreviousValue::MustExist,
                    },
                    deref: false,
                });
            }
        }

        // Checkout and HEAD.
        if options.checkout {
            let (new_head, symbolic) = graph.resolve_head(session.checkout_index)?;
            safe_checkout_from_head(
                new_head,
                &repo,
                Options {
                    skip_head_update: true,
                    merge_base_override: session.merge_base_override,
                    allow_conflicted_commit_checkout: true,
                },
            )?;
            {
                let target = match symbolic {
                    Some(refname) => Target::Symbolic(refname),
                    None => Target::Object(new_head),
                };
                let unchanged = match &target {
                    Target::Object(id) => {
                        repo.head_name()?.is_none() && repo.head_id()?.detach() == *id
                    }
                    Target::Symbolic(refname) => repo.head_name()?.as_ref() == Some(refname),
                };
                if !unchanged {
                    let target_description: BString = match &target {
                        Target::Object(id) => id.to_string().into(),
                        Target::Symbolic(refname) => refname.shorten().to_owned(),
                    };
                    ref_edits.push(RefEdit {
                        change: Change::Update {
                            log: LogChange {
                                mode: RefLog::AndReference,
                                force_create_reflog: false,
                                message: gix::reference::log::message(
                                    "safe checkout",
                                    target_description.as_bstr(),
                                    0,
                                ),
                            },
                            expected: PreviousValue::Any,
                            new: target,
                        },
                        name: "HEAD".try_into().expect("root refs are always valid"),
                        deref: false,
                    });
                }
            }
        }

        repo.edit_references(ref_edits.clone())?;

        let workspace = crate::Graph::from_repo(
            &repo,
            meta,
            graph.project_meta().clone(),
            crate::init::Overlay::default(),
        )?
        .validated()?
        .into_workspace()?;

        Ok(MaterializeOutcome {
            workspace,
            graph,
            commit_mappings: session.commit_mappings.mappings(),
            ref_edits,
        })
    }
}
