#![allow(deprecated)]
use gitbutler_settings::LegacySettings;
use std::sync::Arc;
use tauri::Wry;
use tauri_plugin_store::Store;

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
