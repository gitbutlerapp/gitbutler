//! Discard a commit from the graph.

pub(crate) mod function {
    use anyhow::bail;
    use but_core::RefMetadata;
    use but_rebase::graph_rebase::{
        Editor, Step, SuccessfulRebase, ToCommitSelector,
        mutate::{SegmentDelimiter, SelectorSet},
    };

    /// Discard a commit by removing it from history and reconnecting all its
    /// parents to all of its children.
    ///
    /// `subject_commit` - The selector of the commit to discard.
    ///
    /// Returns the rebase result.
    pub fn discard_commit<'ws, 'meta, M: RefMetadata>(
        mut editor: Editor<'ws, 'meta, M>,
        subject_commit: impl ToCommitSelector,
    ) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
        let (subject_commit_selector, subject_commit) =
            editor.find_selectable_commit(subject_commit)?;

        if subject_commit.clone().attach(editor.repo()).is_conflicted() {
            bail!("Cannot discard a conflicted commit")
        }

        let commit_delimiter = SegmentDelimiter {
            child: subject_commit_selector,
            parent: subject_commit_selector,
        };

        editor.disconnect_segment_from(
            commit_delimiter,
            SelectorSet::All,
            SelectorSet::All,
            false,
        )?;

        editor.replace(subject_commit_selector, Step::None)?;
        editor.rebase()
    }
}
