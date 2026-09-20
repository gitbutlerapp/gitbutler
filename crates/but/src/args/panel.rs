//! Arguments for `panel`.

#![deny(missing_docs)]

/// Serve a live view of the workspace in the browser (experimental).
///
/// The panel shows stacks, branches, commits, diffs, and linked worktrees on a page served from
/// `127.0.0.1`, refreshing every few seconds. It is narrow enough to sit in a side pane next to an
/// agent chat. It can open files, folders and forge pages, fetch, push a branch or its stack, and
/// pull the target's new commits into the workspace; it never edits commits or the worktree.
///
/// One server shows every project, and it runs in the background: `but panel` starts it if it
/// isn't running yet, shows this project in it, and returns. Stop it with `but panel --stop`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The local port to serve the panel on.
    #[clap(long, default_value_t = 7789)]
    pub port: u16,

    /// Start the server without opening a browser.
    #[clap(long)]
    pub no_open: bool,

    /// Open the panel in a window of its own, without browser chrome, in the first of Chrome,
    /// Arc, Edge, Brave or Chromium that is installed. Falls back to the default browser. The
    /// page can also be installed as an app from such a browser's menu.
    #[clap(long, conflicts_with = "no_open")]
    pub app: bool,

    /// Serve from this process instead of the background, until interrupted with Ctrl-C.
    #[clap(long)]
    pub foreground: bool,

    /// Stop the panel server running on `--port`.
    #[clap(long, conflicts_with_all = ["no_open", "app", "foreground"])]
    pub stop: bool,
}
