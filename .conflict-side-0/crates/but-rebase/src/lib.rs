//! An API for an interactive rebases, suitable for interactive, UI driven, and programmatic use.
//!
//! It will only affect the commit-graph, and never the alter the worktree in any way.
#![deny(missing_docs)]

use anyhow::{Context, Ok, Result, anyhow, bail};
use bstr::BString;
use gix::{objs::Exists, prelude::ObjectIdExt};
use tracing::instrument;

use crate::commit::DateMode;

/// Types for use with cherry-picking
pub mod cherry_pick;
pub use cherry_pick::function::cherry_pick_one;

use crate::cherry_pick::{EmptyCommit, PickMode};

/// Utilities to create commits (and deal with signing)
pub mod commit;
/// Utilities around merging
pub mod merge;

/// An instruction for [`RebaseBuilder::rebase()`].
#[derive(Debug, Clone)]
pub enum RebaseStep {
    /// Pick an existing commit and place it on top of `base` and optionally reword it.
    Pick {
        /// Id of an already existing commit
        commit_id: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: Option<BString>,
        // TODO: add `base: Option<ObjectId>` to allow restarting the sequence at a new base
        //       for multi-branch rebasing. It would keep the previous cursor, to allow the last
        //       branch to contain a pick of the merge commit on top, which it can then correctly re-merge.
    },
    /// Squashes an existing commit into the one in the first `Pick` or `Merge` RebaseStep that precedes it.
    ///
    /// If there are neither `Pick` nor `Merge` steps preceding this operation (e.g. only `Reference` steps), the execution will halt with an error.
    /// If the step immediately preceding this step is a `Reference` step, the commit will be squashed into the commit that is referenced.
    /// If the step immediately preceding this step is another `Fixup` step, the commit will be squashed into the same commit as the previous `Fixup` step.
    ///
    /// Optionally sets the message of the new commit.
    SquashIntoPreceding {
        /// Id of an already existing commit
        commit_id: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: Option<BString>,
    },
    /// Create a new reference pointing to the commit that precedes this step.
    /// If this is the first step in the list, the reference will be to the `base` commit.
    /// If the step before this one is another `Reference` step, this reference will point to the same commit.
    Reference(but_core::Reference),
}

impl RebaseStep {
    /// Get the commit id associated with a given step
    pub fn commit_id(&self) -> Option<&gix::oid> {
        match self {
            RebaseStep::Pick { commit_id, .. }
            | RebaseStep::SquashIntoPreceding { commit_id, .. } => Some(commit_id),
            RebaseStep::Reference { .. } => None,
        }
    }
}

/// Setup a list of [instructions](RebaseStep) for the actual [rebase operation](RebaseBuilder::rebase).
#[derive(Debug)]
pub struct Rebase<'repo> {
    repo: &'repo gix::Repository,
    base: Option<gix::ObjectId>,
    base_substitute: Option<gix::ObjectId>,
    steps: Vec<RebaseStep>,
    rebase_noops: bool,
}

impl<'repo> Rebase<'repo> {
    /// Creates a new rebase builder with the provided commit as a `base`, the commit
    /// that all other commits should be placed on top of.
    /// If `None` this means the first picked commit will have no parents.
    /// This means that the first [picked commit](Self::step()) will be placed right on top of `base`.
    ///
    /// If the first pick refers to a merge-commit then we will have to prove it's connected to the `base` commit.
    /// If that `base`, however, is also a new commit, we'd have no way of figuring out which parent in the picked merge
    /// is replaced with `base` to know which commits are involved in the merge.
    /// The `base_substitute` passed here is the commit that stands in for `base` in the original graph that the
    /// picked merge commit is linked to.
    pub fn new(
        repo: &'repo gix::Repository,
        base: impl Into<Option<gix::ObjectId>>,
        base_substitute: Option<gix::ObjectId>,
    ) -> Result<Self> {
        let base = base.into();
        if base.is_some() && base.filter(|base| repo.exists(base)).is_none() {
            bail!("Base commit must exist if provided: {}", base.unwrap());
        }
        Ok(Self {
            repo,
            base,
            base_substitute,
            steps: Vec::new(),
            rebase_noops: true, // default to always rebasing
        })
    }

    /// Adds and validates a list of rebase steps.
    /// Ordered oldest (parentmost) to newest (childmost). Reference steps refer to the commit that precedes them.
    /// Note that `steps` will extend whatever steps were added before.
    pub fn steps(&mut self, steps: impl IntoIterator<Item = RebaseStep>) -> Result<&mut Self> {
        for step in steps {
            self.validate_step(&step)?;
            self.steps.push(step);
        }
        Ok(self)
    }

    /// Configures whether the noop steps should be rebased regardless.
    /// If set to true, commits that dont really change will have their timestamps and ids updated.
    /// Default is `true`
    pub fn rebase_noops(&mut self, value: bool) -> &mut Self {
        self.rebase_noops = value;
        self
    }

