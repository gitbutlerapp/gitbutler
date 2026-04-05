//! An action to amend an existing commit with selected changes.

pub(crate) mod function {
    use anyhow::{Result, bail};
    use but_core::{DiffSpec, RefMetadata};
    use but_rebase::graph_rebase::{Editor, Selector, Step, SuccessfulRebase, ToCommitSelector};

    use crate::commit_engine::{Destination, create_commit};

    /// The result of amending a commit in the graph rebase editor.
    #[derive(Debug)]
    pub struct CommitAmendOutcome<'ws, 'meta, M: RefMetadata> {
        /// A successful rebase result for continuing operations. This will be
        /// always provided regardless of whether a commit was actually
        /// created.
        pub rebase: SuccessfulRebase<'ws, 'meta, M>,
        /// Selector pointing to the amended commit, if the amend was
        /// successful.
        ///
        /// A commit may not be amended if all the diff_specs are rejected. See
        /// [`create_commit`] for more details.
        pub commit_selector: Option<Selector>,
        /// Rejected diff specs from commit creation. See [`create_commit`] for
        /// more details.
        pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
    }

    /// Amend a commit specified by `commit` selector.
    ///
    /// Similar to other `editor` based functions, this consumes an editor and
    /// gives it back as a [`SuccessfulRebase`] which can be used to chain more
    /// operations or just materialize the result.
    ///
    /// `changes` defines which changes from the worktree should be committed.
    /// See [`create_commit`] for more details.
    ///
    /// `context_lines` define how many diff context lines are being used for
    /// this particular function call. The provided `context_lines` MUST align
    /// with the `context_lines` value used to generate the `DiffSpec`s passed
    /// in the `changes` parameter.
    pub fn commit_amend<'ws, 'meta, M: RefMetadata>(
        mut editor: Editor<'ws, 'meta, M>,
        commit: impl ToCommitSelector,
        changes: Vec<DiffSpec>,
        context_lines: u32,
    ) -> Result<CommitAmendOutcome<'ws, 'meta, M>> {
        let (target_selector, target) = editor.find_selectable_commit(commit)?;

        let target_id = target.id;
        if target.attach(editor.repo()).is_conflicted() {
            bail!("Cannot amend a conflicted commit")
        }

        let create_out = create_commit(
            editor.repo(),
            Destination::AmendCommit {
                commit_id: target_id,
                new_message: None,
            },
            changes,
            context_lines,
        )?;

        let Some(new_commit_id) = create_out.new_commit else {
            return Ok(CommitAmendOutcome {
                rebase: editor.rebase()?,
                commit_selector: None,
                rejected_specs: create_out.rejected_specs,
            });
        };

        editor.replace(target_selector, Step::new_pick(new_commit_id))?;

        Ok(CommitAmendOutcome {
            rebase: editor.rebase()?,
            commit_selector: Some(target_selector),
            rejected_specs: create_out.rejected_specs,
        })
    }
}
