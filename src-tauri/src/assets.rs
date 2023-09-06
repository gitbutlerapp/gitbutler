use std::{collections::HashMap, fs, path, sync};

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::sync::Semaphore;
use url::Url;

#[derive(Clone)]
pub struct Proxy {
    cache_dir: path::PathBuf,

    semaphores: sync::Arc<tokio::sync::Mutex<HashMap<url::Url, Semaphore>>>,
}

impl From<&path::PathBuf> for Proxy {
    fn from(value: &path::PathBuf) -> Self {
        Self {
            cache_dir: value.to_path_buf(),
            semaphores: sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
}

impl TryFrom<&AppHandle> for Proxy {
    type Error = anyhow::Error;

    fn try_from(handle: &AppHandle) -> Result<Self, Self::Error> {
        let app_cache_dir = handle
            .path_resolver()
            .app_cache_dir()
            .context("failed to get cache dir")?;
        fs::create_dir_all(&app_cache_dir).context("failed to create cache dir")?;
        let cache_dir = app_cache_dir.join("images");

        Ok(Self::from(&cache_dir))
    }
}

const ASSET_SCHEME: &str = "asset";

impl Proxy {
    // takes a url of a remote assets, downloads it into cache and returns a url that points to the cached file
    pub async fn proxy(&self, src: &Url) -> Result<Url> {
        if src.scheme() == ASSET_SCHEME {
            return Ok(src.clone());
        }

        let hash = md5::compute(src.to_string());
        let path = path::Path::new(src.path());
        let ext = path
            .extension()
            .map(|ext| ext.to_str().unwrap_or("jpg"))
            .unwrap_or("jpg");
        let save_to = self.cache_dir.join(format!("{:X}.{}", hash, ext));

        if save_to.exists() {
            return Ok(build_asset_url(&save_to.display().to_string()));
        }

        // only one download per url at a time
        let mut semaphores = self.semaphores.lock().await;
        let r = semaphores
            .entry(src.clone())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = r.acquire().await?;

        if save_to.exists() {
            // check again, maybe url was downloaded
            return Ok(build_asset_url(&save_to.display().to_string()));
        }

        tracing::debug!("Downloading image {}", src);

        let resp = reqwest::get(src.clone()).await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to download image {}: {}",
                src,
                resp.status()
            ));
        }

        let bytes = resp.bytes().await?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::write(&save_to, bytes)?;

        Ok(build_asset_url(&save_to.display().to_string()))
    }
}

fn build_asset_url(path: &str) -> Url {
    Url::parse(&format!(
        "{}://localhost/{}",
        ASSET_SCHEME,
        urlencoding::encode(path)
    ))
    .unwrap()
}
