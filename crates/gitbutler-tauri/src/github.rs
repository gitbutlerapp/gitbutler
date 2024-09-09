pub mod commands {
    use std::{collections::HashMap, path};

    use anyhow::{Context, Result};
    use gitbutler_fs::list_files;
    use serde::{Deserialize, Serialize};
    use tracing::instrument;

    use crate::error::Error;

    const GITHUB_CLIENT_ID: &str = "cd51880daa675d9e6452";

    #[derive(Debug, Deserialize, Serialize, Clone, Default)]
    pub struct Verification {
        pub user_code: String,
        pub device_code: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Path {
        pub value: String,
        pub label: String,
    }

    #[tauri::command(async)]
    #[instrument]
    pub async fn init_device_oauth() -> Result<Verification, Error> {
        let mut req_body = HashMap::new();
        req_body.insert("client_id", GITHUB_CLIENT_ID);
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
    #[instrument]
    pub async fn check_auth_status(device_code: &str) -> Result<String, Error> {
        #[derive(Debug, Deserialize, Serialize, Clone, Default)]
        struct AccessTokenContainer {
            access_token: String,
        }

        let mut req_body = HashMap::new();
        req_body.insert("client_id", GITHUB_CLIENT_ID);
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

    #[tauri::command(async)]
    #[instrument]
    pub fn get_available_github_pr_templates(path: &path::Path) -> Result<Vec<Path>, Error> {
        let walked_paths = list_files(path, &[path])?;

        let mut available_paths = Vec::new();
        for entry in walked_paths {
            let path_entry = entry.as_path();
            let path_str = path_entry.to_string_lossy();
            if path_str == "PULL_REQUEST_TEMPLATE.md"
                || path_str == "pull_request_template.md"
                || path_str.contains("PULL_REQUEST_TEMPLATE/")
            {
                available_paths.push(Path {
                    value: path.join(path_entry).to_string_lossy().to_string(),
                    label: path_entry.to_string_lossy().to_string(),
                });
            }
        }

        Ok(available_paths)
    }
}
