use crate::{Config, RepositoryExt};
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
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
}
pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<String>>;
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;
    fn read_file_from_workspace(&self, relative_path: &Path) -> Result<FileInfo>;
    fn read_untracked_file(&self, path_in_worktree: &Path) -> Result<FileInfo>;
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

    fn read_file_from_workspace(&self, relative_path: &Path) -> Result<FileInfo> {
        let ctx = CommandContext::open(self)?;
        let path_in_worktree = gix::path::realpath(self.path.join(relative_path))?;
        if !path_in_worktree.starts_with(self.path.clone()) {
            anyhow::bail!(
                "Path to read from at '{}' isn't in the worktree directory '{}'",
                relative_path.display(),
                self.path.display()
            );
        }

        let status = ctx.repository().status_file(relative_path)?;

        if status.is_wt_new() {
            return self.read_untracked_file(&path_in_worktree);
        }

        let tree = ctx.repository().head()?.peel_to_tree()?;
        let entry = tree.get_path(relative_path)?;
        let blob = ctx.repository().find_blob(entry.id())?;

        if !blob.is_binary() {
            let content = std::str::from_utf8(blob.content())?.to_string();
            Ok(FileInfo {
                name: None,
                size: None,
                content,
                mime_type: None,
            })
        } else {
            self.process_binary_file(&path_in_worktree, blob.content(), blob.size())
        }
    }

    fn read_untracked_file(&self, path_in_worktree: &Path) -> Result<FileInfo> {
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
            name: Some(file_name),
            size: Some(size),
            content: String::new(),
            mime_type: None,
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
