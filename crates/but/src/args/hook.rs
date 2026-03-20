/// Arguments for git hook subcommands.
///
/// These commands implement GitButler's workspace guard and cleanup logic
/// as standalone CLI commands that any hook manager (prek, lefthook, husky, etc.)
/// can invoke.
#[derive(Debug, clap::Parser)]
pub struct Platform {
    /// The hook subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

/// Available hook subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Workspace guard for pre-commit hooks.
    ///
    /// Blocks direct `git commit` on the `gitbutler/workspace` branch with a
    /// helpful error message directing the user to use `but commit` instead.
    /// Exits 0 (allow) on any other branch.
    ///
    /// This command is designed to be called from a hook manager configuration:
    ///
    /// ## Examples
    ///
    /// prek.toml:
    ///
    /// ```text
    /// [[repos]]
    /// repo = "local"
    /// hooks = [{ id = "gitbutler-workspace-guard", language = "system", entry = "but hook pre-commit" }]
    /// ```
    ///
    /// lefthook.yml:
    ///
    /// ```text
    /// pre-commit:
    ///   commands:
    ///     gitbutler-guard:
    ///       run: but hook pre-commit
    /// ```
    ///
    #[clap(verbatim_doc_comment)]
    PreCommit,

    /// Informational hook for post-checkout events.
    ///
    /// When leaving the `gitbutler/workspace` branch, prints an informational
    /// message noting you have left GitButler mode and directing you to run
    /// `but setup` to return.
    ///
    /// Accepts the standard post-checkout arguments (prev_head, new_head, is_branch_checkout)
    /// as positional parameters, matching git's post-checkout hook signature.
    ///
    /// ## Examples
    ///
    /// prek.toml:
    ///
    /// ```text
    /// [[repos]]
    /// repo = "local"
    /// hooks = [{ id = "gitbutler-post-checkout", language = "system", entry = "but hook post-checkout" }]
    /// ```
    ///
    #[clap(verbatim_doc_comment)]
    PostCheckout {
        /// The ref of the previous HEAD (provided by git).
        #[clap(default_value = "")]
        prev_head: String,
        /// The ref of the new HEAD (provided by git).
        #[clap(default_value = "")]
        new_head: String,
        /// Whether this is a branch checkout (1) or a file checkout (0).
        #[clap(default_value = "1")]
        is_branch_checkout: String,
    },

    /// Show hook ownership and integration state.
    ///
    /// Displays diagnostics about the current repository's Git hook
    /// configuration: which hooks are installed, whether they are
    /// GitButler-managed or owned by an external hook manager, and
    /// recommended next actions.
    ///
    /// ## Examples
    ///
    /// ```text
    /// but hook status
    /// but hook status --json
    /// ```
    ///
    #[clap(verbatim_doc_comment)]
    Status,

    /// Push guard for pre-push hooks.
    ///
    /// Blocks `git push` when on the `gitbutler/workspace` branch with a
    /// helpful error message directing the user to use `but push` instead.
    /// Exits 0 (allow) on any other branch.
    ///
    /// Accepts the standard pre-push arguments (remote_name, remote_url)
    /// as positional parameters, matching git's pre-push hook signature.
    /// Stdin refspec lines from git are not inspected.
    ///
    /// ## Examples
    ///
    /// prek.toml:
    ///
    /// ```text
    /// [[repos]]
    /// repo = "local"
    /// hooks = [{ id = "gitbutler-push-guard", language = "system", entry = "but hook pre-push" }]
    /// ```
    ///
    /// lefthook.yml:
    ///
    /// ```text
    /// pre-push:
    ///   commands:
    ///     gitbutler-push-guard:
    ///       run: but hook pre-push
    /// ```
    ///
    #[clap(verbatim_doc_comment)]
    PrePush {
        /// The name of the remote being pushed to (provided by git).
        #[clap(default_value = "")]
        remote_name: String,
        /// The URL of the remote being pushed to (provided by git).
        #[clap(default_value = "")]
        remote_url: String,
    },
}
