use anyhow::{Context as _, Result};
use but_ctx::Context;
use gitbutler_repo::first_parent_commit_ids_until;
use gitbutler_stack::Stack;

pub trait RepoActionsExt {
    fn distance(&self, from: gix::ObjectId, to: gix::ObjectId) -> Result<u32>;
    fn delete_branch_reference(&self, stack: &Stack) -> Result<()>;
    fn add_branch_reference(&self, stack: &Stack) -> Result<()>;
}

impl RepoActionsExt for Context {
    fn add_branch_reference(&self, stack: &Stack) -> Result<()> {
        let repo = self.repo.get()?;
        let refname = stack.refname()?.to_string();
        let head_oid = stack.head_oid(self)?;
        let previous = match repo
            .try_find_reference(&refname)
            .context("failed to lookup reference")?
        {
            Some(reference) => {
                if reference.id() == head_oid {
                    return Ok(());
                }
                gix::refs::transaction::PreviousValue::Any
            }
            None => gix::refs::transaction::PreviousValue::MustNotExist,
        };

        let refname: gix::refs::FullName = refname.as_str().try_into()?;
        repo.reference(refname, head_oid, previous, "new vbranch")
            .context("failed to create branch reference")?;

        Ok(())
    }

    fn delete_branch_reference(&self, stack: &Stack) -> Result<()> {
        let repo = self.repo.get()?;
        match repo
            .try_find_reference(&stack.refname()?.to_string())
            .context("failed to lookup reference")?
        {
            Some(reference) => reference
                .delete()
                .context("failed to delete branch reference"),
            None => Ok(()),
        }
    }

    // returns the number of commits between the first oid to the second oid
    fn distance(&self, from: gix::ObjectId, to: gix::ObjectId) -> Result<u32> {
        let repo = self.repo.get()?;
        let oids = first_parent_commit_ids_until(&repo, from, to)?;
        Ok(oids.len().try_into()?)
    }
}
