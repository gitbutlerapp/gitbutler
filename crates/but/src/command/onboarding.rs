use anyhow::Result;

use crate::utils::OutputChannel;

/// If `onboarding_complete` is false, marks onboarding as complete and shows
/// the metrics info message (only for human-readable output formats).
/// If already complete, does nothing (silent).
pub fn handle(out: &mut OutputChannel) -> Result<()> {
    let app_settings_sync = match crate::command::config::load_app_settings_sync() {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!(?err, "Failed to load app settings for onboarding check");
            return Ok(());
        }
    };

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

    if let Err(err) = app_settings_sync.update_onboarding_complete(true) {
        tracing::warn!(?err, "Failed to persist onboarding status");
    }

    if let Some(human_out) = out.for_human_ui() {
        std::fmt::Write::write_str(
            human_out,
            "GitButler uses metrics to help us know what is useful and improve it. Configure with `but config metrics`.\n",
        )?;
    }

    Ok(())
}
