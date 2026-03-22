use std::{path::Path, sync::Mutex};

use anyhow::{Result, bail};
use base64::engine::Engine as _;
use but_core::commit::sign_buffer;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt as _, OidExt as _};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use git2::Oid;
use ignore::WalkBuilder;
use infer::MatcherType;
use itertools::Itertools;
use serde::Serialize;
use tracing::warn;

use crate::{RepositoryExt, remote::GitRemote};

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

/// Computes a match score for a file path against a search pattern.
///
/// Returns `None` if the file doesn't match the pattern or if the pattern is empty.
/// For empty patterns, returns a score of 0 to include all files.
///
/// The score is calculated based on:
/// - Base fuzzy match score
/// - Bonus for exact filename matches (case-insensitive)
/// - Bonus for filename prefix matches (case-insensitive)
/// - Bonus for matches in the filename itself
/// - Penalty for deeply nested files
fn compute_match_score(
    matcher: &SkimMatcherV2,
    path_str: &str,
    relative_path: &Path,
    pattern: &str,
) -> Option<i64> {
    if pattern.is_empty() {
        return Some(0);
    }

    let base_score = matcher.fuzzy_match(path_str, pattern)?;
    let filename = relative_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    let filename_bonus = calculate_filename_bonus(matcher, filename, pattern);
    let depth_penalty = (relative_path.components().count() as i64) * 50;
    let adjusted_score = base_score + filename_bonus - depth_penalty;

    (adjusted_score > 0).then_some(adjusted_score)
}

/// Calculates a bonus score based on how well the pattern matches the filename.
fn calculate_filename_bonus(matcher: &SkimMatcherV2, filename: &str, pattern: &str) -> i64 {
    if filename.eq_ignore_ascii_case(pattern) {
        10000 // Exact match
    } else if filename.to_lowercase().starts_with(&pattern.to_lowercase()) {
        5000 // Prefix match
    } else {
        matcher
            .fuzzy_match(filename, pattern)
            .map(|score| score / 2)
            .unwrap_or(0)
    }
}

pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<GitRemote>>;
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
    /// `path` can be a repository-relative path, or a path that is within the
    /// worktree of the current repository.
    ///
    /// Returns `FileInfo::default()` if file could not be found.
    fn read_file_from_workspace(&self, path: &Path) -> Result<FileInfo>;

    /// Find files in the repository that match the given search query.
    ///
    /// Uses fuzzy matching similar to VSCode's file search.
    /// Returns up to `limit` file paths relative to the repository root, sorted by match quality.
    /// The search respects `.gitignore` rules and excludes the `.git` directory.
    fn find_files(&self, query: &str, limit: usize) -> Result<Vec<String>>;
}

impl RepoCommands for Context {
    fn check_signing_settings(&self) -> Result<bool> {
        let repo = self.repo.get()?;
        match sign_buffer(&repo, b"test") {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn remotes(&self) -> anyhow::Result<Vec<GitRemote>> {
        let repo = self.repo.get()?;
        repo.remote_names()
            .iter()
            .map(|name| -> Result<_> {
                let remote = repo.find_remote(name.as_ref())?;
                Ok(GitRemote::from_gix(name.to_string(), &remote))
            })
            .collect()
    }

    fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        let repo = self.git2_repo.get()?;

        // Bail if remote with given name already exists.
        if repo.find_remote(name).is_ok() {
            bail!("Remote name '{name}' already exists");
        }

        // Bail if remote with given url already exists.
        if repo
            .remotes_as_string()?
            .iter()
            .map(|name| repo.find_remote(name))
            .any(|result| result.is_ok_and(|remote| remote.url() == Some(url)))
        {
            bail!("Remote with url '{url}' already exists");
        }

        repo.remote(name, url)?;
        Ok(())
    }

    fn read_file_from_commit(&self, commit_id: Oid, relative_path: &Path) -> Result<FileInfo> {
        if !relative_path.is_relative() {
            bail!(
                "Refusing to read '{relative_path:?}' from commit {commit_id:?} as it's not relative to the worktree"
            );
        }

        let repo = self.repo.get()?;
        let tree = repo.find_commit(commit_id.to_gix())?.tree()?;

        Ok(match tree.lookup_entry_by_path(relative_path)? {
            Some(entry) => {
                let blob = repo.find_blob(entry.id())?;
                FileInfo::from_content(relative_path, &blob.data)
            }
            None => FileInfo::deleted(),
        })
    }

    /// Note that `path` can be relative or absolute, and we must validate that it's in the worktree.
    fn read_file_from_workspace(&self, path: &Path) -> Result<FileInfo> {
        let workdir = self.workdir_or_fail()?;
        let canonical_workdir = gix::path::realpath(&workdir)?;
        let path = gix::path::realpath(canonical_workdir.join(path))?;
        // Double-check that the path is still in the worktree - this might not be the case
        // if it was aboslute to begin with, or leads through symlinks.
        let relative_path = match path.strip_prefix(&canonical_workdir) {
            Ok(relative_path) => relative_path.to_owned(),
            Err(_) => {
                bail!(
                    "Path to read from at '{}' isn't in the worktree directory '{}'",
                    path.display(),
                    canonical_workdir.display()
                );
            }
        };

        let out = match path.symlink_metadata() {
            Ok(md) => {
                if md.is_file() {
                    let content = std::fs::read(&path)?;
                    FileInfo::from_content(&relative_path, &content)
                } else if md.is_symlink() {
                    let content = std::fs::read_link(&path)?;
                    FileInfo::utf8_text_or_binary(&relative_path, &gix::path::into_bstr(content))
                } else if md.is_dir() {
                    bail!(
                        "Path to read from at '{}' is a directory",
                        relative_path.display(),
                    );
                } else {
                    warn!(
                        ?relative_path,
                        "Path can't be read as its type isn't supported, default to binary",
                    );
                    FileInfo::binary(&relative_path, md.len())
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let repo = self.repo.get()?;
                let relative_path_bstr = gix::path::to_unix_separators_on_windows(
                    gix::path::into_bstr(relative_path.clone()),
                );
                let index = repo.index_or_empty()?;
                match index.entry_by_path(relative_path_bstr.as_ref()) {
                    // Read file that has been deleted and not staged for commit.
                    Some(entry) => {
                        let blob = repo.find_blob(entry.id)?;
                        FileInfo::from_content(&relative_path, blob.data.as_ref())
                    }
                    // Read file that has been deleted and staged for commit. Note that file not
                    // found returns FileInfo::default() rather than an error.
                    None => self.read_file_from_commit(
                        repo.head_id()?.detach().to_git2(),
                        &relative_path,
                    )?,
                }
            }
            Err(err) => return Err(err.into()),
        };
        Ok(out)
    }

    fn find_files(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        static FAIR_QUEUE: Mutex<()> = Mutex::new(());
        let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();

        let workdir = self.workdir_or_fail()?;
        let matcher = SkimMatcherV2::default();

        let scored_files = WalkBuilder::new(&workdir)
            .git_exclude(true) // Respect .git/info/exclude
            .git_global(true) // Respect global gitignore
            .git_ignore(true) // Respect .gitignore
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter_map(|entry| {
                let relative_path = entry.path().strip_prefix(&workdir).ok()?;
                let path_str = relative_path.to_string_lossy();
                let score = compute_match_score(&matcher, &path_str, relative_path, query)?;
                Some((score, path_str.to_string()))
            })
            .filter(|entry| entry.0 > 0)
            .sorted_by(|a, b| b.0.cmp(&a.0))
            .take(limit)
            .map(|(_, path)| path)
            .collect();

        Ok(scored_files)
    }
}
