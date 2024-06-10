use std::{collections::HashMap, path, sync, time::Duration};

use anyhow::Result;
use futures::future::join_all;
use tokio::sync::Semaphore;
use url::Url;

use crate::{
    users,
    virtual_branches::{
        Author, BaseBranch, RemoteBranchData, RemoteCommit, VirtualBranch, VirtualBranchCommit,
    },
};

#[derive(Clone)]
pub struct Proxy {
    cache_dir: path::PathBuf,

    semaphores: sync::Arc<tokio::sync::Mutex<HashMap<url::Url, Semaphore>>>,
}

impl Proxy {
    pub fn new(cache_dir: path::PathBuf) -> Self {
        Proxy {
            cache_dir,
            semaphores: sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn proxy_user(&self, user: users::User) -> users::User {
        match Url::parse(&user.picture) {
            Ok(picture) => users::User {
                picture: self.proxy(&picture).await.map_or_else(
                    |error| {
                        tracing::error!(?error, "failed to proxy user picture");
                        user.picture.clone()
                    },
                    |url| url.to_string(),
                ),
                ..user
            },
            Err(_) => user,
        }
    }

    async fn proxy_virtual_branch_commit(
        &self,
        commit: VirtualBranchCommit,
    ) -> VirtualBranchCommit {
        VirtualBranchCommit {
            author: self.proxy_author(commit.author).await,
            ..commit
        }
    }

    pub async fn proxy_virtual_branch(&self, branch: VirtualBranch) -> VirtualBranch {
        VirtualBranch {
            commits: join_all(
                branch
                    .commits
                    .iter()
                    .map(|commit| self.proxy_virtual_branch_commit(commit.clone()))
                    .collect::<Vec<_>>(),
            )
            .await,
            ..branch
        }
    }

    pub async fn proxy_virtual_branches(&self, branches: Vec<VirtualBranch>) -> Vec<VirtualBranch> {
        join_all(
            branches
                .into_iter()
                .map(|branch| self.proxy_virtual_branch(branch))
                .collect::<Vec<_>>(),
        )
        .await
    }

    pub async fn proxy_remote_branch_data(&self, branch: RemoteBranchData) -> RemoteBranchData {
        RemoteBranchData {
            commits: join_all(
                branch
                    .commits
                    .into_iter()
                    .map(|commit| self.proxy_remote_commit(commit))
                    .collect::<Vec<_>>(),
            )
            .await,
            ..branch
        }
    }

    async fn proxy_author(&self, author: Author) -> Author {
        Author {
            gravatar_url: self.proxy(&author.gravatar_url).await.unwrap_or_else(|error| {
                tracing::error!(gravatar_url = %author.gravatar_url, ?error, "failed to proxy gravatar url");
                author.gravatar_url
            }),
            ..author
        }
    }

    async fn proxy_remote_commit(&self, commit: RemoteCommit) -> RemoteCommit {
        RemoteCommit {
            author: self.proxy_author(commit.author).await,
            ..commit
        }
    }

    pub async fn proxy_base_branch(&self, base_branch: BaseBranch) -> BaseBranch {
        BaseBranch {
            recent_commits: join_all(
                base_branch
                    .clone()
                    .recent_commits
                    .into_iter()
                    .map(|commit| self.proxy_remote_commit(commit))
                    .collect::<Vec<_>>(),
            )
            .await,
            upstream_commits: join_all(
                base_branch
                    .clone()
                    .upstream_commits
                    .into_iter()
                    .map(|commit| self.proxy_remote_commit(commit))
                    .collect::<Vec<_>>(),
            )
            .await,
            ..base_branch.clone()
        }
    }

    // takes a url of a remote assets, downloads it into cache and returns a url that points to the cached file
    pub async fn proxy(&self, src: &Url) -> Result<Url> {
        #[cfg(unix)]
        if src.scheme() == "asset" {
            return Ok(src.clone());
        }

        if src.scheme() == "https" && src.host_str() == Some("asset.localhost") {
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

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;

        let resp = client.get(src.clone()).send().await?;
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

#[cfg(unix)]
fn build_asset_url(path: &str) -> Url {
    Url::parse(&format!("asset://localhost/{}", urlencoding::encode(path))).unwrap()
}

#[cfg(windows)]
fn build_asset_url(path: &str) -> Url {
    Url::parse(&format!(
        "https://asset.localhost/{}",
        urlencoding::encode(path)
    ))
    .unwrap()
}
