//! In place of commands.rs

use anyhow::Result;
use but_action::cli::get_cli_path;
#[cfg(target_os = "macos")]
use but_action::cli::{ExistingSymlinkPolicy, InstallMode};
use but_api_macros::but_api;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn install_cli() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        but_action::cli::do_install_cli_v2(
            &get_cli_path()?,
            InstallMode::AllowPrivilegeElevation,
            ExistingSymlinkPolicy::Replace,
        )
    }
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!(
        "Automatic CLI installation is only supported on macOS. See Settings for manual installation instructions."
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn cli_path() -> Result<String> {
    let cli_path = get_cli_path()?;
    Ok(cli_path.to_string_lossy().to_string())
}
