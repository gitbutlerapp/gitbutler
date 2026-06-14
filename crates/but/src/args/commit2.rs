use crate::args::atoms::BranchArg;

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(long)]
    pub no_message: bool,
    #[clap(short, long, conflicts_with = "no_message")]
    pub message: Option<Vec<String>>,
    #[clap(short, long)]
    pub branch: Option<Option<BranchArg>>,
    #[clap(long)]
    pub empty: bool,
}
