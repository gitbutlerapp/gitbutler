//! An action to perform a reword of a commit

pub(crate) mod function {
    use anyhow::Result;
    use bstr::BStr;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{Editor, Selector, Step, rebase::SuccessfulRebase},
    };

    /// This action will rewrite a commit and any relevant history so it uses
    /// the new name.
    ///
    /// Returns a selector to the rewritten commit
    pub fn reword(
        mut editor: Editor,
        commit_id: gix::ObjectId,
        new_message: &BStr,
    ) -> Result<(SuccessfulRebase, Selector)> {
        let target_selector = editor.select_commit(commit_id)?;

        let mut commit = editor.find_commit(commit_id)?;
        commit.message = new_message.to_owned();
        let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?;

        editor.replace(target_selector, Step::new_pick(new_id));

        let outcome = editor.rebase()?;

        Ok((outcome, target_selector))
    }
}
