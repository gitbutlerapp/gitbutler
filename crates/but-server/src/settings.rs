use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

use crate::RequestContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateOnboardingCompleteParams {
    update: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTelemetryParams {
    update: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTelemetryDistinctIdParams {
    app_distinct_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFeatureFlagsParams {
    update: serde_json::Value,
}

pub fn get_app_settings(ctx: &RequestContext) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(ctx.app_settings.get()?.clone())?)
}

pub fn update_onboarding_complete(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: UpdateOnboardingCompleteParams = serde_json::from_value(params)?;
    ctx.app_settings.update_onboarding_complete(params.update)?;
    Ok(json!({}))
}

pub fn update_telemetry(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: UpdateTelemetryParams = serde_json::from_value(params)?;
    let update = serde_json::from_value(params.update)?;
    ctx.app_settings.update_telemetry(update)?;
    Ok(json!({}))
}

pub fn update_telemetry_distinct_id(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: UpdateTelemetryDistinctIdParams = serde_json::from_value(params)?;
    ctx.app_settings
        .update_telemetry_distinct_id(params.app_distinct_id)?;
    Ok(json!({}))
}

pub fn update_feature_flags(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: UpdateFeatureFlagsParams = serde_json::from_value(params)?;
    let update = serde_json::from_value(params.update)?;
    ctx.app_settings.update_feature_flags(update)?;
    Ok(json!({}))
}
