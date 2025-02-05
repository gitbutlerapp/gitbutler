#![deny(missing_docs, rust_2018_idioms)]
//! Rebase for your flying but.

use anyhow::Result;
use bstr::{BStr, BString};

#[derive(Debug)]
/// A step to be taken as part of rebase.
pub enum Step<'a> {
    /// Reference updates the given reference to the commit beneath it.
    ///
    /// Should be a full reference identifer, IE: refs/heads/foo
    Reference(&'a BStr),
    /// Picks a commit and squashes it into the commit beneath it.
    ///
    /// If a name is provided, it will be used as the new commit's title.
    /// Otherwise, the commit's title that is getting picked will be taken.
    ///
    /// TODO: ADD A VALIDATION STEP FOR THIS
    /// Must never be the first operation.
    /// Must never come after a Reference operation.
    Ammend(gix::ObjectId, Option<&'a BStr>),
    /// Picks a commit
    ///
    /// If a name is provided, it will be used as the new commit's title.
    /// Otherwise, the commit's title that is getting picked will be taken.
    Pick(gix::ObjectId, Option<&'a BStr>),
    /// Merges in a commit and it's parents
    ///
    /// The name provided will be used as the merge commit's name
    Merge(gix::ObjectId, &'a BStr),
}

// vec![
//      Step::Pick(fdsaadfs),
//      Step::Pick(fdsaadfs),
//      Step::Ammend(fdsaadfs),
//      Step::Reference("refs/heads/foo"),
// ]

fn but_base(base: gix::ObjectId, steps: &[Step]) -> Result<RebaseOutput> {
    let mut references = vec![];
    let mut commits = vec![];

    for step in steps {
        match step {
            Step::Reference(refname) => references.push(Reference {
                name: refname.to_owned().into(),
                oid: commits.last().unwrap_or(&base).to_owned(),
            }),
            Step::Pick(to_pick, commit_name) => {
                let target_commit = commits.last().unwrap_or(&base).to_owned();
                let result = cherry_rebase_group(todo!(), target_commit, &[to_pick], true)?;
                commits.push(result)
            }
            Step::Ammend(to_pick, commit_name) => {
                let target_commit = commits
                    .last()
                    .expect("Ammend should never be the first operation");
                let result = cherry_rebase_group(todo!(), target_commit, &[to_pick], true)?;
                commits.pop(); // Remove the commit that we ammended
                commits.push(result)
            }
            Step::Merge(to_merge, commit_name) => {
                // Do the same with gitbutler_merge_commits
            }
        }
    }

    Ok(RebaseOutput {
        references,
        commits,
    })
}

/// Describes a reference
pub struct Reference {
    /// The reference identifier, IE: refs/heads/foo
    name: BString,
    /// The oid that the reference points to.
    oid: gix::ObjectId,
}

/// The result of a rebase
pub struct RebaseOutput {
    /// The references, from child-most to parent-most.
    references: Vec<Reference>,
    /// The commits, from child-most to parent-most.
    commits: Vec<gix::ObjectId>,
}
