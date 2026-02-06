//! Insertion of a blank commit

pub(crate) mod function {
    use anyhow::Result;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{Editor, Selector, Step, SuccessfulRebase, ToSelector, mutate::InsertSide},
    };

    /// Inserts a blank commit relative to either a reference or a commit
    pub fn insert_blank_commit(
        mut editor: Editor,
        side: InsertSide,
        relative_to: impl ToSelector,
    ) -> Result<(SuccessfulRebase, Selector)> {
        let commit = editor.empty_commit()?;
        let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorUpdate)?;

        let blank_commit_selector = editor.insert(relative_to, Step::new_pick(new_id), side)?;

        let outcome = editor.rebase()?;

        Ok((outcome, blank_commit_selector))
    }
}
