/// Create an empty Git repository in the current directory.
///
/// Use `but -C <path> init` to initialize another directory. Existing repositories
/// are left unchanged. Does not create commits, configure remotes, or set up a
/// GitButler workspace.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {}
