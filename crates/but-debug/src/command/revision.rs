//! Implementation of the `revision` debug commands.

use std::io::{self, Write as _};

use anyhow::{Context as _, Result, bail, ensure};
use but_graph::FirstParent;
use but_graph::init::Tip;
use gix::{odb::store::RefreshMode, reference::Category, revision::plumbing::Spec};

use crate::{
    args::{Args, LogArgs, MergeBaseArgs, RevisionArgs, RevisionGraphArgs, RevisionSubcommands},
    metadata::EmptyRefMetadata,
    setup,
};

/// Execute the `revision` subcommand.
pub(crate) fn run(
    args: &Args,
    revision_args: &RevisionArgs,
    out: &mut dyn io::Write,
) -> Result<()> {
    let mut repo = setup::repo_from_args(args)?;
    repo.objects.refresh = RefreshMode::Never;
    let meta = EmptyRefMetadata;

    match &revision_args.cmd {
        RevisionSubcommands::Log(log_args) => log(&repo, &meta, log_args, out),
        RevisionSubcommands::MergeBase(merge_base_args) => {
            merge_base(&repo, &meta, merge_base_args, out)
        }
    }
}

fn log(
    repo: &gix::Repository,
    meta: &EmptyRefMetadata,
    log_args: &LogArgs,
    out: &mut dyn io::Write,
) -> Result<()> {
    let parsed = repo
        .rev_parse(log_args.rev_spec.as_str())
        .with_context(|| format!("Failed to parse rev-spec '{}'", log_args.rev_spec))?
        .detach();

    let (included, excluded) = match parsed {
        Spec::Include(commit_id) => (commit_id, None),
        Spec::Range { from, to } => (to, Some(from)),
        other => bail!("Unsupported rev-spec for revision log: {other}"),
    };
    let mut graph_commits = vec![included];
    graph_commits.extend(excluded);

    let graph_tips = args_to_tips(repo, &log_args.graph)?;
    let graph = {
        let _span =
            tracing::info_span!("build graph", commit_count = graph_commits.len()).entered();
        graph_for_revisions(repo, meta, &graph_commits, graph_tips)?
    };

    let _span = tracing::info_span!("traverse graph").entered();
    let commits = if let Some(excluded) = excluded {
        graph.find_commit_ids_reachable_from_a_not_b(
            included,
            excluded,
            FirstParent::from(log_args.first_parent),
        )?
    } else {
        bail!("Need to specify a rev-spec of form `a..b` to indicate an exclusion for now.")
    };

    let mut out = io::BufWriter::new(out);
    for commit_id in commits {
        commit_id.write_hex_to(&mut out)?;
        writeln!(out)?;
    }
    Ok(())
}

fn merge_base(
    repo: &gix::Repository,
    meta: &EmptyRefMetadata,
    merge_base_args: &MergeBaseArgs,
    out: &mut dyn io::Write,
) -> Result<()> {
    let commits = {
        let _span = tracing::info_span!(
            "resolve revisions",
            revision_count = merge_base_args.revisions.len()
        )
        .entered();
        merge_base_args
            .revisions
            .iter()
            .map(|rev| {
                repo.rev_parse_single(rev.as_str())
                    .map(|id| id.detach())
                    .with_context(|| format!("Failed to resolve revision '{rev}'"))
            })
            .collect::<Result<Vec<_>>>()?
    };

    let graph_tips = args_to_tips(repo, &merge_base_args.graph)?;
    let graph = {
        let _span = tracing::info_span!("build graph", commit_count = commits.len()).entered();
        graph_for_revisions(repo, meta, &commits, graph_tips)?
    };

    let segments = {
        let _span = tracing::info_span!("map commit ids to segments", commit_count = commits.len())
            .entered();
        commits
            .iter()
            .copied()
            .map(|commit_id| graph.segment_id_by_commit_id(commit_id))
            .collect::<Result<Vec<_>>>()
            .context("Failed to map commit ids to graph segments")?
    };

    let merge_base = {
        let _span = tracing::info_span!("compute octopus merge-base", commit_count = commits.len())
            .entered();
        graph
            .find_merge_base_octopus(segments)
            .map(|segment_id| {
                graph
                    .tip_skip_empty(segment_id)
                    .map(|commit| commit.id)
                    .with_context(|| {
                        format!(
                            "BUG: Segment {segment_id:?} does not contain a reachable tip commit"
                        )
                    })
            })
            .transpose()
            .context("Failed to compute octopus merge-base from graph")?
    };

    let Some(merge_base) = merge_base else {
        bail!(
            "No merge-base found for revisions: {}",
            merge_base_args.revisions.join(", ")
        );
    };
    writeln!(out, "{merge_base}")?;

    Ok(())
}

fn args_to_tips(repo: &gix::Repository, graph_args: &RevisionGraphArgs) -> Result<Vec<Tip>> {
    let mut tips = Vec::new();

    if let Some(tip) = graph_args
        .target_ref
        .as_deref()
        .map(|target_ref| {
            let mut reference = repo
                .find_reference(target_ref)
                .with_context(|| format!("Failed to find target ref '{target_ref}'"))?;
            let name = reference.name().to_owned();
            ensure!(
                name.category() == Some(Category::RemoteBranch),
                "Target ref '{name}' resolved from '{target_ref}' is not a remote-tracking branch; use --extra-target for arbitrary revisions"
            );
            let id = reference.peel_to_id()?.detach();
            Ok(Tip::integrated(id, Some(name)))
        })
        .transpose()?
    {
        tips.push(tip);
    }

    if let Some(tip) = graph_args
        .extra_target
        .as_deref()
        .map(|rev| {
            repo.rev_parse_single(rev)
                .map(|id| Tip::integrated(id.detach(), None))
                .with_context(|| format!("Failed to resolve extra target '{rev}'"))
        })
        .transpose()?
    {
        tips.push(tip);
    }

    Ok(tips)
}

fn graph_for_revisions(
    repo: &gix::Repository,
    meta: &EmptyRefMetadata,
    commits: &[gix::ObjectId],
    graph_tips: Vec<Tip>,
) -> Result<but_graph::Graph> {
    let first = *commits
        .first()
        .context("BUG: revision graph requires at least one commit")?;
    let options = but_graph::init::Options {
        collect_tags: false,
        commits_limit_hint: None,
        ..Default::default()
    };
    let tips = std::iter::once(Tip::entrypoint(first, None))
        .chain(
            commits
                .iter()
                .copied()
                .skip(1)
                .map(|id| Tip::reachable(id, None)),
        )
        .chain(graph_tips);

    but_graph::Graph::from_commit_traversal_tips(
        repo,
        tips,
        meta,
        but_core::ref_metadata::ProjectMeta::default(),
        options,
    )
}
