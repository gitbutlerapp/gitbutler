#![allow(deprecated)]
mod legacy;
pub use legacy::LegacySettings;

mod app_settings;
mod json;
mod persistence;
mod watch;
pub use app_settings::AppSettings;
pub use watch::SettingsHandle;
