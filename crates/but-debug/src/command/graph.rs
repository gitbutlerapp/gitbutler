//! Implementation of the `graph` debug command.

use std::{
    fmt::Write as _,
    io::{self, Write as _},
    process::{Command, Stdio},
};

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
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: extra_target,
        ..Default::default()
    };

    let overlay = match graph_args.ref_name.as_deref() {
        None => but_graph::init::Overlay::default(),
        Some(ref_name) => {
            let mut reference = repo.find_reference(ref_name)?;
            let name = reference.name().to_owned();
            let id = reference.peel_to_id()?.detach();
            but_graph::init::Overlay::default().with_entrypoint(id, Some(name))
        }
    };
    let graph = but_graph::Graph::from_repo(&repo, &meta, project_meta, overlay)?;

    let workspace = graph.into_workspace()?;
    emit_workspace(&workspace, graph_args, out, err)
}

fn emit_workspace(
    workspace: &but_graph::Workspace,
    graph_args: &GraphArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    if graph_args.stats {
        emit_statistics(&workspace.graph, err)?;
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
            out.write_all(node_graph_dot(&workspace.graph).as_bytes())?;
        }
        Some(DotMode::OpenAsSvg) => {
            #[cfg(unix)]
            open_as_svg(&workspace.graph)?;
        }
        Some(DotMode::Debug) => {
            writeln!(err, "{graph:#?}", graph = workspace.graph)?;
        }
        None => {}
    }

    Ok(())
}

fn emit_statistics(graph: &but_graph::Graph, err: &mut dyn io::Write) -> Result<()> {
    let mut commits = 0;
    let mut references = 0;
    let mut shallow_points = 0;
    for node in graph.nodes() {
        match node.kind() {
            but_graph::NodeKind::Commit { .. } => commits += 1,
            but_graph::NodeKind::Reference(_) => references += 1,
            but_graph::NodeKind::Boundary { .. } => shallow_points += 1,
        }
    }
    let edges = graph
        .nodes()
        .iter()
        .map(|node| node.parents().len())
        .sum::<usize>();
    writeln!(
        err,
        "Graph with {commits} commits, {references} references, {shallow_points} shallow points, and {edges} edges"
    )?;
    Ok(())
}

pub(super) fn node_graph_dot(graph: &but_graph::Graph) -> String {
    let mut out = String::from("digraph {\n  node [shape=box, fontname=Courier];\n");
    for (index, node) in graph.nodes().iter().enumerate() {
        let entrypoint = matches!(
            graph.entrypoint(),
            but_graph::NodeGraphEntrypoint::Node(entrypoint) if *entrypoint == index
        );
        let label = match node.kind() {
            but_graph::NodeKind::Commit { id } => format!(
                "{}{} {}",
                if entrypoint { "HEAD " } else { "" },
                id.to_hex_with_len(7),
                graph.annotations()[index].debug_string()
            ),
            but_graph::NodeKind::Reference(reference) => format!(
                "{}{}",
                if entrypoint { "HEAD " } else { "" },
                reference.ref_info.ref_name
            ),
            but_graph::NodeKind::Boundary { id, reason } => {
                format!("{} {}", id.to_hex_with_len(7), reason.debug_string())
            }
        };
        writeln!(out, "  {index} [label={label:?}];").expect("writing to a string cannot fail");
        for (order, parent) in node.parents().iter().enumerate() {
            writeln!(out, "  {index} -> {parent} [label=\"{order}\"];")
                .expect("writing to a string cannot fail");
        }
    }
    if let but_graph::NodeGraphEntrypoint::Unborn(reference) = graph.entrypoint() {
        let label = format!("HEAD {} (unborn)", reference.ref_info.ref_name);
        writeln!(out, "  unborn [label={label:?}];").expect("writing to a string cannot fail");
    }
    out.push_str("}\n");
    out
}

#[cfg(unix)]
fn open_as_svg(graph: &but_graph::Graph) -> Result<()> {
    let svg_path = std::env::temp_dir().join(format!("but-debug-graph-{}.svg", std::process::id()));
    let mut dot = Command::new("dot")
        .args(["-Tsvg", "-o"])
        .arg(&svg_path)
        .stdin(Stdio::piped())
        .spawn()
        .context("Could not launch Graphviz")?;
    dot.stdin
        .take()
        .context("Graphviz stdin wasn't piped")?
        .write_all(node_graph_dot(graph).as_bytes())?;
    let status = dot.wait().context("Could not wait for Graphviz")?;
    anyhow::ensure!(status.success(), "Graphviz failed with {status}");
    open::that(&svg_path).context("Could not open graph SVG")?;
    Ok(())
}

/// Return `true` when the command can use the workspace graph from
/// `but_ctx::Context` because nothing special is specified.
///
/// Context discovery loads the same workspace graph used by the application,
/// including metadata-backed target handling. The manual repository path below
/// is still needed for an explicit entrypoint or extra target.
fn uses_context_discovery(graph_args: &GraphArgs) -> bool {
    graph_args.extra_target.is_none() && graph_args.ref_name.is_none()
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
