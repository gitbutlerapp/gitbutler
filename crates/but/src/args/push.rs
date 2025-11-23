#[derive(Debug, clap::Parser)]
pub struct Command {
    /// Branch name or CLI ID to push
    pub branch_id: String,
    /// Force push even if it's not fast-forward
    #[clap(long, short = 'f', default_value_t = true)]
    pub with_force: bool,
    /// Skip force push protection checks
    #[clap(long, short = 's')]
    pub skip_force_push_protection: bool,
    /// Run pre-push hooks
    #[clap(long, short = 'r', default_value_t = true)]
    pub run_hooks: bool,
    /// Mark change as work-in-progress (Gerrit). Mutually exclusive with --ready.
    #[clap(long, short = 'w', conflicts_with = "ready", hide = true)]
    pub wip: bool,
    /// Mark change as ready for review (Gerrit). This is the default state.
    #[clap(long, short = 'y', conflicts_with = "wip", hide = true)]
    pub ready: bool,
    /// Add hashtag(s) to change (Gerrit). Can be used multiple times.
    #[clap(long, short = 'a', alias = "tag", value_name = "TAG", hide = true)]
    pub hashtag: Vec<String>,
    /// Add custom topic to change (Gerrit). At most one topic can be set.
    #[clap(
        long,
        short = 't',
        value_name = "TOPIC",
        conflicts_with = "topic_from_branch",
        hide = true
    )]
    pub topic: Option<String>,
    /// Use branch name as topic (Gerrit)
    #[clap(
        long = "tb",
        alias = "topic-from-branch",
        conflicts_with = "topic",
        hide = true
    )]
    pub topic_from_branch: bool,
    /// Mark change as private (Gerrit)
    #[clap(long, short = 'p', hide = true)]
    pub private: bool,
}
