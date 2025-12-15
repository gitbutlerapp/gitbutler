pub mod help {
    use but_core::RepositoryExt;

    use crate::utils::OutputChannel;

    fn is_gerrit_enabled() -> bool {
        // Parse the -C flag from command line arguments
        let args: Vec<String> = std::env::args().collect();
        let mut current_dir = std::path::Path::new(".");

        // Look for -C flag
        for (i, arg) in args.iter().enumerate() {
            if arg == "-C" && i + 1 < args.len() {
                current_dir = std::path::Path::new(&args[i + 1]);
                break;
            }
        }

        // Try to check if we're in a gerrit-enabled repository for help display
        if let Ok(repo) = gix::discover(current_dir)
            && let Ok(settings) = repo.git_settings()
        {
            return settings.gitbutler_gerrit_mode.unwrap_or(false);
        }
        false
    }

    pub fn print(out: &mut OutputChannel) -> std::fmt::Result {
        use std::fmt::Write;
        writeln!(out, "Push a branch/stack to remote")?;
        writeln!(out,)?;
        writeln!(out, "Usage: but push [OPTIONS] <BRANCH_ID>")?;
        writeln!(out,)?;
        writeln!(out, "Arguments:")?;
        writeln!(out, "  <BRANCH_ID>  Branch name or CLI ID to push")?;
        writeln!(out,)?;
        writeln!(out, "Options:")?;
        writeln!(
            out,
            "  -f, --with-force                  Force push even if it's not fast-forward"
        )?;
        writeln!(
            out,
            "  -s, --skip-force-push-protection  Skip force push protection checks"
        )?;
        writeln!(
            out,
            "  -r, --run-hooks                   Run pre-push hooks"
        )?;

        // Check if gerrit mode is enabled and show gerrit options
        if is_gerrit_enabled() {
            writeln!(out,)?;
            writeln!(out, "Gerrit Options:")?;
            writeln!(
                out,
                "  -w, --wip                         Mark change as work-in-progress"
            )?;
            writeln!(
                out,
                "  -y, --ready                       Mark change as ready for review (default)"
            )?;
            writeln!(
                out,
                "  -a, --hashtag, --tag <TAG>        Add hashtag to change (can be used multiple times)"
            )?;
            writeln!(
                out,
                "  -t, --topic <TOPIC>               Add custom topic to change"
            )?;
            writeln!(
                out,
                "      --tb, --topic-from-branch     Use branch name as topic"
            )?;
            writeln!(
                out,
                "  -p, --private                     Mark change as private"
            )?;
            writeln!(out,)?;
            writeln!(out, "Notes:")?;
            writeln!(
                out,
                "  - --wip and --ready are mutually exclusive. Ready is the default state."
            )?;
            writeln!(
                out,
                "  - --topic and --topic-from-branch are mutually exclusive. At most one topic can be set."
            )?;
            writeln!(
                out,
                "  - Multiple hashtags can be specified by using --hashtag (or --tag) multiple times."
            )?;
            writeln!(
                out,
                "  - Multiple flags can be combined (e.g., --ready --private --tag tag1 --hashtag tag2)."
            )?;
        }

        writeln!(out, "  -h, --help                        Print help")?;

        Ok(())
    }
}
