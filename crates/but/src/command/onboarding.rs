//! Onboarding command implementation.
//!
//! Marks onboarding as complete and shows metrics info message (for human output only).

use anyhow::Result;

use crate::utils::OutputChannel;

/// Handle the onboarding command.
///
/// If `onboarding_complete` is false, marks onboarding as complete and shows
/// the metrics info message (only for human-readable output formats).
/// If already complete, does nothing (silent).
pub fn handle(out: &mut OutputChannel) -> Result<()> {
    // Load settings to check onboarding status
    let app_settings_sync = match crate::command::config::load_app_settings_sync() {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!(?err, "Failed to load app settings for onboarding check");
            return Ok(());
        }
    };

    // Check if onboarding is already complete
    let onboarding_complete = match app_settings_sync.get() {
        Ok(settings) => settings.onboarding_complete,
        Err(err) => {
            tracing::warn!(?err, "Failed to read app settings for onboarding check");
            return Ok(());
        }
    };
    if onboarding_complete {
        return Ok(());
    }

    // Mark onboarding as complete
    if let Err(err) = app_settings_sync.update_onboarding_complete(true) {
        tracing::warn!(?err, "Failed to persist onboarding status");
    }

    // Show the metrics info message (only for human output)
    if let Some(human_out) = out.for_human() {
        std::fmt::Write::write_str(
            human_out,
            "GitButler uses metrics to help us know what is useful and improve it. Configure with `but config metrics`.\n",
        )?;
    }

    Ok(())
}
