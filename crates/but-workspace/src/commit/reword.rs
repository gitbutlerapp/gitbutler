//! An action to perform a reword of a commit

pub(crate) mod function {
    use anyhow::Result;
    use bstr::BStr;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{Editor, Selector, Step, SuccessfulRebase, ToCommitSelector},
    };

    /// This action will rewrite a commit and any relevant history so it uses
    /// the new name.
    ///
    /// Returns a selector to the rewritten commit
    pub fn reword(
        mut editor: Editor,
        commit: impl ToCommitSelector,
        new_message: &BStr,
    ) -> Result<(SuccessfulRebase, Selector)> {
        let (target_selector, mut commit) = editor.find_selectable_commit(commit)?;

        commit.message = new_message.to_owned();
        let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?;

        editor.replace(target_selector, Step::new_pick(new_id))?;

        let outcome = editor.rebase()?;

        Ok((outcome, target_selector))
    }
}
