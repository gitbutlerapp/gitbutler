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
}