    /// Performs a rebase on top of a given base, according to the provided steps, or fails if no step was provided.
    /// It does not actually create new git references nor does it update existing ones, it only deals with
    /// altering commits and providing the information needed to update refs.
    ///
    /// Use it to
    ///
    ///  - drop commits
    ///  - insert new commits
    ///  - reorder commits
    ///  - rewrite the history at will
    ///
    /// **However, note that it will also make all input commits sequential, so the caller must assure
    /// these actually form a 'line'.**
    pub fn rebase(&mut self) -> Result<RebaseOutput> {
        if self.steps.is_empty() {
            return Err(anyhow!("No rebase steps provided"));
        }
        let pick_mode = if self.rebase_noops {
            PickMode::Unconditionally
        } else {
            PickMode::SkipIfNoop
        };
        rebase(
            self.repo,
            self.base,
            self.base_substitute,
            std::mem::take(&mut self.steps),
            pick_mode,
        )
    }
}

impl Rebase<'_> {
    /// Pick, Merge and Fixup operations:
    /// - The commit must already exist in the repository
    /// - The commit must not be the base commit
    ///
    /// Pick and Merge operations:
    /// - The commit must not be a commit that is already in a pick, merge or fixup step
    ///
    /// Fixup operations:
    /// - Must not be a reference step immediately before it
    /// - Must not be the first operation
    ///
    /// Reference operations:
    /// - The refname must be a valid reference name
    fn validate_step(&self, step: &RebaseStep) -> Result<()> {
        match step {
            RebaseStep::Pick { commit_id, .. } => {
                self.assure_unique_step_and_existing_non_base(commit_id, "Picked")?;
            }
            RebaseStep::SquashIntoPreceding {
                commit_id,
                new_message: _,
            } => {
                self.assure_unique_step_and_existing_non_base(commit_id, "Fixup")?;
                if matches!(self.steps.last(), Some(RebaseStep::Reference { .. })) {
                    bail!("Fixup commit must not come after a reference step");
                }
                if self.steps.is_empty() {
                    bail!("Fixup must have a commit to work on");
                }
            }
            RebaseStep::Reference(name) => {
                if matches!(name, but_core::Reference::Virtual(name) if name.is_empty()) {
                    return Err(anyhow!(
                        "Reference step must have a non-empty virtual branch name"
                    ));
                }
            }
        }
        Ok(())
    }

    fn assure_unique_step_and_existing_non_base(
        &self,
        commit_id: &gix::oid,
        kind: &str,
    ) -> Result<()> {
        self.repo.find_commit(commit_id)?;
        if Some(commit_id) == self.base.as_deref() {
            bail!("{kind} commit cannot be the base commit");
        }
        if self.steps.iter().any(|s| s.commit_id() == Some(commit_id)) {
            bail!("Picked commit already exists in a previous step");
        }
        Ok(())
    }
}

