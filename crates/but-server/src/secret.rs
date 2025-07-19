use std::sync::Mutex;

use anyhow::Result;
use gitbutler_secret::{Sensitive, secret};
use serde_json::json;

use crate::RequestContext;

pub fn secret_get_global(
    _ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let handle = params["handle"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'handle' parameter"))?;

    let secret_value = secret::retrieve(handle, secret::Namespace::Global)?;

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

    let handle = params["handle"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'handle' parameter"))?;

    let secret_value = params["secret"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'secret' parameter"))?;

    secret::persist(
        handle,
        &Sensitive(secret_value.to_string()),
        secret::Namespace::Global,
    )?;

    Ok(json!({}))
}
