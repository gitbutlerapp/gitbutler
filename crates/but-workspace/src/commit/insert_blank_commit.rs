//! Insertion of a blank commit

pub(crate) mod function {
    use anyhow::Result;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{Editor, RelativeTo, Selector, Step, SuccessfulRebase, mutate::InsertSide},
    };

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

        let blank_commit_selector = editor.insert(target_selector, Step::new_pick(new_id), side)?;

        let outcome = editor.rebase()?;

        Ok((outcome, blank_commit_selector))
    }
}
