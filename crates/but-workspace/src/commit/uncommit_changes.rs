//! An action to remove changes from a commit

pub(crate) mod function {
    use anyhow::{Result, bail};
    use but_core::DiffSpec;
    use but_rebase::{
        commit::DateMode,
        graph_rebase::{Editor, Selector, Step, SuccessfulRebase, ToCommitSelector},
    };

    use crate::tree_manipulation::{ChangesSource, create_tree_without_diff};

    /// The result of an uncommit_changes operation.
    #[derive(Debug)]
    pub struct UncommitChangesOutcome {
        /// The successful rebase result
        pub rebase: SuccessfulRebase,
        /// Selector pointing to the modified commit (with changes removed)
        pub commit_selector: Selector,
    }

    /// Removes the specified changes from a commit.
    ///
    /// The changes are removed from the commit's tree, effectively "uncommitting"
    /// them so they appear in the working directory as uncommitted changes.
    pub fn uncommit_changes(
        mut editor: Editor,
        commit: impl ToCommitSelector,
        changes: impl IntoIterator<Item = DiffSpec>,
        context_lines: u32,
    ) -> Result<UncommitChangesOutcome> {
        let (commit_selector, commit) = editor.find_selectable_commit(commit)?;

        if commit.is_conflicted() {
            bail!("Cannot uncommit changes from a conflicted commit")
        }

        let (tree_without_changes, dropped_diffs) = create_tree_without_diff(
            editor.repo(),
            ChangesSource::Commit { id: commit.id.into() },
            changes,
            context_lines,
        )?;

        if !dropped_diffs.is_empty() {
            bail!("Failed to remove specified changes from commit");
        }

        let new_commit_id = {
            let mut new_commit = commit.clone();
            new_commit.tree = tree_without_changes;
            editor.new_commit(new_commit, DateMode::CommitterUpdateAuthorKeep)?
        };

        editor.replace(commit_selector, Step::new_pick(new_commit_id))?;

        let rebase = editor.rebase()?;

        Ok(UncommitChangesOutcome {
            rebase,
            commit_selector,
        })
    }
}
