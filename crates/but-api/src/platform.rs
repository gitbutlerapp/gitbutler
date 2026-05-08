use anyhow::Result;
use but_api_macros::but_api;
use serde::Serialize;

/// Capabilities that vary by how the backend was launched.
///
/// Consumed by the frontend to conditionally show or hide affordances that
/// only work when the user is on the same machine as the server (e.g. adding
/// a local project requires a real filesystem path).
#[but_api_macros::but_transport(register = false)]
pub struct ServerCapabilities {
    /// True when the server is reachable from outside localhost (e.g. a
    /// tunnel is active). False in the Tauri desktop app and when
    /// but-server is running locally.
    pub is_remote: bool,
    /// Whether the user can add a local project via a filesystem path.
    pub can_add_projects: bool,
}

/// Get the build type of the current GitButler build.
#[but_api]
pub fn build_type() -> Result<String> {
    let build_type = match option_env!("CHANNEL") {
        Some("release") => "release",
        Some("nightly") => "nightly",
        _ => "development",
    };

    Ok(build_type.to_string())
}
