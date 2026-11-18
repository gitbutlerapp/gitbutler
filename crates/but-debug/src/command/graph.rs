use std::io;

use anyhow::{Context as _, Result};
use gix::odb::store::RefreshMode;

use crate::{
    args::{Args, GraphArgs},
    metadata::EmptyRefMetadata,
    setup,
};

/// How graph output should be emitted after the workspace is computed.
#[derive(Debug, Clone, Copy)]
enum DotMode {
    /// Write the DOT representation to stdout.
    Print,
    /// Open the DOT representation as SVG.
    OpenAsSvg,
    Debug,
}

pub(crate) fn run(
    args: &Args,
    graph_args: &GraphArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    if uses_context_discovery(graph_args) {
        let ctx = but_ctx::Context::discover(&args.current_dir).with_context(|| {
            format!(
                "Could not open GitButler context at '{}'",
                args.current_dir.display()
            )
        })?;
        let (_guard, _repo, workspace, _db) = ctx.workspace_and_db()?;
        return emit_workspace(&workspace, graph_args, out, err);
    }

    let mut repo = setup::repo_from_args(args)?;
    repo.objects.refresh = RefreshMode::Never;
    let (seen_from_ref, seen_from_commit) = match graph_args.ref_name.as_deref() {
        None => {
            let head = repo.head()?;
            (
                head.referent_name().map(ToOwned::to_owned),
                head.id().map(|id| id.detach()),
            )
        }
        Some(ref_name) => {
            let mut reference = repo.find_reference(ref_name)?;
            let id = reference.peel_to_id()?.detach();
            (Some(reference.name().to_owned()), Some(id))
        }
    };
    let workspace = but_graph::nosegment::workspace(
        &repo,
        &EmptyRefMetadata,
        but_graph::init::Overlay::default(),
        but_core::ref_metadata::ProjectMeta::default(),
        seen_from_ref.as_ref().map(|name| name.as_ref()),
        seen_from_commit,
        [],
        Vec::new(),
        true,
        graph_args.limit.flatten(),
    )?;
    emit_workspace(&workspace, graph_args, out, err)
}

fn emit_workspace(
    workspace: &but_graph::Workspace,
    graph_args: &GraphArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let errors = workspace.graph.validation_errors();
    if !errors.is_empty() {
        writeln!(err, "VALIDATION FAILED: {errors:?}")?;
    }
    if graph_args.stats {
        writeln!(err, "{:#?}", workspace.graph.statistics())?;
    }

    if graph_args.no_debug_workspace {
        writeln!(
            err,
            "Workspace with {} stacks and {} segments across all stacks with {} commits total",
            workspace.stacks.len(),
            workspace
                .stacks
                .iter()
                .map(|stack| stack.segments.len())
                .sum::<usize>(),
            workspace
                .stacks
                .iter()
                .flat_map(|stack| stack.segments.iter().map(|segment| segment.commits.len()))
                .sum::<usize>(),
        )?;
    } else {
        writeln!(err, "{workspace:#?}")?;
    }

    match dot_mode(graph_args) {
        Some(DotMode::Print) => {
            out.write_all(workspace.graph.dot_graph_pruned().as_bytes())?;
        }
        Some(DotMode::OpenAsSvg) => {
            #[cfg(unix)]
            workspace.graph.open_as_svg();
        }
        Some(DotMode::Debug) => {
            writeln!(err, "{graph:#?}", graph = workspace.graph)?;
        }
        None => {}
    }

    Ok(())
}

/// Return `true` when the command can use the workspace graph from
/// `but_ctx::Context` because nothing special is specified.
///
/// Context discovery loads the same workspace graph used by the application,
/// including metadata-backed target handling. The repository is opened directly
/// instead when a reference or another limit is given, which the context doesn't take.
fn uses_context_discovery(graph_args: &GraphArgs) -> bool {
    graph_args.limit == Some(Some(300)) && graph_args.ref_name.is_none()
}

fn dot_mode(graph_args: &GraphArgs) -> Option<DotMode> {
    if graph_args.debug {
        Some(DotMode::Debug)
    } else if graph_args.dot_show {
        Some(DotMode::OpenAsSvg)
    } else if graph_args.dot {
        Some(DotMode::Print)
    } else {
        None
    }
}
