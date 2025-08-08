use std::path::Path;

use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;

pub fn project_status(project_dir: &Path) -> anyhow::Result<but_tools::workspace::ProjectStatus> {
    let repo = crate::mcp_internal::project::project_repo(project_dir)?;

    let project = super::project::project_from_path(project_dir)?;
    // Enable v3 feature flags for the command context
    let app_settings = AppSettings {
        feature_flags: but_settings::app_settings::FeatureFlags {
            v3: true,
            // Keep this off until it caught up at least.
            ws3: false,
            actions: false,
            butbot: false,
            rules: false,
        },
        ..AppSettings::load_from_default_path_creating()?
    };
    let mut ctx = CommandContext::open(&project, app_settings)?;

    but_tools::workspace::get_project_status(&mut ctx, &repo, None)
}
