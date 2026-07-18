//! Implementation of the `revision` debug commands.

use std::io::{self, Write as _};

use anyhow::{Context as _, Result, bail};
use gix::{odb::store::RefreshMode, revision::plumbing::Spec};

use crate::{
    args::{Args, LogArgs, MergeBaseArgs, RevisionArgs, RevisionSubcommands},
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

    match &revision_args.cmd {
        RevisionSubcommands::Log(log_args) => log(&repo, log_args, out),
        RevisionSubcommands::MergeBase(merge_base_args) => merge_base(&repo, merge_base_args, out),
    }
}

fn log(repo: &gix::Repository, log_args: &LogArgs, out: &mut dyn io::Write) -> Result<()> {
    let parsed = repo
        .rev_parse(log_args.rev_spec.as_str())
        .with_context(|| format!("Failed to parse rev-spec '{}'", log_args.rev_spec))?
        .detach();

    let (included, excluded) = match parsed {
        Spec::Include(commit_id) => (commit_id, None),
        Spec::Range { from, to } => (to, Some(from)),
        other => bail!("Unsupported rev-spec for revision log: {other}"),
    };
    let _span = tracing::info_span!("traverse graph").entered();
    let commits = if let Some(excluded) = excluded {
        let walk = repo.rev_walk([included]).with_hidden([excluded]);
        let walk = if log_args.first_parent {
            walk.first_parent_only()
        } else {
            walk
        };
        walk.all()?
            .map(|info| Ok(info?.id))
            .collect::<Result<Vec<_>>>()?
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

    let merge_base = {
        let _span = tracing::info_span!("compute octopus merge-base", commit_count = commits.len())
            .entered();
        repo.merge_base_octopus(commits.iter().copied())
            .context("Failed to compute octopus merge-base")?
            .detach()
    };
    writeln!(out, "{merge_base}")?;

    Ok(())
}
