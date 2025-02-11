//! An API for an interactive rebase.
#![deny(rust_2018_idioms, missing_docs)]

use anyhow::{anyhow, bail, Ok, Result};
use bstr::{BString, ByteSlice};
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_repo::rebase::{cherry_rebase_group, merge_commits};

///
pub mod commit;

/// An instruction for [`RebaseBuilder::rebase()`].
#[derive(Debug)]
pub enum RebaseStep {
    /// Pick an existing commit and place it on top of `base` and optionally reword it.
    Pick {
        /// Id of an already existing commit
        commit_id: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: Option<BString>,
    },
    /// Merge an existing commit and it's parents producing a new merge commit.
    Merge {
        /// Id of an already existing commit
        commit_id: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: BString,
    },
    /// Squashes an existing commit into the one in the first `Pick` or `Merge` RebaseStep that precedes it.
    ///
    /// If there are neither `Pick` nor `Merge` steps preceding this operation (e.g. only `Reference` steps), the execution will halt with an error.
    /// If the step immediately preceding this step is a `Reference` step, the commit will be squashed into the commit that is referenced.
    /// If the step immediately preceding this step is another `Fixup` step, the commit will be squashed into the same commit as the previous `Fixup` step.
    ///
    /// Optionally sets the message of the new commit.
    Fixup {
        /// Id of an already existing commit
        commit_id: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: Option<BString>,
    },
    /// Create a new reference pointing to the commit that precedes this step.
    /// If this is the first step in the list, the reference will be to the `base` commit.
    /// If the step before this one is another `Reference` step, this reference will point to the same commit.
    Reference {
        /// The name of the reference (literally, a name to refer to an object, not necessarily a valid Git reference)
        /// that should refer to the possibly rewritten commit that precedes it, as returned in [`RebaseOutput::references`].
        name: BString,
    },
}

impl RebaseStep {
    fn commit_id(&self) -> Option<&gix::oid> {
        match self {
            RebaseStep::Pick { commit_id, .. }
            | RebaseStep::Merge { commit_id, .. }
            | RebaseStep::Fixup { commit_id, .. } => Some(commit_id),
            RebaseStep::Reference { .. } => None,
        }
    }
}

/// Setup a list of [instructions](RebaseStep) for the actual [rebase operation](RebaseBuilder::rebase).
#[derive(Debug)]
pub struct RebaseBuilder {
    repo: gix::Repository,
    base: gix::ObjectId,
    steps: Vec<RebaseStep>,
}

impl RebaseBuilder {
    /// Creates a new rebase builder with the provided commit as a `base`, the commit
    /// that all other commits should be placed on top of.
    /// This means that the first [picked commit](Self::step()) will be placed right on top of `base`.
    pub fn new(repo: gix::Repository, base: gix::ObjectId) -> Result<Self> {
        repo.find_commit(base)?;
        Ok(Self {
            repo,
            base,
            steps: Vec::new(),
        })
    }

    /// Adds a rebase step to the list of steps.
    /// The steps must be added in the order in which they should appear in the graph,
    /// i.e. the first step will be the first commit in the rebase and the last step will be the last commit.
    pub fn step(&mut self, step: RebaseStep) -> Result<&mut Self> {
        self.validate_step(&step)?;
        self.steps.push(step);
        Ok(self)
    }

    /// Performs a rebase on top of a given base, according to the provided steps, or fails if no step was provided.
    /// It does not actually create new git references nor does it update existing ones, it only deals with
    /// altering commits and providing the information needed to update refs.
    pub fn rebase(self) -> Result<RebaseOutput> {
        if self.steps.is_empty() {
            return Err(anyhow!("No rebase steps provided"));
        }
        rebase(&self.repo, self.base, self.steps)
    }
}

