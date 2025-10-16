//! In place of commands.rs
use anyhow::Result;
use but_github::{CheckAuthStatusParams, Verification};

use crate::{NoParams, error::Error};

pub async fn init_device_oauth(_params: NoParams) -> Result<Verification, Error> {
    but_github::init_device_oauth().await.map_err(Into::into)
}

pub async fn check_auth_status(params: CheckAuthStatusParams) -> Result<String, Error> {
    but_github::check_auth_status(params)
        .await
        .map_err(Into::into)
}
