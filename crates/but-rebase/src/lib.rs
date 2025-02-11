//! An API for an interactive rebase.
#![deny(rust_2018_idioms, missing_docs)]

use anyhow::{anyhow, bail, Ok, Result};
use bstr::{BString, ByteSlice};
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_repo::rebase::{cherry_rebase_group, merge_commits};

/// An instruction for [`RebaseBuilder::rebase()`].
#[derive(Debug)]
pub enum RebaseStep {
    /// Pick an existing commit and optionally reword it.
    Pick {
        /// Oid of an already existing commit
        oid: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: Option<BString>,
    },
    /// Merge an existing commit and it's parents producing a new merge commit.
    Merge {
        /// Oid of an already existing commit
        oid: gix::ObjectId,
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
        /// Oid of an already existing commit
        oid: gix::ObjectId,
        /// Optional message to use for newly produced commit
        new_message: Option<BString>,
    },
    /// Create a new reference pointing to the commit that precedes this step.
    /// If this is the first step in the list, the reference will be to the `base` commit.
    /// If the step before this one is another `Reference` step, this reference will point to the same commit.
    Reference {
        /// The name of the reference (literally, a name to refer to an object, not necessarily a valid Git reference)
        /// that should refer to the possibly rewritten commit that precedes it, as returned in [`RebaseOutput::references`].
        refname: BString,
    },
}

impl RebaseStep {
    fn commit_id(&self) -> Option<&gix::oid> {
        match self {
            RebaseStep::Pick { oid, .. }
            | RebaseStep::Merge { oid, .. }
            | RebaseStep::Fixup { oid, .. } => Some(oid),
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
                oid,
                new_message: _,
            } => {
                self.assure_unique_step_and_existing_non_base(oid, "Picked")?;
            }
            RebaseStep::Merge {
                oid,
                new_message: _,
            } => {
                self.assure_unique_step_and_existing_non_base(oid, "Merge")?;
            }
            RebaseStep::Fixup {
                oid,
                new_message: _,
            } => {
                self.assure_unique_step_and_existing_non_base(oid, "Fixup")?;
                if matches!(self.steps.last(), Some(RebaseStep::Reference { .. })) {
                    bail!("Fixup commit must not come after a reference step");
                }
                if self.steps.is_empty() {
                    bail!("Fixup must have a commit to work on");
                }
            }
            RebaseStep::Reference { refname } => {
                if refname.is_empty() {
                    return Err(anyhow!("Reference step must have a non-empty refname"));
                }
                // TODO: remove validation as it can be anything really.
                gix::validate::reference::name(refname.as_ref())?;
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
    let mut head = base;
    // Running cherry_rebase_group for each step individually
    for step in steps {
        match step {
            RebaseStep::Pick { oid, new_message } => {
                let mut new_head =
                    cherry_rebase_group(&git2_repo, base.to_git2(), &[oid.to_git2()], true)?
                        .to_gix();
                if let Some(new_message) = new_message {
                    new_head = reword_commit(repo, new_head, new_message.clone())?;
                }
                // Update the base for the next loop iteration
                head = new_head;
            }
            RebaseStep::Merge { oid, new_message } => {
                head = merge_commits(repo, head, oid, &new_message.to_str_lossy())?;
            }
            RebaseStep::Fixup { oid, new_message } => {
                // This time, the base is the parent of the last commit
                let base_commit = repo.find_commit(head)?;

                // First cherry-pick the target oid on top of base_commit
                let new_head =
                    cherry_rebase_group(&git2_repo, head.to_git2(), &[oid.to_git2()], true)?
                        .to_gix();

                // Now, lets pretend the base didn't exist by swapping parent with the parent of the base
                let commit = repo.find_commit(new_head)?;
                let mut new_commit = commit.decode()?.to_owned();
                new_commit.parents = base_commit.parent_ids().map(|id| id.detach()).collect();
                // Optionally reword the commit
                if let Some(new_message) = new_message {
                    new_commit.message = new_message;
                }
                let new_head = repo.write_object(new_commit)?.detach();
                // Update the base for the next loop iteration
                head = new_head;
            }
            RebaseStep::Reference { refname } => {
                references.push(ReferenceSpec {
                    refname: refname.clone(),
                    oid: head,
                });
            }
        }
    }

    Ok(RebaseOutput {
        new_head: head,
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
    Ok(repo.write_object(new_commit)?.detach())
}

/// A reference that is an output of a rebase operation.
/// This is simply a marker for where the actual reference should point to after the rebase operation.
pub struct ReferenceSpec {
    /// A literal reference, useful only to the caller.
    pub refname: BString,
    /// The commit it now points to.
    pub oid: gix::ObjectId,
}

/// The output of the [rebase](RebaseBuilder::rebase()) operation.
pub struct RebaseOutput {
    /// The oid of the last commit in the rebase operation, i.e. the new head.
    pub new_head: gix::ObjectId,
    /// The list of references that should be created, ordered from the least recent to the most recent.
    pub references: Vec<ReferenceSpec>,
}
