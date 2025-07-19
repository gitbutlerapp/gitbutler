use crate::RequestContext;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub user_code: String,
    pub device_code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckAuthParams {
    device_code: String,
}

pub async fn init_device_oauth(
    ctx: &RequestContext,
    _params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let mut req_body = HashMap::new();
    let client_id = ctx
        .app_settings
        .get()?
        .github_oauth_app
        .oauth_client_id
        .clone();
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

    let verification: Verification =
        serde_json::from_str(&rsp_body).context("Failed to parse response body")?;

    Ok(serde_json::to_value(verification)?)
}

pub async fn check_auth_status(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    struct AccessTokenContainer {
        access_token: String,
    }

    let params: CheckAuthParams = serde_json::from_value(params)?;

    let mut req_body = HashMap::new();
    let client_id = ctx
        .app_settings
        .get()?
        .github_oauth_app
        .oauth_client_id
        .clone();
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

    let token_container: AccessTokenContainer =
        serde_json::from_str(&rsp_body).context("Failed to parse response body")?;

    Ok(serde_json::to_value(token_container.access_token)?)
}
