//! An action to create a new commit relative to a commit or reference.

pub(crate) mod function {
    use anyhow::Result;
    use but_core::{DiffSpec, RefMetadata};
    use but_rebase::graph_rebase::{
        Editor, LookupStep, Pick, PickDivergent, Selector, Step, SuccessfulRebase, ToSelector,
        mutate::InsertSide,
    };

    use crate::commit_engine::{Destination, create_commit};

    /// The result of creating and inserting a new commit in the graph rebase editor.
    #[derive(Debug)]
    pub struct CommitCreateOutcome<'ws, 'meta, M: RefMetadata> {
        /// A successful rebase result for continuing operations. This will be
        /// always provided regardless of whether a commit was actually
        /// created.
        pub rebase: SuccessfulRebase<'ws, 'meta, M>,
        /// Selector pointing to the newly created commit, if one was created.
        ///
        /// A commit may not be created if all the diff_specs are rejected. See
        /// [`create_commit`] for more details.
        pub commit_selector: Option<Selector>,
        /// Rejected diff specs from commit creation. See [`create_commit`] for
        /// more details.
        pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
    }

    /// Create a commit from `changes` and insert it relative to `relative_to` on `side`.
    ///
    /// Similar to other `editor` based functions, this consumes an editor and
    /// gives it back as a [`SuccessfulRebase`] which can be used to chain more
    /// operations or just materialize the result.
    ///
    /// `changes` defines which changes from the worktree should be committed.
    /// See [`create_commit`] for more details.
    ///
    /// `relative_to` and `side` determine the position to insert the commit.
    /// See [`InsertSide`] to learn more about insertion semantics.
    ///
    /// `message` will be the message used for the newly created commit.
    ///
    /// `context_lines` define how many diff context lines are being used for
    /// this particular function call. The provided `context_lines` MUST align
    /// with the `context_lines` value used to generate the `DiffSpec`s passed
    /// in the `changes` parameter.
    pub fn commit_create<'ws, 'meta, M: RefMetadata>(
        mut editor: Editor<'ws, 'meta, M>,
        changes: Vec<DiffSpec>,
        relative_to: impl ToSelector,
        side: InsertSide,
        message: &str,
        context_lines: u32,
    ) -> Result<CommitCreateOutcome<'ws, 'meta, M>> {
        let relative_to_selector = relative_to.to_selector(&editor)?;
        let parent_commit_id = parent_commit_id_for_new_commit(
            &editor,
            editor.lookup_step(relative_to_selector)?,
            side,
        )?;

        let create_out = create_commit(
            editor.repo(),
            Destination::NewCommit {
                parent_commit_id,
                stack_segment: None,
                message: message.to_owned(),
            },
            changes,
            context_lines,
        )?;

        let Some(new_commit_id) = create_out.new_commit else {
            return Ok(CommitCreateOutcome {
                rebase: editor.rebase()?,
                commit_selector: None,
                rejected_specs: create_out.rejected_specs,
            });
        };

        let commit_selector = editor.insert(
            relative_to_selector,
            Step::new_untracked_pick(new_commit_id),
            side,
        )?;

        Ok(CommitCreateOutcome {
            rebase: editor.rebase()?,
            commit_selector: Some(commit_selector),
            rejected_specs: create_out.rejected_specs,
        })
    }

    fn parent_commit_id_for_new_commit<'ws, 'meta, M: RefMetadata>(
        editor: &Editor<'ws, 'meta, M>,
        target_step: Step,
        side: InsertSide,
    ) -> Result<Option<gix::ObjectId>> {
        Ok(match (target_step, side) {
            (
                Step::Pick(Pick { id, .. })
                | Step::PickDivergent(PickDivergent {
                    remote_commit: id, ..
                }),
                InsertSide::Above,
            ) => Some(id),
            (
                Step::Pick(Pick { id, .. })
                | Step::PickDivergent(PickDivergent {
                    remote_commit: id, ..
                }),
                InsertSide::Below,
            ) => {
                let commit = editor.find_commit(id)?;
                commit.parents.first().copied()
            }
            (Step::Reference { refname }, _) => Some(editor.find_reference_target(refname)?.1.id),
            (Step::None, _) => None,
        })
    }
}
