//! Arguments for `pick`.

#![deny(missing_docs)]

use crate::args::atoms::{AllowMergedArg, CliIdArg};

/// Cherry-pick commits into an applied branch.
///
/// Each source commit is copied to the target location as a new commit.
///
/// If there are no branches applied, a new branch is created for the picked commits. If there is
/// only one stack of branches applied, the commits are placed at the tip of that stack. Otherwise,
/// the targeting flags `--above`, `--below`, and `--branch` control where the commits are placed.
/// `--above` and `--below` are mutually exclusive; combine either with `--branch` to name a new
/// branch when targeting a branch.
///
/// For more details about CLI IDs, see `but help cli-ids`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// Place the picked commits on the branch `BRANCH`.
    ///
    /// With `--above` or `--below`, name the new branch created relative to the target branch.
    /// The name must not already exist; omit it for a generated name. Cannot be combined with
    /// commit or worktree targets.
    ///
    /// Otherwise, if `BRANCH` does not exist, it is created as an unstacked branch.
    /// If `BRANCH` is omitted, an unstacked branch with a generated name is created.
    ///
    /// Attempting to pick onto a branch that exists but is not applied is an error.
    #[clap(short, long, value_name = "BRANCH")]
    pub branch: Option<Option<CliIdArg>>,

    /// Place the picked commits above `BRANCH_OR_COMMIT`.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the picked commits are placed on the same branch as the
    /// targeted commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the picked commits are placed on a new branch above the
    /// targeted branch. Use `--branch <NAME>` to name it; otherwise a name is generated.
    #[clap(
        short = 'A',
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting"
    )]
    pub above: Option<CliIdArg>,

    /// Place the picked commits below `BRANCH_OR_COMMIT`.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the picked commits are placed on the same branch as the
    /// targeted commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the picked commits are placed on a new branch below the
    /// targeted branch. Branches are treated as buckets, meaning that "below a branch" is treated
    /// as below the oldest ancestor on that branch. Use `--branch <NAME>` to name the new branch;
    /// otherwise a name is generated.
    #[clap(
        short = 'B',
        long,
        value_name = "BRANCH_OR_COMMIT",
        group = "targeting"
    )]
    pub below: Option<CliIdArg>,

    /// The commits to copy, as SHAs or as CLI IDs of commits on applied branches. IDs
    /// shown for unapplied branches do not resolve; use the SHA.
    #[clap(group = "changes_to_commit", required = true)]
    pub sources: Vec<CliIdArg>,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub allow_merged: AllowMergedArg,
}
