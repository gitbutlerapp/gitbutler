use crate::{Config, RepositoryExt};
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use std::collections::HashMap;
use std::path::Path;

pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<String>>;
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;
    fn read_file_from_workspace(&self, relative_path: &Path) -> Result<HashMap<String, String>>;
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

    fn read_file_from_workspace(&self, relative_path: &Path) -> Result<HashMap<String, String>> {
        let ctx = CommandContext::open(self)?;
        if self
            .path
            .join(relative_path)
            .canonicalize()?
            .as_path()
            .starts_with(self.path.clone())
        {
            let tree = ctx.repository().head()?.peel_to_tree()?;
            let entry = tree.get_path(relative_path)?;
            let blob = ctx.repository().find_blob(entry.id())?;

            let mut file_info = HashMap::new();

            if !blob.is_binary() {
                let content = std::str::from_utf8(blob.content())?.to_string();
                file_info.insert("content".to_string(), content);
            } else {
                let file_name = relative_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let extension = relative_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();

                file_info.insert("name".to_string(), file_name);
                file_info.insert("size".to_string(), blob.size().to_string());

                let image_types = [
                    ("jpg", "image/jpeg"),
                    ("jpeg", "image/jpeg"),
                    ("png", "image/png"),
                    ("gif", "image/gif"),
                    ("svg", "image/svg+xml"),
                    ("webp", "image/webp"),
                    ("bmp", "image/bmp"),
                ];

                if let Some(&(_, mime_type)) =
                    image_types.iter().find(|&&(ext, _)| ext == extension)
                {
                    let binary_content = blob.content();
                    let encoded_content = general_purpose::STANDARD.encode(binary_content);
                    file_info.insert("content".to_string(), encoded_content);
                    file_info.insert("mimeType".to_string(), mime_type.to_string());
                }
            }

            Ok(file_info)
        } else {
            anyhow::bail!("Invalid workspace file");
        }
    }
}
