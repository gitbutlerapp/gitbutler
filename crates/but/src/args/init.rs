/// Create an empty Git repository in the current directory.
///
/// Use `but -C <path> init` to initialize another directory. Existing repositories
/// are left unchanged.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {
    /// Also create an initial empty commit.
    #[clap(long)]
    pub commit: bool,
}
