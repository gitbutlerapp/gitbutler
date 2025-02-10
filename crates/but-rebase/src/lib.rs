#![deny(rust_2018_idioms)]

use anyhow::{anyhow, bail, Ok, Result};
use bstr::BString;
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_repo::rebase::cherry_rebase_group;
use gix::validate;
use smallvec::SmallVec;

#[derive(Debug)]
pub enum RebaseStep {
    /// Pick an existing commit and optionally reword it
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
    Reference { refname: BString },
}

#[derive(Debug)]
pub struct RebaseBuilder {
    repo: gix::Repository,
    base: gix::ObjectId,
    steps: Vec<RebaseStep>,
}

impl RebaseBuilder {
    /// Creates a new rebase builder with the provided commit as a base.
    /// The commit must already exist in the odb.
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

    /// Pick, Merge and Fixup operations:
    /// - The commit must already exist in the repository
    /// - The commit must not be the base commit
    ///
    /// Pick and Merge operations:
    /// - The commit must not be a commit that is already in a pick, merge or fixup step
    ///
    /// Fixup operations:
    /// - The must be a Pick or Merge step prior to this step (not necessarily immediately before)
    ///
    /// Reference operations:
    /// - The refname must be a valid reference name
    fn validate_step(&self, step: &RebaseStep) -> Result<()> {
        match step {
            RebaseStep::Pick {
                oid,
                new_message: _,
            } => {
                self.repo.find_commit(*oid)?;
                if *oid == self.base {
                    bail!("Picked commit cannot be the base commit");
                }
                if self.steps.iter().any(|s| {
                    matches!(
                        s,
                        RebaseStep::Pick { oid: o, .. } |
                        RebaseStep::Merge { oid: o, .. } |
                        RebaseStep::Fixup { oid: o, .. }
                        if o == oid
                    )
                }) {
                    bail!("Picked commit already exists in a previous step");
                }
            }
            RebaseStep::Merge {
                oid,
                new_message: _,
            } => {
                self.repo.find_commit(*oid)?;
                if *oid == self.base {
                    bail!("Merge commit cannot be the base commit");
                }
                if self.steps.iter().any(|s| {
                    matches!(
                        s,
                        RebaseStep::Pick { oid: o, .. } |
                        RebaseStep::Merge { oid: o, .. } |
                        RebaseStep::Fixup { oid: o, .. }
                        if o == oid
                    )
                }) {
                    bail!("Picked commit already exists in a previous step");
                }
            }
            RebaseStep::Fixup {
                oid,
                new_message: _,
            } => {
                self.repo.find_commit(*oid)?;
                if *oid == self.base {
                    bail!("Fixup commit cannot be the base commit");
                }
                if self.steps.iter().any(|s| {
                    matches!(
                        s,
                        RebaseStep::Pick { oid: o, .. } |
                        RebaseStep::Merge { oid: o, .. } |
                        RebaseStep::Fixup { oid: o, .. }
                        if o == oid
                    )
                }) {
                    bail!("Picked commit already exists in a previous step");
                }
                if self
                    .steps
                    .iter()
                    .all(|s| !matches!(s, RebaseStep::Pick { .. } | RebaseStep::Merge { .. }))
                {
                    bail!("Fixup commit must have a Pick or Merge step preceding it");
                }
            }
            RebaseStep::Reference { refname } => {
                if refname.is_empty() {
                    return Err(anyhow!("Reference step must have a non-empty refname"));
                }
                validate::reference::name(refname.as_ref())?;
            }
        }
        Ok(())
    }

    /// Consumes the builder and returns a `Rebase` object.
    ///
    /// The list of steps must not be empty.
    pub fn build(self) -> Result<Rebase> {
        if self.steps.is_empty() {
            return Err(anyhow!("No rebase steps provided"));
        }
        Ok(Rebase {
            repo: self.repo,
            base: self.base,
            steps: self.steps,
        })
    }
}

#[derive(Debug)]
pub struct Rebase {
    repo: gix::Repository,
    base: gix::ObjectId,
    /// The first step will be the first commit in the rebase and the last step will be the last commit.
    steps: Vec<RebaseStep>,
}

impl Rebase {
    /// Performs a rebase on top of a given base, according to the provided steps.
    /// It does not actually create new git references nor does it update existing ones.
    pub fn rebase(&self) -> Result<RebaseOutput> {
        let repo = git2::Repository::open(self.repo.path())?;
        let mut references = vec![];
        // Start with the base commit
        let mut base = self.base.to_git2();
        // Running cherry_rebase_group for each step individually
        for step in self.steps {
            match step {
                RebaseStep::Pick { oid, new_message } => {
                    let mut new_head = cherry_rebase_group(
                        &repo,
                        self.base.to_git2(),
                        &vec![oid.to_git2()],
                        true,
                    )?;
                    if let Some(new_message) = new_message {
                        new_head = self.reword_commit(new_head.into(), new_message)?.to_git2();
                    }
                    // Update the base for the next loop iteration
                    base = new_head;
                }
                RebaseStep::Merge { oid, new_message } => {
                    // TODO:
                    // - find the merge base between the two legs (current head of the rebase and the other commit
                    // - do 3 way merge between the merge base as base, and the two commits being the left and the right.
                    // The parents of the new commit will be the left and the right of the new commit
                    // This is just like gitbutler_merge_commits, but taking commits instead of branches

                    todo!();

                    // Update the base for the next loop iteration
                    // base = new_head;
                }
                RebaseStep::Fixup { oid, new_message } => {
                    // This time, the base is the parent of the last commit
                    let base_commit = self.repo.find_commit(base.into())?;

                    // First cherry-pick the target oid on top of base_commit
                    let new_head = cherry_rebase_group(&repo, base, &vec![oid.to_git2()], true)?;

                    // Now, lets pretend the base didn't exist by swapping parent with the parent of the base
                    let commit = self.repo.find_commit(new_head.to_gix())?;
                    let mut new_commit: gix::objs::Commit = commit.decode().into();
                    new_commit.parents = SmallVec::from_slice(&[base_commit.parent_ids()]);
                    let mut new_head = self.repo.write_object(new_commit)?.detach().to_git2();

                    // Optionally reword the commit
                    if let Some(new_message) = new_message {
                        new_head = self.reword_commit(new_head.into(), new_message)?.to_git2();
                    }
                    // Update the base for the next loop iteration
                    base = new_head;
                }
                RebaseStep::Reference { refname } => {
                    references.push(ReferenceSpec {
                        refname,
                        oid: base.to_gix(),
                    });
                }
            }
        }

        Ok(RebaseOutput {
            new_head: base.to_gix(), // the base from the last iteration is the tip / head
            references,
        })
    }
    fn reword_commit(&self, oid: gix::ObjectId, new_message: BString) -> Result<gix::ObjectId> {
        let commit = self.repo.find_commit(oid)?;
        let mut new_commit: gix::objs::Commit = commit.decode().into();
        new_commit.message = new_message;
        // TODO: maybe update timestamps?
        Ok(self.repo.write_object(new_commit)?.detach())
    }
}

/// A reference that is an output of a rebase operation. This is simply a marker for where the actual reference should point to after the rebase operation.
pub struct ReferenceSpec {
    pub refname: BString,
    pub oid: gix::ObjectId,
}

pub struct RebaseOutput {
    /// The oid of the last commit in the rebase operation, i.e. the new head.
    pub new_head: gix::ObjectId,
    /// The list of references that should be created, ordered from the least recent to the most recent.
    pub references: Vec<ReferenceSpec>,
}
