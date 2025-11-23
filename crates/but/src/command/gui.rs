use anyhow::{Context as _, Result};
use but_path::AppChannel;

/// Open the GitButler GUI application for `possibly_project_dir`.
///
/// This expects that the GUI application is present and has correctly registered URL
/// schemes for the different channels.
pub fn open(possibly_project_dir: &std::path::Path) -> Result<()> {
    let channel = AppChannel::new();
    let absolute_path = std::fs::canonicalize(possibly_project_dir).with_context(|| {
        format!(
            "Failed to canonicalize path: {}",
            possibly_project_dir.display()
        )
    })?;
    channel.open(&absolute_path)?;
    Ok(())
}
