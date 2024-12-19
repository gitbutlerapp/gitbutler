#![allow(deprecated)]
use anyhow::Result;
use gitbutler_settings::api;
use gitbutler_settings::api::TelemetryUpdate;
use gitbutler_settings::AppSettings;
use gitbutler_settings::LegacySettings;
use gitbutler_settings::SettingsHandle;
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
pub fn get_app_settings(handle: State<'_, SettingsHandle>) -> Result<AppSettings, Error> {
    api::get_app_settings(&handle).map_err(|e| e.into())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn update_onboarding_complete(
    handle: State<'_, SettingsHandle>,
    update: bool,
) -> Result<(), Error> {
    api::update_onboarding_complete(&handle, update).map_err(|e| e.into())
}

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn update_telemetry(
    handle: State<'_, SettingsHandle>,
    update: TelemetryUpdate,
) -> Result<(), Error> {
    api::update_telemetry(&handle, update).map_err(|e| e.into())
}
