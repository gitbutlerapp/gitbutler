//! Implementation of the `graph` debug command.

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
    /// Debug-print the internal graph structure.
    Debug,
}

/// Execute the `graph` subcommand.
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
    let meta = EmptyRefMetadata;

    let extra_target = graph_args
        .extra_target
        .as_deref()
        .map(|rev_spec| repo.rev_parse_single(rev_spec))
        .transpose()?
        .map(|id| id.detach());
    let opts = but_graph::init::Options {
        extra_target_commit_id: extra_target,
        collect_tags: true,
        hard_limit: graph_args.hard_limit,
        commits_limit_hint: graph_args.limit.flatten(),
        commits_limit_recharge_location: graph_args
            .limit_extension
            .iter()
            .map(|short_hash| {
                repo.objects
                    .lookup_prefix(
                        gix::hash::Prefix::from_hex(short_hash).expect("valid hex prefix"),
                        None,
                    )
                    .unwrap()
                    .expect("object for prefix exists")
                    .expect("the prefix is unambiguous")
            })
            .collect(),
        dangerously_skip_postprocessing_for_debugging: graph_args.no_post,
    };

    let graph = match graph_args.ref_name.as_deref() {
        None => but_graph::Graph::from_head(
            &repo,
            &meta,
            but_core::ref_metadata::ProjectMeta::default(),
            opts,
        ),
        Some(ref_name) => {
            let mut reference = repo.find_reference(ref_name)?;
            let id = reference.peel_to_id()?;
            but_graph::Graph::from_commit_traversal(
                id,
                reference.name().to_owned(),
                &meta,
                but_core::ref_metadata::ProjectMeta::default(),
                opts,
            )
        }
    }?;

    let workspace = graph.into_workspace()?;
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
/// including metadata-backed target handling. The manual repository path below
/// is still needed when any debug traversal knob is set, because those options
/// must be passed directly into `but_graph::Graph::from_*`.
fn uses_context_discovery(graph_args: &GraphArgs) -> bool {
    graph_args.extra_target.is_none()
        && !graph_args.no_post
        && graph_args.hard_limit.is_none()
        && graph_args.limit == Some(Some(300))
        && graph_args.limit_extension.is_empty()
        && graph_args.ref_name.is_none()
}

/// Determine which graph output mode should be used.
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
