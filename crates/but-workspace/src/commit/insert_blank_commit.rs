//! Insertion of a blank commit

use std::collections::HashMap;

/// Describes where the blank commit should be inserted relative to.
#[derive(Debug, Clone)]
pub enum RelativeTo<'a> {
    /// Relative to a commit
    Commit(gix::ObjectId),
    /// Relative to a reference
    Reference(&'a gix::refs::FullNameRef),
}

/// Describes the outcome of the rebase
#[derive(Debug, Clone)]
pub struct InsertCommitOutcome {
    /// The blank commit that was creatd
    pub blank_commit_id: gix::ObjectId,
    /// Any commits that were mapped.
    pub commit_mapping: HashMap<gix::ObjectId, gix::ObjectId>,
}

pub(crate) mod function {
    use anyhow::Result;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{Editor, Selector, Step, mutate::InsertSide, rebase::SuccessfulRebase},
    };

    use crate::commit::insert_blank_commit::RelativeTo;

    /// Inserts a blank commit relative to either a reference or a commit
    pub fn insert_blank_commit(
        mut editor: Editor,
        side: InsertSide,
        relative_to: RelativeTo,
    ) -> Result<(SuccessfulRebase, Selector)> {
        let target_selector = match relative_to {
            RelativeTo::Commit(id) => editor.select_commit(id)?,
            RelativeTo::Reference(r) => editor.select_reference(r)?,
        };

        let commit = editor.empty_commit()?;
        let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorUpdate)?;

        let blank_commit_selector = editor.insert(target_selector, Step::new_pick(new_id), side);

        let outcome = editor.rebase()?;

        Ok((outcome, blank_commit_selector))
    }
}
