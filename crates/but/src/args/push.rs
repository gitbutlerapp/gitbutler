use crate::args::push;

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

pub fn get_gerrit_flags(
    args: &push::Command,
    branch_name: &str,
    gerrit_mode: bool,
) -> anyhow::Result<Vec<but_gerrit::PushFlag>> {
    let has_gerrit_flag = args.wip
        || args.ready
        || !args.hashtag.is_empty()
        || args.topic.is_some()
        || args.topic_from_branch
        || args.private;

    if has_gerrit_flag && !gerrit_mode {
        return Err(anyhow::anyhow!(
            "Gerrit push flags (--wip, --ready, --hashtag/--tag, --topic, --topic-from-branch, --private) can only be used when gerrit_mode is enabled for this repository"
        ));
    }

    if !gerrit_mode {
        return Ok(vec![]);
    }

    let mut flags = Vec::new();

    // Handle Wip/Ready - Ready is default if neither is specified
    if args.wip {
        flags.push(but_gerrit::PushFlag::Wip);
    } else {
        // Default to Ready, or explicit Ready
        flags.push(but_gerrit::PushFlag::Ready);
    }

    // Handle hashtags - can be multiple
    for hashtag in &args.hashtag {
        if hashtag.trim().is_empty() {
            return Err(anyhow::anyhow!("Hashtag cannot be empty"));
        }
        flags.push(but_gerrit::PushFlag::Hashtag(hashtag.clone()));
    }

    // Handle topic - at most one
    if let Some(topic) = &args.topic {
        if topic.trim().is_empty() {
            return Err(anyhow::anyhow!("Topic cannot be empty"));
        }
        flags.push(but_gerrit::PushFlag::Topic(topic.clone()));
    } else if args.topic_from_branch {
        flags.push(but_gerrit::PushFlag::Topic(branch_name.to_string()));
    }

    // Handle private flag
    if args.private {
        flags.push(but_gerrit::PushFlag::Private);
    }

    Ok(flags)
}
