//! Arguments for `panel`.

#![deny(missing_docs)]

/// Serve a live, read-only view of the workspace in the browser (experimental).
///
/// The panel shows stacks, branches, commits, diffs, and linked worktrees on a page served from
/// `127.0.0.1`, refreshing every few seconds. It is narrow enough to sit in a side pane next to an
/// agent chat. Nothing in it changes the repository.
///
/// The server runs in the foreground; stop it with Ctrl-C.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The local port to serve the panel on.
    #[clap(long, default_value_t = 7789)]
    pub port: u16,

    /// Start the server without opening a browser.
    #[clap(long)]
    pub no_open: bool,
}
