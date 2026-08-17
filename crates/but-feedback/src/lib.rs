//! Provide utilities for creating archives for letting users provide feedback.
#![deny(missing_docs)]
use std::path::PathBuf;

use anyhow::Result;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};

/// A utility to keep important paths to make archival/zip-file creation easier later.
pub struct Archival {
    /// The directory to put the feedback archive in.
    pub cache_dir: PathBuf,
    /// The directory containing application logs.
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
    pub fn zip_anonymous_graph(&self, ctx: &Context) -> Result<PathBuf> {
        let mut options = but_graph::init::Options {
            worktrees: ctx.settings.feature_flags.worktree_manipulation,
            ..Default::default()
        };
        let repo = ctx.repo.get()?;
        let meta = ctx.meta()?;
        let project_meta = ctx.project_meta()?;
        let mut db = ctx.db.get_cache_mut()?;
        let mut graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta.clone(),
            &mut db,
            options.clone(),
        )
        .or_else(|_| {
            // Assume it fails because of post-processing, try again without.
            options.dangerously_skip_postprocessing_for_debugging = true;
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, options)
        })?;
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
