use anyhow::Result;
use serde_json::json;

use crate::RequestContext;

pub fn get_app_settings(ctx: &RequestContext) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(ctx.app_settings.get()?.clone())?)
}

pub fn update_onboarding_complete(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let update = params["update"].as_bool().unwrap();
    ctx.app_settings.update_onboarding_complete(update)?;
    Ok(json!({}))
}

pub fn update_telemetry(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let update = serde_json::from_value(params["update"].clone())?;
    ctx.app_settings.update_telemetry(update)?;
    Ok(json!({}))
}

pub fn update_telemetry_distinct_id(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let app_distinct_id = params["app_distinct_id"].as_str().map(|s| s.to_string());
    ctx.app_settings
        .update_telemetry_distinct_id(app_distinct_id)?;
    Ok(json!({}))
}

pub fn update_feature_flags(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let update = serde_json::from_value(params["update"].clone())?;
    ctx.app_settings.update_feature_flags(update)?;
    Ok(json!({}))
}
