use crate::{Config, RepositoryExt};
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use git2::Oid;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub content: String,
    pub name: Option<String>,
    pub size: Option<usize>,
    pub mime_type: Option<String>,
    pub status: FileStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Normal,
    Deleted,
}

pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<String>>;
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;
    fn read_file_from_workspace(
        &self,
        commit_id: Option<Oid>,
        relative_path: &Path,
    ) -> Result<FileInfo>;
    fn read_file_from_tree(&self, oid: Option<Oid>, relative_path: &Path) -> Result<FileInfo>;
    fn read_file_from_worktree(&self, path_in_worktree: &Path) -> Result<FileInfo>;
    fn process_binary_file(
        &self,
        path_in_worktree: &Path,
        content: &[u8],
        size: usize,
    ) -> Result<FileInfo>;
}

impl RepoCommands for Project {
    fn get_local_config(&self, key: &str) -> Result<Option<String>> {
        let ctx = CommandContext::open(self)?;
        let config: Config = ctx.repository().into();
        config.get_local(key)
    }

    fn set_local_config(&self, key: &str, value: &str) -> Result<()> {
        let ctx = CommandContext::open(self)?;
        let config: Config = ctx.repository().into();
        config.set_local(key, value)
    }

    fn check_signing_settings(&self) -> Result<bool> {
        let ctx = CommandContext::open(self)?;
        let signed = ctx.repository().sign_buffer(b"test");
        match signed {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn remotes(&self) -> Result<Vec<String>> {
        let ctx = CommandContext::open(self)?;
        ctx.repository().remotes_as_string()
    }

    fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        let ctx = CommandContext::open(self)?;
        ctx.repository().remote(name, url)?;
        Ok(())
    }

    fn read_file_from_workspace(
        &self,
        commit_id: Option<Oid>,
        relative_path: &Path,
    ) -> Result<FileInfo> {
        let ctx = CommandContext::open(self)?;
        let repo = ctx.repository();
        let path_in_worktree = gix::path::realpath(self.path.join(relative_path))?;
        if !path_in_worktree.starts_with(self.path.clone()) {
            anyhow::bail!(
                "Path to read from at '{}' isn't in the worktree directory '{}'",
                relative_path.display(),
                self.path.display()
            );
        }

        if let Some(commit_oid) = commit_id {
            return self.read_file_from_tree(Some(commit_oid), relative_path);
        }

        // Check Worktree & Index
        let status = repo.status_file(relative_path)?;
        let is_deleted = status.is_wt_deleted() || status.is_index_deleted();

        if is_deleted {
            return Ok(FileInfo {
                content: String::new(),
                name: None,
                size: None,
                mime_type: None,
                status: FileStatus::Deleted,
            });
        }

        if status.is_wt_new() || status.is_wt_modified() {
            return self.read_file_from_worktree(&path_in_worktree);
        }

        // Check HEAD
        self.read_file_from_tree(None, relative_path)
    }

    fn read_file_from_tree(
        &self,
        commit_oid: Option<Oid>,
        relative_path: &Path,
    ) -> Result<FileInfo> {
        let ctx = CommandContext::open(self)?;
        let repo = ctx.repository();
        let tree_id = if let Some(id) = commit_oid {
            let commit = repo.find_commit(id)?;
            commit.tree_id()
        } else {
            repo.head()?.peel_to_tree()?.id()
        };
        let tree = repo.find_tree(tree_id)?;
        match tree.get_path(relative_path) {
            Ok(entry) => {
                let blob = repo.find_blob(entry.id())?;

                if blob.is_binary() {
                    self.process_binary_file(relative_path, blob.content(), blob.size())
                } else {
                    Ok(FileInfo {
                        content: std::str::from_utf8(blob.content())?.to_string(),
                        name: Some(
                            relative_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        ),
                        size: Some(blob.size()),
                        mime_type: None,
                        status: FileStatus::Normal,
                    })
                }
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(FileInfo {
                content: String::new(),
                name: None,
                size: None,
                mime_type: None,
                status: FileStatus::Deleted,
            }),
            Err(e) => Err(e.into()),
        }
    }

    fn read_file_from_worktree(&self, path_in_worktree: &Path) -> Result<FileInfo> {
        let content = std::fs::read(path_in_worktree)?;
        self.process_binary_file(path_in_worktree, &content, content.len())
    }

    fn process_binary_file(
        &self,
        path_in_worktree: &Path,
        content: &[u8],
        size: usize,
    ) -> Result<FileInfo> {
        let file_name = path_in_worktree
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let mut file_info = FileInfo {
            content: String::new(),
            name: Some(file_name),
            size: Some(size),
            mime_type: None,
            status: FileStatus::Normal,
        };

        if let Some(kind) = infer::get(content) {
            if infer::is_image(content) {
                let encoded_content = general_purpose::STANDARD.encode(content);
                file_info.content = encoded_content;
                file_info.mime_type = Some(kind.mime_type().to_string());
            }
        }

        Ok(file_info)
    }
}
