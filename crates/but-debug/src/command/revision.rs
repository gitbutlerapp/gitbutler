use std::io::{self, Write as _};

use anyhow::{Context as _, Result, bail, ensure};
use but_graph::FirstParent;
use gix::{odb::store::RefreshMode, reference::Category, revision::plumbing::Spec};

use crate::{
    args::{Args, LogArgs, MergeBaseArgs, RevisionArgs, RevisionGraphArgs, RevisionSubcommands},
    metadata::EmptyRefMetadata,
    setup,
};

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

    let target_commit_id = target_commit_from_args(repo, &log_args.graph)?;
    let graph = {
        let _span =
            tracing::info_span!("build graph", commit_count = graph_commits.len()).entered();
        graph_for_revisions(repo, meta, &graph_commits, target_commit_id)?
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

    let target_commit_id = target_commit_from_args(repo, &merge_base_args.graph)?;
    let graph = {
        let _span = tracing::info_span!("build graph", commit_count = commits.len()).entered();
        graph_for_revisions(repo, meta, &commits, target_commit_id)?
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

/// The commit of the remote-tracking branch `graph_args` names as the target, if any.
fn target_commit_from_args(
    repo: &gix::Repository,
    graph_args: &RevisionGraphArgs,
) -> Result<Option<gix::ObjectId>> {
    graph_args
        .target_ref
        .as_deref()
        .map(|target_ref| {
            let mut reference = repo
                .find_reference(target_ref)
                .with_context(|| format!("Failed to find target ref '{target_ref}'"))?;
            let name = reference.name().to_owned();
            ensure!(
                name.category() == Some(Category::RemoteBranch),
                "Target ref '{name}' resolved from '{target_ref}' is not a remote-tracking branch"
            );
            Ok(reference.peel_to_id()?.detach())
        })
        .transpose()
}

/// Walk the graph from `commits` and the target commit, ending below the latter.
fn graph_for_revisions(
    repo: &gix::Repository,
    meta: &EmptyRefMetadata,
    commits: &[gix::ObjectId],
    target_commit_id: Option<gix::ObjectId>,
) -> Result<but_graph::Graph> {
    let first = *commits
        .first()
        .context("BUG: revision graph requires at least one commit")?;
    let mut heads = commits.to_vec();
    heads.extend(target_commit_id);
    but_graph::nosegment::segment_graph(
        repo,
        meta,
        but_graph::init::Overlay::default(),
        but_core::ref_metadata::ProjectMeta::default(),
        None,
        Some(first),
        heads,
        Vec::new(),
        target_commit_id,
        false,
        None,
    )
}
