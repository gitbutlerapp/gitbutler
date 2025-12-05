//! An action to perform a reword of a commit

pub(crate) mod function {
    use anyhow::Result;
    use bstr::BStr;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{GraphExt, Step},
    };

    /// This action will rewrite a commit and any relevant history so it uses
    /// the new name.
    ///
    /// Returns the ID of the newly renamed commit
    pub fn reword(
        graph: &but_graph::Graph,
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        new_message: &BStr,
    ) -> Result<gix::ObjectId> {
        let mut editor = graph.to_editor(repo)?;
        let target_selector = editor.select_commit(commit_id)?;

        let mut commit = editor.find_commit(commit_id)?;
        commit.message = new_message.to_owned();
        let new_id = editor.write_commit(commit, DateMode::CommitterUpdateAuthorKeep)?;

        editor.replace(&target_selector, Step::new_pick(new_id));

        let outcome = editor.rebase()?;
        outcome.materialize()?;

        Ok(new_id)
    }
}
