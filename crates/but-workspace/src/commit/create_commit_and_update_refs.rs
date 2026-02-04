pub(crate) mod function {
    use anyhow::Result;
    use but_core::DiffSpec;
    use but_rebase::graph_rebase::{RelativeTo, mutate::InsertSide};

    use crate::commit_engine::{CreateCommitOutcome, create_commit};

    pub fn create_commit_and_update_refs(
        repo: &gix::Repository,
        relative_to: RelativeTo,
        side: InsertSide,
        changes: Vec<DiffSpec>,
        context_lines: u32,
    ) -> Result<CreateCommitOutcome> {
        let mut out = create_commit(repo, destination.clone(), changes.clone(), context_lines)?;
        todo!()
    }
}
