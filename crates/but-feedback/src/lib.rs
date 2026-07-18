//! Provide utilities for creating archives for letting users provide feedback.
#![deny(missing_docs)]
use std::{fmt::Write as _, path::PathBuf};

use anyhow::Result;
use but_core::RefMetadata;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use but_graph::{Graph, NodeGraphEntrypoint, NodeKind, ReferenceMetadata};

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

    /// Create an anonymous archive commit graph for `repo` and `meta`, such that it doesn't reveal PII.
    pub fn zip_anonymous_graph(
        &self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
    ) -> Result<PathBuf> {
        let project_meta = but_core::ref_metadata::ProjectMeta::resolve(repo, meta)?;
        let options = but_graph::init::Options::default().with_hard_limit(5000);
        let graph =
            Graph::from_head(repo, meta, project_meta.clone(), options.clone()).or_else(|_| {
                Graph::from_head(
                    repo,
                    meta,
                    project_meta,
                    but_graph::init::Options {
                        // Preserve the diagnostic even if reference placement fails.
                        dangerously_skip_postprocessing_for_debugging: true,
                        ..options
                    },
                )
            })?;
        let dot_file_contents = anonymous_dot(&graph);
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

fn anonymous_dot(graph: &Graph) -> String {
    let mut out = String::from("digraph {\n  node [shape=box, fontname=Courier];\n");
    for (index, node) in graph.nodes().iter().enumerate() {
        let entrypoint = matches!(
            graph.entrypoint(),
            NodeGraphEntrypoint::Node(entrypoint) if *entrypoint == index
        );
        let label = match node.kind() {
            NodeKind::Commit { id } => {
                let flags = graph.annotations()[index].debug_string(None);
                let flags = if flags.is_empty() {
                    String::new()
                } else {
                    format!(" ({flags})")
                };
                format!(
                    "{}{}{flags}",
                    if entrypoint { "👉" } else { "" },
                    id.to_hex_with_len(7),
                )
            }
            NodeKind::Reference(reference) => format!(
                "{}{}{kind}-{index}{remote}",
                if entrypoint { "👉" } else { "" },
                match reference.metadata {
                    Some(ReferenceMetadata::Workspace(_)) => "📕",
                    Some(ReferenceMetadata::Branch(_)) => "📙",
                    None => "",
                },
                kind = match reference.ref_info.ref_name.category() {
                    Some(gix::refs::Category::LocalBranch) => "local",
                    Some(gix::refs::Category::RemoteBranch) => "remote",
                    Some(gix::refs::Category::Tag) => "tag",
                    _ => "ref",
                },
                remote = reference
                    .remote_tracking_ref_name
                    .as_ref()
                    .map(|_| " <> tracking")
                    .unwrap_or_default(),
            ),
            NodeKind::ShallowPoint { id, reason } => format!(
                "{}{} shallow",
                reason.debug_string(graph.hard_limit_hit()),
                id.to_hex_with_len(7),
            ),
        };
        writeln!(out, "  {index} [label=\"{label}\"];").expect("writing to a string cannot fail");
        for (order, parent) in node.parents().iter().enumerate() {
            writeln!(out, "  {index} -> {parent} [label=\"{order}\"];")
                .expect("writing to a string cannot fail");
        }
    }
    if graph.nodes().is_empty() && matches!(graph.entrypoint(), NodeGraphEntrypoint::Unborn(_)) {
        writeln!(out, "  unborn [label=\"👉unborn-ref\"];")
            .expect("writing to a string cannot fail");
    }
    out.push_str("}\n");
    out
}

mod zip;
pub use zip::{create_zip_file_from_content, create_zip_file_from_dir};
