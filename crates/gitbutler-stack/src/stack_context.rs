use anyhow::Result;
use gitbutler_command_context::CommandContext;

use crate::{Target, VirtualBranchesHandle};

pub struct StackContext<'repositroy> {
    repository: &'repositroy git2::Repository,
    target: Target,
}

impl<'repository> StackContext<'repository> {
    pub fn new(repository: &'repository git2::Repository, target: Target) -> Self {
        Self { repository, target }
    }

    pub fn repository(&self) -> &'repository git2::Repository {
        self.repository
    }

    pub fn target(&self) -> &Target {
        &self.target
    }
}

pub trait CommandContextExt {
    fn to_stack_context(&self) -> Result<StackContext>;
}

impl CommandContextExt for CommandContext {
    fn to_stack_context(&self) -> Result<StackContext> {
        let virtual_branch_state = VirtualBranchesHandle::new(self.project().gb_dir());
        let default_target = virtual_branch_state.get_default_target()?;

        Ok(StackContext::new(self.repository(), default_target))
    }
}
