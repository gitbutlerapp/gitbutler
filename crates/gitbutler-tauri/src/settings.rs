use gitbutler_settings::AppSettings;
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
    pub fn app_settings(&self) -> AppSettings {
        AppSettings {
            app_metrics_enabled: self.get_bool("appMetricsEnabled"),
            app_error_reporting_enabled: self.get_bool("appErrorReportingEnabled"),
            app_non_anon_metrics_enabled: self.get_bool("appNonAnonMetricsEnabled"),
            app_analytics_confirmed: self.get_bool("appAnalyticsConfirmed"),
        }
    }

    fn get_bool(&self, value: &str) -> Option<bool> {
        self.store.get(value).and_then(|v| v.as_bool())
    }
}
