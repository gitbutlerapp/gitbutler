use anyhow::Result;
use but_api_macros::but_api;

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
