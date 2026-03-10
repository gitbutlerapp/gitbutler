//! An action to sign existing commits, even if they are otherwise unchanged in the workspace.

pub(crate) mod function {
    use anyhow::Result;
    use but_rebase::graph_rebase::{
        Editor, Step, SuccessfulRebase, ToCommitSelector, cherry_pick::PickSignMode,
    };

    /// The result of signing a set of commits in the graph rebase editor.
    #[derive(Debug)]
    pub struct CommitsSignOutcome {
        /// A successful rebase result for continuing operations. This will be
        /// always provided regardless of whether a commit was actually
        /// created.
        pub rebase: SuccessfulRebase,
    }

    /// Sign all commits specified by `commits`.
    ///
    /// Similar to other `editor` based functions, this consumes an editor and
    /// gives it back as a [`SuccessfulRebase`] which can be used to chain more
    /// operations or just materialize the result.
    pub fn commits_sign(
        mut editor: Editor,
        commits: impl IntoIterator<Item = impl ToCommitSelector>,
    ) -> Result<CommitsSignOutcome> {
        for commit in commits {
            let (target_selector, target) = editor.find_selectable_commit(commit)?;
            let pick = Step::new_pick_with_sign_mode(target.id, PickSignMode::Force);
            editor.replace(target_selector, pick)?;
        }

        let rebase = editor.rebase()?;

        Ok(CommitsSignOutcome { rebase })
    }
}
