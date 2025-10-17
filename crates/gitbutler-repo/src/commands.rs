use crate::{Config, RepositoryExt, remote::GitRemote};
use anyhow::{Result, bail};
use base64::engine::Engine as _;
use git2::Oid;
use gitbutler_project::Project;
use infer::MatcherType;
use itertools::Itertools;
use serde::Serialize;
use std::path::Path;
use tracing::warn;

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
// TODO: turn this whole struct into an enum, it's : everything is an option style tells us that.
pub struct FileInfo {
    /// If `None`, this means the file was deleted or has no meaningful content.
    /// If `Some` and `size` is `Some`, then the `content` is base64 encoded, and
    /// `size` is its decoded size.
    pub content: Option<String>,
    /// The basename as derived from the relative filepath.
    pub file_name: String,
    /// The size of the content in bytes, which is always set unless the file is deleted.
    pub size: Option<usize>,
    /// If `None`, it's considered a text file. Otherwise, it's a binary file with the given
    /// inferred mimetype.
    pub mime_type: Option<String>,
}

impl FileInfo {
    pub fn deleted() -> Self {
        Self::default()
    }

    pub fn from_content(path_in_worktree: &Path, content: &[u8]) -> Self {
        if Self::is_binary(content) {
            FileInfo::image_or_empty(path_in_worktree, content)
        } else {
            FileInfo::utf8_text_or_binary(path_in_worktree, content)
        }
    }

    /// Create a new instance for if content is text.
    /// Note that UTF8 is assumed, or else the file will be considered binary.
    pub fn utf8_text_or_binary(path_in_worktree: &Path, content: &[u8]) -> Self {
        FileInfo {
            content: std::str::from_utf8(content).map(ToOwned::to_owned).ok(),
            file_name: Self::file_name_str(path_in_worktree),
            size: Some(content.len()),
            mime_type: None,
        }
    }

    /// No content is provided, just the path and the size, denoting a binary file.
    pub fn binary(path_in_worktree: &Path, len: u64) -> Self {
        FileInfo {
            content: None,
            file_name: Self::file_name_str(path_in_worktree),
            size: Some(len as usize),
            mime_type: None,
        }
    }

    /// Create an instance from `path_in_worktree` and what looks like binary `content`.
    /// If the content type can be inferred *and* is an image, the `content` field of the returned instance
    /// will be set as base64 encoded string.
    pub fn image_or_empty(path_in_worktree: &Path, content: &[u8]) -> Self {
        let mut file_info = FileInfo {
            content: None,
            file_name: Self::file_name_str(path_in_worktree),
            size: Some(content.len()),
            mime_type: None,
        };

        let kind = infer::get(content);
        if let Some(mime_type) = kind.and_then(|kind| {
            (kind.matcher_type() == MatcherType::Image).then_some(kind.mime_type())
        }) {
            let base64_content = base64::engine::general_purpose::STANDARD.encode(content);
            file_info.content = Some(base64_content);
            file_info.mime_type = Some(mime_type.to_owned())
        }
        file_info
    }

    fn file_name_str(path: &Path) -> String {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    fn is_binary(content: &[u8]) -> bool {
        let partial_content = &content[..content.len().min(8000)];
        gix::filter::plumbing::eol::Stats::from_bytes(partial_content).is_binary()
    }
}

pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<GitRemote>>;
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;

    /// Read `path` from the tree of the given commit.
    ///
    /// Bails when given an absolute path since that would suggest we are looking for a file in
    /// the workspace. Returns `FileInfo::default()` if file could not be found.
    fn read_file_from_commit(&self, commit_id: Oid, path: &Path) -> Result<FileInfo>;

    /// Read `path` in the following order:
    ///
    /// * worktree
    /// * index
    /// * `HEAD^{tree}`
    ///
    /// This order makes sense if you imagine that deleted files are shown, like in a `git status`,
    /// so we want to know what's deleted.
    ///
    /// Returns `FileInfo::default()` if file could not be found.
    fn read_file_from_workspace(&self, path: &Path) -> Result<FileInfo>;
}

