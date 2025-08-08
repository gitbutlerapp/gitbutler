#![allow(deprecated)]
use but_api::NoParams;
use but_api::{commands::settings, App};
use but_settings::api::{FeatureFlagsUpdate, TelemetryUpdate};
use but_settings::AppSettings;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_app_settings(app: State<'_, App>) -> Result<AppSettings, Error> {
    settings::get_app_settings(&app, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_onboarding_complete(app: State<'_, App>, update: bool) -> Result<(), Error> {
    settings::update_onboarding_complete(&app, settings::UpdateOnboardingCompleteParams { update })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_telemetry(app: State<'_, App>, update: TelemetryUpdate) -> Result<(), Error> {
    settings::update_telemetry(&app, settings::UpdateTelemetryParams { update })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_telemetry_distinct_id(
    app: State<'_, App>,
    app_distinct_id: Option<String>,
) -> Result<(), Error> {
    settings::update_telemetry_distinct_id(
        &app,
        settings::UpdateTelemetryDistinctIdParams { app_distinct_id },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_feature_flags(app: State<'_, App>, update: FeatureFlagsUpdate) -> Result<(), Error> {
    settings::update_feature_flags(&app, settings::UpdateFeatureFlagsParams { update })
}
