use std::{collections::HashMap, path, sync};

use anyhow::Result;
use tokio::sync::Semaphore;
use url::Url;

pub struct Proxy {
    cache_dir: path::PathBuf,

    semaphores: sync::Arc<tokio::sync::Mutex<HashMap<url::Url, Semaphore>>>,
}

const ASSET_SCHEME: &str = "asset";

impl Proxy {
    pub fn new<P: AsRef<path::Path>>(cache_dir: P) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            semaphores: sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    // takes a url of a remote assets, downloads it into cache and returns a url that points to the cached file
    pub async fn proxy(&self, src: &Url) -> Result<Url> {
        if src.scheme() == ASSET_SCHEME {
            return Ok(src.clone());
        }

        let hash = md5::compute(src.to_string());
        let path = path::Path::new(src.path());
        let ext = path.extension().unwrap().to_str().unwrap();
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

        log::info!("Downloading image {}", src);

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
