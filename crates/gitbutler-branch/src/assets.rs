use futures::future::join_all;

use crate::{VirtualBranch, VirtualBranchCommit};

#[derive(Clone)]
pub struct Proxy {
    core_proxy: gitbutler_core::assets::Proxy,
}

impl Proxy {
    pub fn new(core_proxy: gitbutler_core::assets::Proxy) -> Self {
        Proxy { core_proxy }
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

    async fn proxy_virtual_branch_commit(
        &self,
        commit: VirtualBranchCommit,
    ) -> VirtualBranchCommit {
        VirtualBranchCommit {
            author: self.core_proxy.proxy_author(commit.author).await,
            ..commit
        }
    }
}
