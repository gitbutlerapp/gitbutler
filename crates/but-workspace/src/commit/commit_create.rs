//! An action to create a new commit relative to a commit or reference.

pub(crate) mod function {
    use anyhow::Result;
    use but_core::DiffSpec;
    use but_rebase::graph_rebase::{
        Editor, LookupStep, Pick, Selector, Step, SuccessfulRebase, ToSelector, mutate::InsertSide,
    };

    use crate::commit_engine::{Destination, create_commit};

    /// The result of creating and inserting a new commit in the graph rebase editor.
    #[derive(Debug)]
    pub struct CommitCreateOutcome {
        /// The successful rebase result, if a new commit was created.
        pub rebase: Option<SuccessfulRebase>,
        /// Selector pointing to the newly created commit, if one was created.
        pub commit_selector: Option<Selector>,
        /// Rejected diff specs from commit creation, matching legacy behavior.
        pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
        /// The intermediate tree before cherry-picking back onto the target tree.
        pub changed_tree_pre_cherry_pick: Option<gix::ObjectId>,
    }

    /// Create a commit from `changes` and insert it relative to `relative_to` on `side`.
    pub fn commit_create(
        mut editor: Editor,
        changes: Vec<DiffSpec>,
        relative_to: impl ToSelector,
        side: InsertSide,
        message: &str,
        context_lines: u32,
    ) -> Result<CommitCreateOutcome> {
        let relative_to_selector = relative_to.to_selector(&editor)?;
        let parent_commit_id =
            parent_commit_id_for_new_commit(&editor, editor.lookup_step(relative_to_selector)?, side)?;

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
                rebase: None,
                commit_selector: None,
                rejected_specs: create_out.rejected_specs,
                changed_tree_pre_cherry_pick: create_out.changed_tree_pre_cherry_pick,
            });
        };

        let commit_selector = editor.insert(relative_to_selector, Step::new_pick(new_commit_id), side)?;
        let rebase = editor.rebase()?;

        Ok(CommitCreateOutcome {
            rebase: Some(rebase),
            commit_selector: Some(commit_selector),
            rejected_specs: create_out.rejected_specs,
            changed_tree_pre_cherry_pick: create_out.changed_tree_pre_cherry_pick,
        })
    }

    fn parent_commit_id_for_new_commit(
        editor: &Editor,
        target_step: Step,
        side: InsertSide,
    ) -> Result<Option<gix::ObjectId>> {
        Ok(match (target_step, side) {
            (Step::Pick(Pick { id, .. }), InsertSide::Above) => Some(id),
            (Step::Pick(Pick { id, .. }), InsertSide::Below) => {
                let commit = editor.find_commit(id)?;
                commit.parents.first().copied()
            }
            (Step::Reference { refname }, _) => {
                let mut reference = editor.repo().find_reference(refname.as_ref())?;
                let target_id = reference.peel_to_id()?.detach();
                Some(target_id)
            }
            (Step::None, _) => None,
        })
    }
}
