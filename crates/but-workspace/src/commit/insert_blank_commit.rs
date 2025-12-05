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
        graph_rebase::{GraphExt, Step, mutate::InsertSide},
    };

    use crate::commit::insert_blank_commit::{InsertCommitOutcome, RelativeTo};

    /// Inserts a blank commit relative to either a reference or a commit
    pub fn insert_blank_commit(
        graph: &but_graph::Graph,
        repo: &gix::Repository,
        side: InsertSide,
        relative_to: RelativeTo,
    ) -> Result<InsertCommitOutcome> {
        let mut editor = graph.to_editor(repo)?;
        let target_selector = match relative_to {
            RelativeTo::Commit(id) => editor.select_commit(id)?,
            RelativeTo::Reference(r) => editor.select_reference(r)?,
        };

        let commit = editor.empty_commit()?;
        let new_id = editor.write_commit(commit, DateMode::CommitterUpdateAuthorUpdate)?;

        editor.insert(&target_selector, Step::new_pick(new_id), side);

        let outcome = editor.rebase()?;
        let mat_output = outcome.materialize()?;

        Ok(InsertCommitOutcome {
            blank_commit_id: new_id,
            commit_mapping: mat_output.commit_mapping,
        })
    }
}
