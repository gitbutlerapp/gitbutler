#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Check if a new version of the GitButler CLI is available
    Check,

    /// Suppress update notifications temporarily
    ///
    /// Hide update notifications for the specified number of days (1-30).
    /// Useful when you want to stay on a specific version temporarily.
    Suppress {
        /// Number of days to suppress (1-30, default: 1)
        #[clap(default_value = "1", value_parser = clap::value_parser!(u32).range(1..=30))]
        days: u32,
    },

    /// Install or update the GitButler desktop application (macOS only)
    ///
    /// Downloads and installs the GitButler desktop app. The CLI (but) is included
    /// with the app and will also be updated.
    ///
    /// By default, auto-detects your current channel (release/nightly) and installs
    /// the latest version for that channel.
    ///
    /// Note: Currently only supported on macOS. For other platforms, download from
    /// <https://gitbutler.com/downloads>
    #[cfg(target_os = "macos")]
    Install {
        /// What to install: "nightly", "release", or a version like "0.18.7"
        ///
        /// Examples:
        ///   but update install           Auto-detect channel and install latest
        ///   but update install nightly   Install latest nightly build
        ///   but update install release   Install latest stable release
        ///   but update install 0.18.7    Install specific version
        target: Option<String>,
    },
}
