use gitbutler_secret::{Sensitive, secret};
use serde::Deserialize;
use std::sync::Mutex;

use crate::error::Error;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretGetGlobalParams {
    pub handle: String,
}

pub fn secret_get_global(params: SecretGetGlobalParams) -> Result<Option<String>, Error> {
    Ok(secret::retrieve(&params.handle, secret::Namespace::Global)?.map(|s| s.0))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretSetGlobalParams {
    pub handle: String,
    pub secret: String,
}

pub fn secret_set_global(params: SecretSetGlobalParams) -> Result<(), Error> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    Ok(secret::persist(
        &params.handle,
        &Sensitive(params.secret),
        secret::Namespace::Global,
    )?)
}