#[instrument(level = tracing::Level::DEBUG, skip(repo))]
fn rebase(
    repo: &gix::Repository,
    base: Option<gix::ObjectId>,
    base_substitute: Option<gix::ObjectId>,
    steps: Vec<RebaseStep>,
    pick_mode: PickMode,
) -> Result<RebaseOutput> {
    let (mut references, mut commit_mapping) = (
        vec![],
        Vec::<(Option<gix::ObjectId>, gix::ObjectId, gix::ObjectId)>::new(),
    );
    let (mut cursor, mut last_seen_commit) = (base, base);
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    for step in steps {
        match step {
            RebaseStep::Pick {
                commit_id,
                new_message,
            } => {
                // This should be the source commit id
                last_seen_commit = Some(commit_id);

                let commit = to_commit(repo, commit_id)?;
                if commit.parents.len() > 1 {
                    let mut merge_commit = commit;
                    if let Some(new_message) = new_message {
                        merge_commit.message = new_message;
                    }
                    // Find any parent that we have seen during picking.
                    let parent_to_replace = match merge_commit.parents.iter_mut().find(|id| {
                        (Some(**id) == base_substitute)
                            || commit_mapping.iter().any(|(mapping_base, old, _new)| {
                                *mapping_base == base && (*id == old)
                            })
                    }) {
                        None => merge_commit
                            .parents
                            .iter_mut()
                            .next()
                            .expect("more than one parents"),
                        Some(parent) => parent,
                    };
                    *parent_to_replace = cursor.context("Expecting a base for any merge")?;
                    cursor = merge::octopus(repo, merge_commit, &mut graph)
                        .context(
                            "The rebase failed as a merge could not be repeated without conflicts",
                        )?
                        .into();
                } else {
                    match &mut cursor {
                        Some(cursor) => {
                            let mut new_commit = cherry_pick_one(
                                repo,
                                *cursor,
                                commit_id,
                                pick_mode,
                                EmptyCommit::Keep,
                            )?;
                            if let Some(new_message) = new_message {
                                new_commit = reword_commit(repo, new_commit, new_message.clone())?;
                            }
                            *cursor = new_commit;
                        }
                        None if commit.parents.is_empty() => {
                            let mut new_commit = commit;
                            if let Some(new_message) = new_message {
                                new_commit.message = new_message;
                            }
                            cursor = Some(commit::create(
                                repo,
                                new_commit,
                                DateMode::CommitterUpdateAuthorKeep,
                            )?);
                        }
                        None => {
                            // TODO: should this be supported? This would be as easy as forgetting its parents.
                            bail!(
                                "Cannot currently rebase a commit so that it becomes the first commit in the history"
                            )
                        }
                    }
                }
            }
            RebaseStep::SquashIntoPreceding {
                commit_id,
                new_message,
            } => {
                let Some(cursor) = &mut cursor else {
                    bail!("Can't squash if previous commit is missing");
                };
                last_seen_commit = Some(commit_id);
                let base_commit = repo.find_commit(*cursor)?;
                let new_commit = cherry_pick_one(
                    repo,
                    *cursor,
                    commit_id,
                    PickMode::Unconditionally,
                    EmptyCommit::Keep,
                )?;

                // Now, lets pretend the base didn't exist by swapping parent with the parent of the base
                let mut new_commit = repo.find_commit(new_commit)?.decode()?.to_owned();
                new_commit.parents = base_commit.parent_ids().map(|id| id.detach()).collect();
                if let Some(new_message) = new_message {
                    new_commit.message = new_message;
                }
                *cursor = commit::create(repo, new_commit, DateMode::CommitterUpdateAuthorKeep)?;
            }
            RebaseStep::Reference(reference) => {
                references.push(ReferenceSpec {
                    reference,
                    commit_id: cursor
                        .expect("Validation assures there is a rewritten commit prior"),
                    previous_commit_id: last_seen_commit
                        .expect("Validation assures there is a commit prior"),
                });
            }
        }
        if let Some((old, new)) = last_seen_commit.zip(cursor) {
            commit_mapping.push((base, old, new));
        }
    }

    Ok(RebaseOutput {
        top_commit: cursor.expect("validation assures we have at least one commit to process"),
        references,
        commit_mapping,
    })
}

fn to_commit(repo: &gix::Repository, commit_id: gix::ObjectId) -> Result<gix::objs::Commit> {
    Ok(commit_id
        .attach(repo)
        .object()?
        .into_commit()
        .decode()?
        .into())
}

fn reword_commit(
    repo: &gix::Repository,
    oid: gix::ObjectId,
    new_message: BString,
) -> Result<gix::ObjectId> {
    let mut new_commit = repo.find_commit(oid)?.decode()?.to_owned();
    new_commit.message = new_message;
    Ok(commit::create(
        repo,
        new_commit,
        DateMode::CommitterUpdateAuthorKeep,
    )?)
}

/// Replaces the tree of a commit for use in the rebase engine.
pub fn replace_commit_tree(
    repo: &gix::Repository,
    oid: gix::ObjectId,
    new_tree: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let mut new_commit = repo.find_commit(oid)?.decode()?.to_owned();
    new_commit.tree = new_tree;
    Ok(commit::create(
        repo,
        new_commit,
        DateMode::CommitterUpdateAuthorKeep,
    )?)
}

/// A reference that is an output of a rebase operation.
/// This is simply a marker for where the actual reference should point to after the rebase operation.
#[derive(Debug, Clone)]
pub struct ReferenceSpec {
    /// A literal reference, useful only to the caller.
    pub reference: but_core::Reference,
    /// The commit it now points to.
    pub commit_id: gix::ObjectId,
    /// The commit it previously pointed to (as per pick-list).
    /// Useful for reference-transactions that validate the current value before changing it to the new one.
    pub previous_commit_id: gix::ObjectId,
}

/// The output of the [rebase](RebaseBuilder::rebase()) operation.
#[derive(Debug, Clone)]
pub struct RebaseOutput {
    /// The id of the most recently created commit in the rebase operation.
    pub top_commit: gix::ObjectId,
    /// The list of references along with their new locations, ordered from the least recent to the most recent.
    pub references: Vec<ReferenceSpec>,
    /// A listing of all commits `(base, old, new)` in order of [steps](RebaseBuilder::step()), with its base followed by
    /// the initial commit hash on the left and the rewritten version of it on the right side of each tuple.
    ///
    /// That way programmatic users may perform their own remapping without having to deal with [references](RebaseStep::Reference).
    pub commit_mapping: Vec<(Option<gix::ObjectId>, gix::ObjectId, gix::ObjectId)>,
}
