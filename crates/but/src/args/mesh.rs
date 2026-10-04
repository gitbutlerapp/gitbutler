//! Arguments for `mesh`.

#![deny(missing_docs)]

/// Every machine and repository of your GitButler account at a glance.
///
/// Lists this machine's repositories and what your other machines published to
/// the hosted server, with who's online, kept live as machines publish. Signs
/// in to GitButler first if needed. The server is `BUT_HOSTED_URL`,
/// https://mesh.but.dev by default.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// Forget the signed-in GitButler account and exit.
    #[clap(long)]
    pub sign_out: bool,
}
