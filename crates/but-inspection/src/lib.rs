//! Hello there friend!

use anyhow::{Context as _, Result};
use async_openai::{
    config::{Config, OpenAIConfig},
    types::CreateEmbeddingRequest,
};
use bstr::ByteSlice;
use but_core::{TreeStatus, UnifiedDiff};
use gitbutler_command_context::CommandContext;
use serde::Serialize;
use tiktoken_rs::cl100k_base_singleton;
use tracing::{Level, instrument, span};

use crate::db::{Commit, DbHandle, Hunk};

mod db;
mod lance;

#[derive(Serialize, Debug)]
pub struct RepositoryIndexStats {
    /// How many commits we have indexed
    commits_indexed: usize,
    /// How many commits are reachable
    total_commits: usize,
    /// How many reachable commits are indexed
    reachable_commits_indexed: usize,
    /// How many commits are indexed but no longer reachable
    unreachable_commits_indexed: usize,
}
pub async fn index_stats(ctx: &CommandContext) -> Result<RepositoryIndexStats> {
    let db_handle = DbHandle::new(ctx);
    let db = db_handle.read()?;
    let all_commits = commits_to_index(ctx)?;

    let reachable_commits_indexed = db
        .commits
        .iter()
        .filter(|c| all_commits.contains(&c.oid))
        .count();

    Ok(RepositoryIndexStats {
        commits_indexed: db.commits.len(),
        total_commits: all_commits.len(),
        reachable_commits_indexed,
        unreachable_commits_indexed: db.commits.len() - reachable_commits_indexed,
    })
}

#[instrument(skip(ctx))]
pub async fn generate_embeddings(ctx: &CommandContext) -> Result<RepositoryIndexStats> {
    let repo = ctx.gix_repo()?;
    let db_handle = DbHandle::new(ctx);
    let mut commits = commits_to_index(ctx)?;
    let ai = async_openai::Client::new();

    let db = db_handle.read()?;
    commits.retain(|c| !db.commits.iter().any(|o| o.oid == *c));

    for commit in commits {
        generate_embedding(&repo, &db_handle, &ai, commit).await?;
    }

    index_stats(ctx).await
}

#[instrument(skip(repo, db_handle, ai))]
async fn generate_embedding<C: Config>(
    repo: &gix::Repository,
    db_handle: &DbHandle,
    ai: &async_openai::Client<C>,
    oid: gix::ObjectId,
) -> Result<()> {
    let commit = repo.find_commit(oid)?;
    let parent = commit.parent_ids().next().map(gix::Id::detach);

    let (diff, _) = but_core::diff::tree_changes(repo, parent, commit.id)?;

    let mut to_store = vec![];

    for change in diff {
        match change.status {
            TreeStatus::Modification {
                previous_state,
                state,
                ..
            } => {
                let unidiff = but_core::UnifiedDiff::compute(
                    repo,
                    change.path.as_bstr(),
                    change.previous_path(),
                    state,
                    previous_state,
                    3,
                )?;

                match unidiff {
                    UnifiedDiff::Patch { hunks, .. } => {
                        for hunk in hunks {
                            let header = format!(
                                "@@ -{},{} +{},{} @@",
                                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                            );
                            let body = format!("{}\n{}", header, hunk.diff);
                            // Make sure that we don't have too many tokens for the embedding
                            let encoder = cl100k_base_singleton();
                            let body = encoder.decode(
                                encoder
                                    .encode_ordinary(&body)
                                    .into_iter()
                                    .take(8000)
                                    .collect(),
                            )?;
                            let span = span!(Level::INFO, "calling openai");
                            let _enter = span.enter();
                            let embedding = ai
                                .embeddings()
                                .create(CreateEmbeddingRequest {
                                    model: "text-embedding-3-large".into(),
                                    input: body.into(),
                                    ..Default::default()
                                })
                                .await?;
                            to_store.push(Hunk {
                                oid: commit.id,
                                header,
                                path: change.path.clone(),
                                previous_path: change.previous_path().map(ToOwned::to_owned),
                                vector: embedding.data.first().context("Nah")?.embedding.clone(),
                            });
                        }
                    }
                    _ => {
                        // Not right now
                    }
                }
            }
            _ => {
                // Not right now
            }
        }
    }

    db_handle.upsert_many_hunks(&to_store)?;
    db_handle.upsert_many_commits(&[Commit { oid: commit.id }])?;

    println!("Upserted {} hunks", to_store.len());

    Ok(())
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
