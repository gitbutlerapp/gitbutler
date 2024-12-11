#![allow(deprecated)]
mod legacy;
pub use legacy::LegacySettings;

mod app_settings;
mod json;
pub mod persistence;
pub use app_settings::AppSettings;
