#![allow(deprecated)]
use anyhow::Result;
use but_settings::api::FeatureFlagsUpdate;
use but_settings::api::TelemetryUpdate;
use but_settings::AppSettings;
use but_settings::AppSettingsWithDiskSync;
use but_settings::LegacySettings;
use std::sync::Arc;
use tauri::State;
use tauri::Wry;
use tauri_plugin_store::Store;
use tracing::instrument;

use crate::error::Error;

pub struct SettingsStore {
    store: Arc<Store<Wry>>,
}

impl From<Arc<Store<Wry>>> for SettingsStore {
    fn from(store: Arc<Store<Wry>>) -> Self {
        Self { store }
    }
}

impl SettingsStore {
    pub fn app_settings(&self) -> LegacySettings {
        LegacySettings {
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
#[instrument(skip(handle), err(Debug))]
pub fn get_app_settings(handle: State<'_, AppSettingsWithDiskSync>) -> Result<AppSettings, Error> {
    Ok(handle.get()?.clone())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn update_onboarding_complete(
    handle: State<'_, AppSettingsWithDiskSync>,
    update: bool,
) -> Result<(), Error> {
    handle
        .update_onboarding_complete(update)
        .map_err(|e| e.into())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn update_telemetry(
    handle: State<'_, AppSettingsWithDiskSync>,
    update: TelemetryUpdate,
) -> Result<(), Error> {
    handle.update_telemetry(update).map_err(|e| e.into())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn update_feature_flags(
    handle: State<'_, AppSettingsWithDiskSync>,
    update: FeatureFlagsUpdate,
) -> Result<(), Error> {
    handle.update_feature_flags(update).map_err(|e| e.into())
}
