use but_rebase::RebaseStep;
use gitbutler_command_context::CommandContext;

/// Extension trait for `gitbutler_stack::Stack`.
pub trait StackExt {
    /// Return the stack as a series of rebase steps in the order the steps should be applied.
    fn as_rebase_steps(
        &self,
        ctx: &CommandContext,
        repo: &gix::Repository,
    ) -> anyhow::Result<Vec<RebaseStep>>;
    /// Return the stack as a series of rebase steps in reverse order, i.e. in the order they were generated.
    ///
    /// The generation order starts at the top of the stack (tip first) and goes down to the merge base (parent most commit).
    /// This is useful for operations that need to process the stack in reverse order.
    fn as_rebase_steps_rev(
        &self,
        ctx: &CommandContext,
        repo: &gix::Repository,
    ) -> anyhow::Result<Vec<RebaseStep>>;
}

impl StackExt for gitbutler_stack::Stack {
    fn as_rebase_steps(
        &self,
        ctx: &CommandContext,
        repo: &gix::Repository,
    ) -> anyhow::Result<Vec<RebaseStep>> {
        self.as_rebase_steps_rev(ctx, repo).map(|mut steps| {
            steps.reverse();
            steps
        })
    }

    fn as_rebase_steps_rev(
        &self,
        ctx: &CommandContext,
        repo: &gix::Repository,
    ) -> anyhow::Result<Vec<RebaseStep>> {
        let mut steps: Vec<RebaseStep> = Vec::new();
        for branch in crate::stack_branches(self.id, ctx)? {
            if branch.archived {
                continue;
            }
            let reference_step = if let Some(reference) = repo.try_find_reference(&branch.name)? {
                RebaseStep::Reference(but_core::Reference::Git(reference.name().to_owned()))
            } else {
                RebaseStep::Reference(but_core::Reference::Virtual(branch.name.to_string()))
            };
            steps.push(reference_step);
            let commits = crate::stacks::stack_branch_local_and_remote_commits(
                self.id,
                branch.name.to_string(),
                ctx,
                repo,
            )?;
            for commit in commits {
                let pick_step = RebaseStep::Pick {
                    commit_id: commit.id,
                    new_message: None,
                };
                steps.push(pick_step);
            }
        }
        Ok(steps)
    }
}

/// Extension trait for `but_workspace::ui::StackDetails`.
pub trait StackDetailsExt {
    /// Return the stack as a series of rebase steps in the order the steps should be applied.
    fn as_rebase_steps(&self, repo: &gix::Repository) -> anyhow::Result<Vec<RebaseStep>>;
    /// Return the stack as a series of rebase steps in reverse order, i.e. in the order they were generated.
    ///
    /// The generation order starts at the top of the stack (tip first) and goes down to the merge base (parent most commit).
    /// This is useful for operations that need to process the stack in reverse order.
    fn as_rebase_steps_rev(&self, repo: &gix::Repository) -> anyhow::Result<Vec<RebaseStep>>;
}

impl StackDetailsExt for crate::ui::StackDetails {
    fn as_rebase_steps(&self, repo: &gix::Repository) -> anyhow::Result<Vec<RebaseStep>> {
        self.as_rebase_steps_rev(repo).map(|mut steps| {
            steps.reverse();
            steps
        })
    }

    fn as_rebase_steps_rev(&self, repo: &gix::Repository) -> anyhow::Result<Vec<RebaseStep>> {
        let mut steps: Vec<RebaseStep> = Vec::new();
        for branch in &self.branch_details {
            let reference_step = if let Some(reference) = repo.try_find_reference(&branch.name)? {
                RebaseStep::Reference(but_core::Reference::Git(reference.name().to_owned()))
            } else {
                RebaseStep::Reference(but_core::Reference::Virtual(branch.name.to_string()))
            };
            steps.push(reference_step);
            let commits = &branch.commits;
            for commit in commits {
                let pick_step = RebaseStep::Pick {
                    commit_id: commit.id,
                    new_message: None,
                };
                steps.push(pick_step);
            }
        }
        Ok(steps)
    }
}
