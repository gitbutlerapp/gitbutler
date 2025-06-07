//! Hello there friend!

use anyhow::Result;
use gitbutler_command_context::CommandContext;
use serde::Serialize;

use crate::db::DbHandle;

mod db;

#[derive(Serialize, Debug)]
pub struct RepositoryIndexStats {
    /// How many commits we have indexed
    commits_indexed: usize,
    /// How many commits are reachable
    total_commits: usize,
    /// How many reachable commits are indexed
    reachable_commits_indexed: usize,
}
pub async fn index_stats(ctx: &CommandContext) -> Result<RepositoryIndexStats> {
    let db_handle = DbHandle::new(ctx);
    let db = db_handle.read()?;
    let all_commits = commits_to_index(ctx)?;

    let reachable_commits_indexed = db
        .commits
        .iter()
        .filter(|c| all_commits.contains(&c.sha))
        .count();

    Ok(RepositoryIndexStats {
        commits_indexed: db.commits.len(),
        total_commits: all_commits.len(),
        reachable_commits_indexed,
    })
}

#[derive(Serialize, Debug)]
pub struct EmbeddingsResult {}
pub async fn generate_embeddings() -> Result<EmbeddingsResult> {
    todo!()
}

/// Lists all commits referenced by references
fn commits_to_index(ctx: &CommandContext) -> Result<Vec<gix::ObjectId>> {
    let repo = ctx.gix_repo()?;

    let all_references = repo
        .references()?
        .all()?
        .map(|r| {
            let id = r.map_err(|_| anyhow::anyhow!("Ahh"))?.try_id();
            if let Some(id) = id {
                if id.object()?.kind == gix::object::Kind::Commit {
                    Ok(Some(id.detach()))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<gix::ObjectId>>>()?;

    let commits = repo.rev_walk(all_references).all()?;

    commits
        .into_iter()
        .map(|c| Ok(c?.id))
        .collect::<Result<Vec<_>>>()
}
