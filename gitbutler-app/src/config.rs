use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// Configuration loaded from `$config_dir/gitbutler.toml`
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
}

/// Custom theme configuration for the app
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Theme {
    pub base: ThemeBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core_ntrl_100: Option<String>,
}

/// The base theme on which styles are applied
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeBase {
    /// Theme modifications are based on the light theme
    Light,
    /// Theme modifications are based on the dark theme
    Dark,
}

/// Errors that may arise when reading/writing config files
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error: {1}: {0}")]
    Io(String, std::io::Error),
    #[error("toml error: {0}")]
    Toml(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

/// Loads the configuration from `$config_dir/gitbutler.toml`
#[tauri::command(async)]
pub async fn load_config(handle: AppHandle) -> Result<Config, Error> {
    let Some(config_dir) = handle.path_resolver().app_config_dir() else {
        tracing::warn!("failed to get app config dir (none returned)");
        return Ok(Config::default());
    };

    let config_path = config_dir.join("gitbutler.toml");

    match tokio::fs::read_to_string(&config_path).await {
        Ok(contents) => {
            tracing::debug!("loaded config from: {}", config_path.display());
            toml::from_str(&contents).map_err(|err| Error::Toml(err.to_string()))
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                tracing::warn!("config file not found: {}", config_path.display());
                let config = Config::default();

                // Write it back out, only warning if it fails, returning the default config
                // in either case
                if let Err(err) =
                    tokio::fs::write(&config_path, toml::to_string(&config).unwrap()).await
                {
                    tracing::warn!("failed to write default config: {}", err);
                }

                Ok(config)
            } else {
                Err(Error::Io(config_path.display().to_string(), err))
            }
        }
    }
}
