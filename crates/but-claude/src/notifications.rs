use anyhow::Result;
use but_settings::AppSettings;
use notify_rust::Notification;

/// Send a notification when Claude Code has finished executing
pub fn notify_completion(settings: &AppSettings) -> Result<()> {
    if !settings.claude.notify_on_completion {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let _ = notify_rust::set_application(but_path::identifier());
    }

    Notification::new()
        .summary("Claude Code Complete")
        .body("Claude Code has finished executing.")
        .show()?;

    Ok(())
}

/// Send a notification when Claude Code needs permission
pub fn notify_permission_request(settings: &AppSettings, tool_name: &str) -> Result<()> {
    if !settings.claude.notify_on_permission_request {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let _ = notify_rust::set_application(but_path::identifier());
    }

    Notification::new()
        .summary("Claude Code Needs Permission")
        .body(&format!(
            "Claude Code is requesting permission to use: {}",
            tool_name
        ))
        .show()?;

    Ok(())
}
