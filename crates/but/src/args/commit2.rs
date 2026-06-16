use crate::args::atoms::{BranchArg, CliIdArg};

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(long)]
    pub no_message: bool,
    #[clap(short, long, conflicts_with = "no_message")]
    pub message: Option<Vec<String>>,
    #[clap(short, long, group = "targeting")]
    pub branch: Option<Option<BranchArg>>,
    #[clap(long, group = "changes_to_commit")]
    pub empty: bool,
    #[clap(long, group = "targeting")]
    pub above: Option<CliIdArg>,
    #[clap(long, group = "targeting")]
    pub below: Option<CliIdArg>,
    #[clap(group = "changes_to_commit")]
    pub changes: Vec<CliIdArg>,
}
