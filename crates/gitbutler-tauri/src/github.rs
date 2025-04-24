pub mod commands {
    use but_settings::AppSettingsWithDiskSync;
    use std::collections::HashMap;
    use tauri::State;

    use anyhow::{Context, Result};
    use serde::{Deserialize, Serialize};
    use tracing::instrument;

    use crate::error::Error;

    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    pub struct Verification {
        pub user_code: String,
        pub device_code: String,
    }

    #[tauri::command(async)]
    #[instrument(skip(settings), err(Debug))]
    pub async fn init_device_oauth(
        settings: State<'_, AppSettingsWithDiskSync>,
    ) -> Result<Verification, Error> {
        let mut req_body = HashMap::new();
        let client_id = settings.get()?.github_oauth_app.oauth_client_id.clone();
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

    #[tauri::command(async)]
    #[instrument(skip(settings), err(Debug))]
    pub async fn check_auth_status(
        settings: State<'_, AppSettingsWithDiskSync>,
        device_code: &str,
    ) -> Result<String, Error> {
        #[derive(Debug, Deserialize, Serialize, Clone, Default)]
        struct AccessTokenContainer {
            access_token: String,
        }

        let mut req_body = HashMap::new();
        let client_id = settings.get()?.github_oauth_app.oauth_client_id.clone();
        req_body.insert("client_id", client_id.as_str());
        req_body.insert("device_code", device_code);
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
}
