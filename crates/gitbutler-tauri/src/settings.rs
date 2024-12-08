use crate::error::Error;
use gitbutler_settings::{AppSettings, AppSettingsUpdateRequest};
use std::sync::Arc;
use tauri::State;
use tauri::Wry;
use tauri_plugin_store::Store;
use tracing::instrument;

pub struct SettingsStore {
    store: Arc<Store<Wry>>,
}

impl From<Arc<Store<Wry>>> for SettingsStore {
    fn from(store: Arc<Store<Wry>>) -> Self {
        Self { store }
    }
}

impl SettingsStore {
    pub fn app_settings(&self) -> AppSettings {
        AppSettings {
            app_metrics_enabled: self.get_bool("appMetricsEnabled"),
            app_error_reporting_enabled: self.get_bool("appErrorReportingEnabled"),
            app_non_anon_metrics_enabled: self.get_bool("appNonAnonMetricsEnabled"),
            app_analytics_confirmed: self.get_bool("appAnalyticsConfirmed"),
            github_oauth_client_id: self.get_string("githubOauthClientId"),
        }
    }

    fn get_bool(&self, value: &str) -> Option<bool> {
        self.store.get(value).and_then(|v| v.as_bool())
    }

    fn get_string(&self, value: &str) -> Option<String> {
        self.store
            .get(value)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
    }
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn list_app_settings(app: State<'_, super::App>) -> Result<AppSettings, Error> {
    let data_dir = &app.app_data_dir;
    AppSettings::try_from_path(data_dir).map_err(|err| err.into())
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_app_settings(
    app: State<'_, super::App>,
    request: AppSettingsUpdateRequest,
) -> Result<(), Error> {
    let data_dir = &app.app_data_dir;
    let mut settings = AppSettings::try_from_path(data_dir)?;
    settings.update(data_dir, request)?;
    Ok(())
}
