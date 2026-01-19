#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Check for available updates to the GitButler CLI.
    ///
    /// Queries the update server to see if a newer version is available
    /// for your platform and release channel.
    Check,
    /// Suppress update notifications for a specified duration, defaulting to 1 day.
    ///
    /// Temporarily hide update notifications for the CLI. The suppression
    /// will automatically expire after the specified number of days (default: 1 day).
    /// Maximum suppression duration is 30 days.
    Suppress {
        /// Number of days to suppress update notifications (1-30, default: 1)
        #[clap(default_value = "1", value_parser = clap::value_parser!(u32).range(1..=30))]
        days: u32,
    },
}