impl RepoCommands for Project {
    fn get_local_config(&self, key: &str) -> Result<Option<String>> {
        let repo = &git2::Repository::open(&self.path)?;
        let config: Config = repo.into();
        config.get_local(key)
    }

    fn set_local_config(&self, key: &str, value: &str) -> Result<()> {
        let repo = &git2::Repository::open(&self.path)?;
        let config: Config = repo.into();
        config.set_local(key, value)
    }

    fn check_signing_settings(&self) -> Result<bool> {
        let repo = &git2::Repository::open(&self.path)?;
        let signed = repo.sign_buffer(b"test");
        match signed {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn remotes(&self) -> anyhow::Result<Vec<GitRemote>> {
        let repo = &git2::Repository::open(&self.path)?;
        let remotes = repo
            .remotes_as_string()?
            .iter()
            .map(|name| repo.find_remote(name))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|remote| remote.into())
            .collect_vec();
        Ok(remotes)
    }

    fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        let repo = &git2::Repository::open(&self.path)?;

        // Bail if remote with given name already exists.
        if repo.find_remote(name).is_ok() {
            bail!("Remote name '{}' already exists", name);
        }

        // Bail if remote with given url already exists.
        if repo
            .remotes_as_string()?
            .iter()
            .map(|name| repo.find_remote(name))
            .any(|result| result.is_ok_and(|remote| remote.url() == Some(url)))
        {
            bail!("Remote with url '{}' already exists", url);
        }

        repo.remote(name, url)?;
        Ok(())
    }

    fn read_file_from_commit(&self, commit_id: Oid, relative_path: &Path) -> Result<FileInfo> {
        if !relative_path.is_relative() {
            bail!(
                "Refusing to read '{:?}' from commit {:?} as it's not relative to the worktree",
                relative_path,
                commit_id
            );
        }

        let repo = &git2::Repository::open(&self.path)?;
        let tree = repo.find_commit(commit_id)?.tree()?;

        Ok(match tree.get_path(relative_path) {
            Ok(entry) => {
                let blob = repo.find_blob(entry.id())?;
                FileInfo::from_content(relative_path, blob.content())
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => FileInfo::deleted(),
            Err(e) => return Err(e.into()),
        })
    }

    fn read_file_from_workspace(&self, probably_relative_path: &Path) -> Result<FileInfo> {
        let repo = &git2::Repository::open(&self.path)?;

        let (path_in_worktree, relative_path) = if probably_relative_path.is_relative() {
            (
                gix::path::realpath(self.path.join(probably_relative_path))?,
                probably_relative_path.to_owned(),
            )
        } else {
            let Ok(relative_path) = probably_relative_path.strip_prefix(&self.path) else {
                bail!(
                    "Path to read from at '{}' isn't in the worktree directory '{}'",
                    probably_relative_path.display(),
                    self.path.display()
                );
            };
            (probably_relative_path.to_owned(), relative_path.to_owned())
        };

        Ok(match path_in_worktree.symlink_metadata() {
            Ok(md) if md.is_file() => {
                let content = std::fs::read(path_in_worktree)?;
                FileInfo::from_content(&relative_path, &content)
            }
            Ok(md) if md.is_symlink() => {
                let content = std::fs::read_link(&path_in_worktree)?;
                FileInfo::utf8_text_or_binary(&relative_path, &gix::path::into_bstr(content))
            }
            Ok(unsupported) => {
                warn!(
                    ?relative_path,
                    "Path can't be read as its type isn't supported, default to binary",
                );
                FileInfo::binary(&relative_path, unsupported.len())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                match repo.index()?.get_path(&relative_path, 0) {
                    // Read file that has been deleted and not staged for commit.
                    Some(entry) => {
                        let blob = repo.find_blob(entry.id)?;
                        FileInfo::from_content(&relative_path, blob.content())
                    }
                    // Read file that has been deleted and staged for commit. Note that file not
                    // found returns FileInfo::default() rather than an error.
                    None => self.read_file_from_commit(
                        repo.head()?.peel_to_commit()?.id(),
                        &relative_path,
                    )?,
                }
            }
            Err(err) => return Err(err.into()),
        })
    }
}
