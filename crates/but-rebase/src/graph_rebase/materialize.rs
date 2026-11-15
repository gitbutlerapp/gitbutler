use anyhow::{Context, Result, bail};
use but_core::{
    ObjectStorageExt as _, RefMetadata,
    worktree::{checkout::Options, safe_checkout_from_head},
};
use gix::{
    bstr::{BString, ByteSlice as _},
    refs::{
        Category, Target,
        transaction::{Change, PreviousValue, RefEdit},
    },
};

use crate::graph_rebase::{
    Checkout, ConflictedCheckout, HeadPlacement, MaterializeOutcome, PaddedCheckout,
    SuccessfulRebase,
};

struct LinkedCheckoutSpec {
    name: BString,
    initial_head: gix::ObjectId,
    initial_ref_name: Option<gix::refs::FullName>,
    ref_name: Option<gix::refs::FullName>,
    target: gix::ObjectId,
    merge_base_override: Option<gix::ObjectId>,
}

struct HeadCheckout {
    target: gix::ObjectId,
    ref_name: Option<gix::refs::FullName>,
    merge_base_override: Option<gix::ObjectId>,
    placement: HeadPlacement,
}

struct LinkedCheckoutRepo {
    repo: gix::Repository,
    target: gix::ObjectId,
    merge_base_override: Option<gix::ObjectId>,
    worktree: LinkedWorktreeCheckout,
}

/// Context on an error checking out the linked worktree `name` at `path`, which a rewrite
/// moved to a new commit.
#[derive(Debug, Clone)]
struct LinkedWorktreeCheckout {
    name: BString,
    path: std::path::PathBuf,
}

impl std::fmt::Display for LinkedWorktreeCheckout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Could not update worktree '{}' at {}",
            self.name,
            self.path.display()
        )
    }
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

fn names_nest(a: &gix::refs::FullNameRef, b: &gix::refs::FullNameRef) -> bool {
    fn is_directory_of(directory: &[u8], name: &[u8]) -> bool {
        name.len() > directory.len() && name.starts_with(directory) && name[directory.len()] == b'/'
    }
    let (a, b) = (a.as_bstr(), b.as_bstr());
    is_directory_of(a, b) || is_directory_of(b, a)
}

fn edit_references_deleting_directory_conflicts_first(
    repo: &gix::Repository,
    edits: Vec<RefEdit>,
) -> Result<()> {
    let updated: Vec<_> = edits
        .iter()
        .filter(|edit| matches!(edit.change, Change::Update { .. }))
        .map(|edit| edit.name.clone())
        .collect();
    let (directory_conflicts, edits): (Vec<_>, Vec<_>) = edits.into_iter().partition(|edit| {
        matches!(edit.change, Change::Delete { .. })
            && updated
                .iter()
                .any(|name| names_nest(name.as_ref(), edit.name.as_ref()))
    });
    repo.edit_references(directory_conflicts)?;
    repo.edit_references(edits)?;
    Ok(())
}

fn linked_worktree_id(repo: &gix::Repository) -> Result<Option<BString>> {
    Ok(match repo.worktree() {
        Some(worktree) => worktree.id()?.map(ToOwned::to_owned),
        None => None,
    })
}

fn refuse_deleting_branches_checked_out_elsewhere(
    repo: &gix::Repository,
    moved: &[LinkedCheckoutSpec],
    deleted_branches: &[gix::refs::FullName],
) -> Result<()> {
    if deleted_branches.is_empty() {
        return Ok(());
    }
    let own = linked_worktree_id(repo)?;
    let mut elsewhere = Vec::new();
    for worktree in repo.worktrees_including_main()? {
        let worktree = worktree?;
        let id = linked_worktree_id(&worktree)?;
        let moves_with_the_edit = id == own
            || id
                .as_ref()
                .is_some_and(|id| moved.iter().any(|spec| &spec.name == id));
        if !moves_with_the_edit
            && worktree
                .head_name()?
                .is_some_and(|head| deleted_branches.contains(&head))
            && let Some(dir) = worktree.workdir()
        {
            elsewhere.push(dir.display().to_string());
        }
    }
    if !elsewhere.is_empty() {
        but_error::bail_precondition!(
            "Refusing to delete a branch that is checked out. Worktrees are: {}",
            elsewhere.join(", ")
        );
    }
    Ok(())
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
                .find(|proxy| proxy.id().is_ok_and(|id| id == spec.name.as_bstr()))
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
                worktree: LinkedWorktreeCheckout {
                    name: spec.name,
                    path: proxy.base()?,
                },
            })
        })
        .collect()
}

