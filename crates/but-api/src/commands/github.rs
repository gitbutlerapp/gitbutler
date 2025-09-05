//! In place of commands.rs
use anyhow::{Context, Result};
use but_settings::AppSettings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{NoParams, error::Error};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Verification {
    pub user_code: String,
    pub device_code: String,
}

pub async fn init_device_oauth(_params: NoParams) -> Result<Verification, Error> {
    let mut req_body = HashMap::new();
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let client_id = app_settings.github_oauth_app.oauth_client_id.clone();
    req_body.insert("client_id", client_id.as_str());
    req_body.insert("scope", "repo");

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let client = reqwest::Client::new();
    let res = client
        .post("https://github.com/login/device/code")
        .headers(headers)
        .json(&req_body)
        .send()
        .await
        .context("Failed to send request")?;

    let rsp_body = res.text().await.context("Failed to get response body")?;

    serde_json::from_str(&rsp_body)
        .context("Failed to parse response body")
        .map_err(Into::into)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAuthStatusParams {
    pub device_code: String,
}

pub async fn check_auth_status(params: CheckAuthStatusParams) -> Result<String, Error> {
    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    struct AccessTokenContainer {
        access_token: String,
    }

    let mut req_body = HashMap::new();
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let client_id = app_settings.github_oauth_app.oauth_client_id.clone();
    req_body.insert("client_id", client_id.as_str());
    req_body.insert("device_code", params.device_code.as_str());
    req_body.insert("grant_type", "urn:ietf:params:oauth:grant-type:device_code");

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let client = reqwest::Client::new();
    let res = client
        .post("https://github.com/login/oauth/access_token")
        .headers(headers)
        .json(&req_body)
        .send()
        .await
        .context("Failed to send request")?;

    let rsp_body = res.text().await.context("Failed to get response body")?;

    serde_json::from_str::<AccessTokenContainer>(&rsp_body)
        .map(|rsp_body| rsp_body.access_token)
        .context("Failed to parse response body")
        .map_err(Into::into)
}
