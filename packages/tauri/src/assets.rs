use std::{collections::HashMap, fs, path, sync};

use anyhow::{Context, Result};
use futures::future::join_all;
use tauri::AppHandle;
use tokio::sync::Semaphore;
use url::Url;

use crate::{
    users,
    virtual_branches::{Author, BaseBranch, RemoteBranch, RemoteCommit},
};

#[derive(Clone)]
pub struct Proxy {
    cache_dir: path::PathBuf,

    semaphores: sync::Arc<tokio::sync::Mutex<HashMap<url::Url, Semaphore>>>,
}

impl From<&path::PathBuf> for Proxy {
    fn from(value: &path::PathBuf) -> Self {
        Self {
            cache_dir: value.clone(),
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
    pub async fn proxy_user(&self, user: &users::User) -> users::User {
        match Url::parse(&user.picture) {
            Ok(picture) => users::User {
                picture: self.proxy(&picture).await.map_or_else(
                    |error| {
                        tracing::error!(?error, "failed to proxy user picture");
                        user.picture.to_string()
                    },
                    |url| url.to_string(),
                ),
                ..user.clone()
            },
            Err(_) => user.clone(),
        }
    }

    pub async fn proxy_remote_branches(&self, branches: &[RemoteBranch]) -> Vec<RemoteBranch> {
        join_all(
            branches
                .iter()
                .map(|branch| self.proxy_remote_branch(branch))
                .collect::<Vec<_>>(),
        )
        .await
    }

    pub async fn proxy_remote_branch(&self, branch: &RemoteBranch) -> RemoteBranch {
        RemoteBranch {
            commits: join_all(
                branch
                    .commits
                    .iter()
                    .map(|commit| self.proxy_remote_commit(commit))
                    .collect::<Vec<_>>(),
            )
            .await,
            ..branch.clone()
        }
    }

    async fn proxy_remote_commit(&self, commit: &RemoteCommit) -> RemoteCommit {
        RemoteCommit {
            author: Author {
                gravatar_url: self
                    .proxy(&commit.author.gravatar_url)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::error!(gravatar_url = %commit.author.gravatar_url, ?error, "failed to proxy gravatar url");
                        commit.author.gravatar_url.clone()
                    }),
                ..commit.author.clone()
            },
            ..commit.clone()
        }
    }

    pub async fn proxy_base_branch(&self, base_branch: &BaseBranch) -> BaseBranch {
        BaseBranch {
            recent_commits: join_all(
                base_branch
                    .clone()
                    .recent_commits
                    .iter()
                    .map(|commit| self.proxy_remote_commit(commit))
                    .collect::<Vec<_>>(),
            )
            .await,
            upstream_commits: join_all(
                base_branch
                    .clone()
                    .upstream_commits
                    .iter()
                    .map(|commit| self.proxy_remote_commit(commit))
                    .collect::<Vec<_>>(),
            )
            .await,
            ..base_branch.clone()
        }
    }

    // takes a url of a remote assets, downloads it into cache and returns a url that points to the cached file
    pub async fn proxy(&self, src: &Url) -> Result<Url> {
        if src.scheme() == ASSET_SCHEME {
            return Ok(src.clone());
        }

        let hash = md5::compute(src.to_string());
        let path = path::Path::new(src.path());
        let ext = path
            .extension()
            .map_or("jpg", |ext| ext.to_str().unwrap_or("jpg"));
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

        tracing::debug!(url = %src, "downloading image");

        let resp = reqwest::get(src.clone()).await?;
        if !resp.status().is_success() {
            tracing::error!(url = %src, status = %resp.status(), "failed to download image");
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
