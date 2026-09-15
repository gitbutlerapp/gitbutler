//! In place of commands.rs

use anyhow::Result;
#[cfg(target_os = "macos")]
use but_action::cli::InstallMode;
use but_action::cli::{ExistingSymlinkPolicy, get_cli_path};
use but_api_macros::but_api;
use tracing::instrument;

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ExistingSymlinkPolicy);

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

/// Install the bundled macOS CLI. Trusted hosts supply the source path, not renderers.
/// Returns false when administrator authorization is cancelled.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn install_cli_v2(cli_path: String, symlink_policy: ExistingSymlinkPolicy) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        match but_action::cli::do_install_cli_v2(
            std::path::Path::new(&cli_path),
            InstallMode::AllowPrivilegeElevation,
            symlink_policy,
        ) {
            Ok(()) => Ok(true),
            Err(err)
                if err
                    .downcast_ref::<but_error::Context>()
                    .is_some_and(|context| {
                        matches!(context.code, but_error::Code::CliInstallCancelled)
                    }) =>
            {
                Ok(false)
            }
            Err(err) => Err(err),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (cli_path, symlink_policy);
        anyhow::bail!("CLI installation from the app is only supported on macOS")
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn cli_path() -> Result<String> {
    let cli_path = get_cli_path()?;
    Ok(cli_path.to_string_lossy().to_string())
}
