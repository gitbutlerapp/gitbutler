//! Provide utilities for creating archives for letting users provide feedback.
use std::path::PathBuf;

use anyhow::Result;
use but_core::sync::RepoShared;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};

pub struct Archival {
    /// The directory to put the feedback archive in.
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
}

/// Create timestamps like `2025-08-20T14-31-22`, which are safe even for Windows.
fn filesafe_date_time() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

impl Archival {
    /// Create an archive of the entire repository behind `project_id`.
    pub fn zip_entire_repository(
        &self,
        project_id: ProjectHandleOrLegacyProjectId,
    ) -> Result<PathBuf> {
        let ctx: Context = project_id.try_into()?;
        let output_file = self
            .cache_dir
            .join(format!("project-{date}.zip", date = filesafe_date_time()));
        create_zip_file_from_dir(ctx.workdir_or_gitdir()?, output_file)
    }

    /// Create an anonymous archive commit graph for `ctx`, such that it doesn't reveal PII.
    ///
    /// It's the graph the rebase editor is built from. If the workspace can't be built, it's
    /// walked without one from `HEAD` and the linked worktrees' `HEAD`s.
    pub fn zip_anonymous_graph(&self, ctx: &Context, perm: &RepoShared) -> Result<PathBuf> {
        let meta = ctx.meta()?;
        let editor_graph = ctx
            .workspace_and_db_with_perm(perm)
            .and_then(|(repo, ws, _db)| {
                let mut heads = Vec::from_iter(ws.graph.entrypoint()?.commit().map(|c| c.id));
                heads.extend(ws.graph.worktree_tips.iter().map(|tip| tip.id));
                but_graph::nosegment::workspace_segment_graph(&ws, &repo, &meta, heads)
            });
        let mut graph = match editor_graph {
            Ok(graph) => graph,
            Err(_) => {
                // Assume it fails because of post-processing, and walk without a workspace.
                let mut worktree_heads = Vec::new();
                for worktree in ctx.worktrees_with_state()? {
                    if !worktree.archived
                        && let Some(head) = ctx.worktree_head(worktree.name.as_ref())?
                    {
                        worktree_heads.push(head.id);
                    }
                }
                let repo = ctx.repo.get()?;
                let project_meta = ctx.project_meta()?;
                let head_ref = repo.head_name()?;
                let head_id = repo.head_id().ok().map(|id| id.detach());
                // An unavailable target commit bounds nothing, instead of failing the walk.
                let target_commit_id = project_meta
                    .resolve_target_commit(&repo)
                    .ok()
                    .filter(|id| repo.find_commit(*id).is_ok());
                let mut heads = Vec::from_iter(head_id);
                heads.extend(worktree_heads);
                but_graph::nosegment::segment_graph(
                    &repo,
                    &meta,
                    but_graph::init::Overlay::default(),
                    project_meta,
                    head_ref.as_ref().map(|name| name.as_ref()),
                    head_id,
                    heads,
                    Vec::new(),
                    target_commit_id,
                    false,
                    None,
                )?
            }
        };
        let repo = ctx.repo.get()?;
        let dot_file_contents = graph.anonymize(&repo.remote_names())?.dot_graph_pruned();
        let output_file = self.cache_dir.join(format!(
            "commit-graph-anon-{date}.zip",
            date = filesafe_date_time()
        ));
        create_zip_file_from_content(&dot_file_contents, "anon-graph.dot", output_file)
    }

    /// Create an archive of all logs in the application log directory.
    pub fn zip_logs(&self) -> Result<PathBuf> {
        let output_file = self
            .cache_dir
            .join(format!("logs-{date}.zip", date = filesafe_date_time()));
        create_zip_file_from_dir(&self.logs_dir, output_file)
    }
}

mod zip;
pub use zip::{create_zip_file_from_content, create_zip_file_from_dir};