#[derive(Default)]
pub struct MaterializeOptions {
    /// Materializes a rebase without checking out the editor's own worktree.
    ///
    /// Linked worktrees aren't checked out either, but their `HEAD`s still follow the
    /// rewrite, so what they had checked out surfaces as uncommitted changes there -
    /// exactly like the editor's own worktree.
    pub without_checkout: bool,
}

/// The linked worktrees this edit has to move, with where the rewrite put each one.
fn linked_checkout_specs(checkouts: &[PaddedCheckout<'_>]) -> Result<Vec<LinkedCheckoutSpec>> {
    checkouts
        .iter()
        .filter_map(|PaddedCheckout { checkout, tip }| match checkout {
            Checkout::Worktree {
                worktree_name,
                ref_name,
                initial_head,
                merge_base_override,
                ..
            } => Some((
                worktree_name,
                ref_name,
                initial_head,
                merge_base_override,
                tip,
            )),
            Checkout::Head { .. } => None,
        })
        .map(
            |(worktree_name, initial_ref_name, initial_head, merge_base_override, tip)| {
                let tip = tip.as_ref().with_context(|| {
                    format!("Visible worktree {worktree_name} HEAD was removed")
                })?;
                Ok(LinkedCheckoutSpec {
                    name: worktree_name.clone(),
                    initial_head: *initial_head,
                    initial_ref_name: initial_ref_name.clone(),
                    ref_name: tip.ref_name.clone(),
                    target: tip.target(),
                    merge_base_override: *merge_base_override,
                })
            },
        )
        .collect()
}

/// The editor's own `HEAD` checkout, if it recorded one.
fn head_checkout(checkouts: &[PaddedCheckout<'_>]) -> Result<Option<HeadCheckout>> {
    let Some((merge_base_override, placement, tip)) =
        checkouts
            .iter()
            .find_map(|PaddedCheckout { checkout, tip }| match checkout {
                Checkout::Head {
                    merge_base_override,
                    placement,
                    ..
                } => Some((*merge_base_override, *placement, tip)),
                Checkout::Worktree { .. } => None,
            })
    else {
        return Ok(None);
    };
    let tip = tip
        .as_ref()
        .context("Checkout selector is pointing to none")?;
    if matches!(placement, HeadPlacement::Switched) && tip.padding.is_some() {
        return Err(
            anyhow::anyhow!(but_error::Code::ConflictedCommitCheckout).context(format!(
                "Refusing to check out conflicted commit {}",
                tip.commit
            )),
        );
    }
    Ok(Some(HeadCheckout {
        target: tip.target(),
        ref_name: tip.ref_name.clone(),
        merge_base_override,
        placement,
    }))
}

impl<'ws, 'graph, M: RefMetadata> SuccessfulRebase<'ws, 'graph, M> {
    fn refresh_workspace(&mut self, repo: &gix::Repository) -> Result<()> {
        self.workspace.graph.options.extra_refs = self.extra_refs_after_rebase()?;
        let project_meta = self.workspace.graph.project_meta.clone();
        let options = self.workspace.graph.options.clone();
        let head = repo.head()?;
        // Like a traversal, the graph's extra refs join it, leaving out those that are gone.
        let mut extra_heads = Vec::new();
        for ref_name in &options.extra_refs {
            if let Some(mut reference) = repo.try_find_reference(ref_name)? {
                extra_heads.push(reference.peel_to_commit()?.id);
            }
        }
        let mut workspace = but_graph::nosegment::workspace(
            repo,
            &*self.meta,
            but_graph::init::Overlay::default(),
            project_meta,
            head.referent_name(),
            head.id().map(|id| id.detach()),
            extra_heads,
            but_graph::init::discover_worktree_tips(repo, &mut *self.db, options.worktrees)?,
            options.collect_tags,
            options.commits_limit_hint,
        )?;
        // Workspaces made from this one use the options it was made with, linked worktrees
        // included.
        workspace.graph.options = options;
        *self.workspace = workspace;
        Ok(())
    }

    /// Materializes a history rewrite, and records how its branches stack in the metadata.
    ///
    /// The editor's own checkout and the visible linked worktrees move off a branch the rewrite
    /// deletes. If any other worktree has one checked out, this fails without changing anything.
    pub fn materialize(
        mut self,
        materialize_options: MaterializeOptions,
    ) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        let branch_stacks = self.branch_stacks()?;
        let mut repo = self.repo.clone();
        let checkouts = self.padded_checkouts()?;
        let mut ref_edits = self.padded_ref_edits(&checkouts)?;
        if !self.updates_references(&checkouts, &ref_edits) {
            if branch_stacks.persist(self.meta)? {
                self.refresh_workspace(&repo)?;
            }
            return Ok(MaterializeOutcome {
                graph: self.graph,
                history: self.history,
                workspace: self.workspace,
                meta: self.meta,
                db: self.db,
                conflicted_checkouts: Vec::new(),
            });
        }

        let specs = linked_checkout_specs(&checkouts)?;
        let deleted_branches: Vec<_> = ref_edits
            .iter()
            .filter(|edit| {
                matches!(edit.change, Change::Delete { .. })
                    && edit.name.category() == Some(Category::LocalBranch)
            })
            .map(|edit| edit.name.clone())
            .collect();
        refuse_deleting_branches_checked_out_elsewhere(&repo, &specs, &deleted_branches)?;
        let head = if materialize_options.without_checkout {
            None
        } else {
            head_checkout(&checkouts)?
        };

        if let Some(memory) = self.repo.objects.take_object_memory() {
            memory.persist(&self.repo)?;
        }

        ref_edits.extend(worktree_head_edits(&specs)?);

        let mut conflicted_checkouts = Vec::new();
        if !materialize_options.without_checkout {
            let linked_repos = open_linked_checkout_repos(&repo, specs)?;
            for linked_repo in linked_repos {
                let outcome = safe_checkout_from_head(
                    linked_repo.target,
                    &linked_repo.repo,
                    Options {
                        skip_head_update: true,
                        merge_base_override: linked_repo.merge_base_override,
                        allow_conflicted_commit_checkout: false,
                        allow_uncommitted_changes_to_conflict_with_new_head: true,
                    },
                )
                .with_context(|| linked_repo.worktree.clone())?;
                if outcome.conflict_occurred {
                    let LinkedWorktreeCheckout { name, path } = linked_repo.worktree;
                    conflicted_checkouts.push(ConflictedCheckout::Worktree { name, path });
                }
            }

            if let Some(head) = &head {
                let outcome = safe_checkout_from_head(
                    head.target,
                    &repo,
                    Options {
                        skip_head_update: true,
                        merge_base_override: head.merge_base_override,
                        allow_conflicted_commit_checkout: false,
                        allow_uncommitted_changes_to_conflict_with_new_head: matches!(
                            head.placement,
                            HeadPlacement::Followed
                        ),
                    },
                )?;
                if outcome.conflict_occurred {
                    conflicted_checkouts.push(ConflictedCheckout::Head);
                }
            }
        }

        if let Some(head) = head {
            let target = head_target(head.target, head.ref_name);
            if repo.find_reference("HEAD")?.inner().target != target {
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

        edit_references_deleting_directory_conflicts_first(&repo, ref_edits)?;
        if let Err(err) = repo.delete_local_branches(deleted_branches) {
            tracing::warn!(?err, "deleted branches kept their local configuration");
        }
        branch_stacks.persist(self.meta)?;
        self.refresh_workspace(&repo)?;

        Ok(MaterializeOutcome {
            graph: self.graph,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
            db: self.db,
            conflicted_checkouts,
        })
    }

    pub fn materialize_without_checkout(self) -> Result<MaterializeOutcome<'ws, 'graph, M>> {
        self.materialize(MaterializeOptions {
            without_checkout: true,
        })
    }
}
