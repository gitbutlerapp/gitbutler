use anyhow::Result;
use but_api_macros::but_api;

/// Capabilities that vary by how the backend was launched.
///
/// Consumed by the frontend to conditionally show or hide affordances that
/// only work when the user is on the same machine as the server (e.g. adding
/// a local project requires a real filesystem path).
#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// True when the server is reachable from outside localhost (e.g. a
    /// tunnel is active). False in the Tauri desktop app and when
    /// but-server is running locally.
    pub is_remote: bool,
    /// Whether the user can add a local project via a filesystem path.
    pub can_add_projects: bool,
}

/// Describes how the application startup environment was initialized.
#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "napi", napi_derive::napi(string_enum = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub enum ApplicationEnvironment {
    /// The environment inherited from a terminal launch was preserved.
    Inherited,
    /// The user's interactive login-shell environment was imported.
    Imported,
    /// The login-shell environment was unavailable, so the inherited environment was preserved.
    Unavailable,
}

impl From<but_core::cmd::ApplicationEnvironment> for ApplicationEnvironment {
    fn from(value: but_core::cmd::ApplicationEnvironment) -> Self {
        match value {
            but_core::cmd::ApplicationEnvironment::Inherited => Self::Inherited,
            but_core::cmd::ApplicationEnvironment::Imported => Self::Imported,
            but_core::cmd::ApplicationEnvironment::Unavailable => Self::Unavailable,
        }
    }
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

/// Initialize the environment inherited by backend child processes.
///
/// Applications embedding the SDK must call this once during startup, before invoking other
/// backend APIs or launching child processes.
#[but_api(napi)]
pub fn init_application_environment() -> Result<ApplicationEnvironment> {
    let outcome = but_core::cmd::initialize_application_environment();
    tracing::info!(?outcome, "Initialized application environment");
    Ok(outcome.into())
}

/// Initialize the secret namespace used by build-kind scoped credentials.
///
/// Applications embedding the SDK should call this once during startup before
/// invoking APIs that read or write forge credentials. If `identifier` is
/// `None`, the namespace defaults to the SDK's compiled GitButler app channel.
#[but_api(napi)]
pub fn init_application_namespace(identifier: Option<String>) -> Result<()> {
    let identifier = identifier.unwrap_or_else(|| but_path::identifier().to_string());
    but_secret::secret::set_application_namespace(identifier);
    Ok(())
}
