use std::sync::Mutex;

use anyhow::Result;
use gitbutler_secret::{Sensitive, secret};
use serde::Deserialize;
use serde_json::json;

use crate::RequestContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretGetGlobalParams {
    handle: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretSetGlobalParams {
    handle: String,
    secret: String,
}

pub fn secret_get_global(
    _ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: SecretGetGlobalParams = serde_json::from_value(params)?;

    let secret_value = secret::retrieve(&params.handle, secret::Namespace::Global)?;

    match secret_value {
        Some(s) => Ok(json!(s.0)),
        None => Ok(json!(null)),
    }
}

pub fn secret_set_global(
    _ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();

    let params: SecretSetGlobalParams = serde_json::from_value(params)?;

    secret::persist(
        &params.handle,
        &Sensitive(params.secret),
        secret::Namespace::Global,
    )?;

    Ok(json!({}))
}