impl RebaseBuilder {
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
            RebaseStep::Pick {
                commit_id,
                new_message: _,
            } => {
                self.assure_unique_step_and_existing_non_base(commit_id, "Picked")?;
            }
            RebaseStep::Merge {
                commit_id,
                new_message: _,
            } => {
                self.assure_unique_step_and_existing_non_base(commit_id, "Merge")?;
            }
            RebaseStep::Fixup {
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
            RebaseStep::Reference { name } => {
                if name.is_empty() {
                    return Err(anyhow!("Reference step must have a non-empty name"));
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
        if commit_id == self.base {
            bail!("{kind} commit cannot be the base commit");
        }
        if self.steps.iter().any(|s| s.commit_id() == Some(commit_id)) {
            bail!("Picked commit already exists in a previous step");
        }
        Ok(())
    }
}

fn rebase(
    repo: &gix::Repository,
    base: gix::ObjectId,
    steps: Vec<RebaseStep>,
) -> Result<RebaseOutput> {
    let git2_repo = git2::Repository::open(repo.path())?;
    let mut references = vec![];
    // Start with the base commit
    let (mut cursor, mut last_seen_commit) = (base, base);
    // Running cherry_rebase_group for each step individually
    for step in steps {
        match step {
            RebaseStep::Pick {
                commit_id,
                new_message,
            } => {
                last_seen_commit = commit_id;
                let mut new_commit =
                    cherry_rebase_group(&git2_repo, base.to_git2(), &[commit_id.to_git2()], true)?
                        .to_gix();
                if let Some(new_message) = new_message {
                    new_commit = reword_commit(repo, new_commit, new_message.clone())?;
                }
                // Update the base for the next loop iteration
                cursor = new_commit;
            }
            RebaseStep::Merge {
                commit_id,
                new_message,
            } => {
                last_seen_commit = commit_id;
                cursor = merge_commits(repo, cursor, commit_id, &new_message.to_str_lossy())?;
            }
            RebaseStep::Fixup {
                commit_id,
                new_message,
            } => {
                last_seen_commit = commit_id;
                // This time, the base is the parent of the last commit
                let base_commit = repo.find_commit(cursor)?;

                // First cherry-pick the target oid on top of base_commit
                let new_commit = cherry_rebase_group(
                    &git2_repo,
                    cursor.to_git2(),
                    &[commit_id.to_git2()],
                    true,
                )?
                .to_gix();

                // Now, lets pretend the base didn't exist by swapping parent with the parent of the base
                let commit = repo.find_commit(new_commit)?;
                let mut new_commit = commit.decode()?.to_owned();
                new_commit.parents = base_commit.parent_ids().map(|id| id.detach()).collect();
                if let Some(new_message) = new_message {
                    new_commit.message = new_message;
                }
                cursor = commit::create(repo, new_commit)?;
            }
            RebaseStep::Reference { name: refname } => {
                references.push(ReferenceSpec {
                    refname: refname.clone(),
                    commit_id: cursor,
                    previous_commit_id: last_seen_commit,
                });
            }
        }
    }

    Ok(RebaseOutput {
        top_commit: cursor,
        references,
    })
}

fn reword_commit(
    repo: &gix::Repository,
    oid: gix::ObjectId,
    new_message: BString,
) -> Result<gix::ObjectId> {
    let mut new_commit = repo.find_commit(oid)?.decode()?.to_owned();
    new_commit.message = new_message;
    Ok(commit::create(repo, new_commit)?)
}

/// A reference that is an output of a rebase operation.
/// This is simply a marker for where the actual reference should point to after the rebase operation.
pub struct ReferenceSpec {
    /// A literal reference, useful only to the caller.
    pub refname: BString,
    /// The commit it now points to.
    pub commit_id: gix::ObjectId,
    /// The commit it previously pointed to (as per pick-list).
    /// Useful for reference-transactions that validate the current value before changing it to the new one.
    pub previous_commit_id: gix::ObjectId,
}

/// The output of the [rebase](RebaseBuilder::rebase()) operation.
pub struct RebaseOutput {
    /// The id of the most recently created commit in the rebase operation.
    pub top_commit: gix::ObjectId,
    /// The list of references along with their new locations, ordered from the least recent to the most recent.
    pub references: Vec<ReferenceSpec>,
}
