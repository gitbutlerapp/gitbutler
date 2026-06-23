use crate::args::atoms::CliIdArg;

#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    #[clap(short, long, group = "commit_message")]
    pub message: Option<Vec<String>>,
    #[clap(long, group = "commit_message")]
    pub no_message: bool,
    #[clap(long, short = 'u', group = "commit_message")]
    pub use_target_message: bool,
    #[clap(long, group = "commit_message")]
    pub use_source_message: bool,
    #[clap(long, short)]
    pub target: Option<CliIdArg>,
    #[clap(required = true)]
    pub sources: Vec<CliIdArg>,
}
