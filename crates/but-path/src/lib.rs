use std::path::PathBuf;

pub fn app_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(test_dir) = std::env::var("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("com.gitbutler.app"));
    }
    dirs::data_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join(identifier()))
}

pub fn app_config_dir() -> anyhow::Result<PathBuf> {
    if let Ok(test_dir) = std::env::var("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("gitbutler"));
    }
    dirs::data_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join("gitbutler"))
}

fn identifier() -> &'static str {
    option_env!("IDENTIFIER").unwrap_or_else(|| {
        if let Some(channel) = option_env!("CHANNEL") {
            match channel {
                "nightly" => "com.gitbutler.app.nightly",
                "release" => "com.gitbutler.app",
                _ => "com.gitbutler.app.dev",
            }
        } else {
            "com.gitbutler.app.dev"
        }
    })
}
