//! Arguments for `_publish`.

#![deny(missing_docs)]

/// Publish this checkout to a hosted GitButler server, read-only.
///
/// Pushes the checkout's branches and remote-tracking branches into the
/// server's copy of the project, in one atomic push, with a note saying what
/// `HEAD` is and which target it tracks. The server shows what was last
/// published; nothing is uploaded otherwise.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The server to publish to, e.g. `https://but.example.com`.
    #[clap(long, env = "BUT_PUBLISH_URL")]
    pub to: String,

    /// The server's access token.
    #[clap(long, env = "BUT_PUBLISH_TOKEN", hide_env_values = true)]
    pub token: String,

    /// The name this machine is listed under, defaulting to its host name.
    #[clap(long)]
    pub machine: Option<String>,

    /// Also publish uncommitted changes, as a commit on top of `HEAD`.
    #[clap(long)]
    pub include_uncommitted: bool,
}
