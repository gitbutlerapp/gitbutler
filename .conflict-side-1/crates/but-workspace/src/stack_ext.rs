use but_rebase::RebaseStep;
use gitbutler_command_context::CommandContext;

/// Extension trait for `gitbutler_stack::Stack`.
pub trait StackExt {
    /// Return the stack as a series of rebase steps.
    fn as_rebase_steps(
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
        let mut steps: Vec<RebaseStep> = Vec::new();
        for branch in crate::stack_branches(self.id.to_string(), ctx)? {
            if branch.archived {
                continue;
            }
            let reference_step = if let Some(reference) = repo.try_find_reference(&branch.name)? {
                RebaseStep::Reference(but_core::Reference::Git(reference.name().to_owned()))
            } else {
                RebaseStep::Reference(but_core::Reference::Virtual(branch.name.to_string()))
            };
            steps.push(reference_step);
            let commits = crate::stack_branch_local_and_remote_commits(
                self.id.to_string(),
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
        steps.reverse();
        Ok(steps)
    }
}
