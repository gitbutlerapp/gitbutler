use crate::args::atoms::CliIdArg;

#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[clap(group(
    clap::ArgGroup::new("targeting")
        .args(["above", "below", "branch", "unstack"])
        .required(true)
))]
pub struct Platform {
    /// Place `SOURCES` on the branch `BRANCH`.
    ///
    /// If `BRANCH` does not exist, it is created as an unstacked branch.
    ///
    /// If `BRANCH` is omitted, an unstacked branch with a generated name is created.
    ///
    /// Attempting to place `SOURCES` on a branch that exists but is not applied is an error.
    ///
    /// This target is only applicable if `SOURCES` consists of commits or committed files.
    #[clap(short, long, value_name = "BRANCH")]
    pub branch: Option<Option<CliIdArg>>,
    /// Place the `SOURCES` above `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the source commit is placed on the same branch as the
    /// targeted commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the source commit is placed on a new branch above the
    /// targeted branch.
    ///
    /// This target is applicable for all kinds of `SOURCES`.
    #[clap(short = 'A', long, value_name = "BRANCH_OR_COMMIT")]
    pub above: Option<CliIdArg>,
    /// Place the commit below `BRANCH_OR_COMMIT`, which must be an applied branch or commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a commit, the source commit is placed on the same branch as the
    /// targeted commit.
    ///
    /// If `BRANCH_OR_COMMIT` is a branch, the source commit is placed on a new branch below the
    /// targeted branch. Branches are treated as buckets, meaning that "below a branch" is treated
    /// as below the oldest ancestor on that branch.
    ///
    /// This target is only applicable if `SOURCES` consists of commits or committed files.
    #[clap(short = 'B', long, value_name = "BRANCH_OR_COMMIT")]
    pub below: Option<CliIdArg>,
    /// Unstack a branch source from its current stack, making it an independent branch.
    ///
    /// This target is only applicable when `SOURCES` is a single branch. Any other kind of source
    /// will cause an error.
    ///
    /// `--unstack` does not take an argument, so `--unstack <source>` and `<source> --unstack` are
    /// equivalent.
    #[clap(long)]
    pub unstack: bool,
    /// One or more sources to move.
    ///
    /// Providing any of the sources as an argument for `--above` or `--below` is an error.
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,
}
