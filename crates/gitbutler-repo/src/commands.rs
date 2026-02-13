use std::{path::Path, sync::Mutex};

use anyhow::{Context as _, Result, bail};
use base64::engine::Engine as _;
use but_ctx::Context;
use git2::Oid;
use ignore::WalkBuilder;
use infer::MatcherType;
use itertools::Itertools;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
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
        if let Some(mime_type) =
            kind.and_then(|kind| (kind.matcher_type() == MatcherType::Image).then_some(kind.mime_type()))
        {
            let base64_content = base64::engine::general_purpose::STANDARD.encode(content);
            file_info.content = Some(base64_content);
            file_info.mime_type = Some(mime_type.to_owned())
        }
        file_info
    }

    fn file_name_str(path: &Path) -> String {
        path.file_name().unwrap_or_default().to_string_lossy().into_owned()
    }

    fn is_binary(content: &[u8]) -> bool {
        let partial_content = &content[..content.len().min(8000)];
        gix::filter::plumbing::eol::Stats::from_bytes(partial_content).is_binary()
    }
}

/// Computes a match score for a file path against a search pattern.
///
/// Returns `None` if the file doesn't match the pattern.
///
/// The score combines:
/// - nucleo's path-aware fuzzy match score (bonuses for matches after `/`,
///   word boundaries, and camelCase transitions via `Config::match_paths()`)
/// - Bonus for matches in the filename portion
/// - Mild penalty for deeply nested files
fn compute_match_score(
    pattern: &Pattern,
    matcher: &mut Matcher,
    path_str: &str,
    relative_path: &Path,
    buf: &mut Vec<char>,
) -> Option<u32> {
    buf.clear();
    let path_haystack = Utf32Str::new(path_str, buf);
    let path_score = pattern.score(path_haystack, matcher)?;

    let filename = relative_path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    buf.clear();
    let filename_haystack = Utf32Str::new(filename, buf);
    let filename_bonus = pattern.score(filename_haystack, matcher).unwrap_or(0) / 2;

    let depth_penalty = (relative_path.components().count() as u32) * 10;

    Some(path_score.saturating_add(filename_bonus).saturating_sub(depth_penalty))
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
        let signed = self.git2_repo.get()?.sign_buffer(b"test");
        match signed {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn remotes(&self) -> anyhow::Result<Vec<GitRemote>> {
        let repo = self.git2_repo.get()?;
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

        let repo = self.git2_repo.get()?;
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
        let repo = self.git2_repo.get()?;
        let workdir = repo
            .workdir()
            .context("BUG: can't yet handle bare repos and we shouldn't run into this until we do")?;
        let (path_in_worktree, relative_path) = if probably_relative_path.is_relative() {
            (
                gix::path::realpath(workdir.join(probably_relative_path))?,
                probably_relative_path.to_owned(),
            )
        } else {
            let Ok(relative_path) = probably_relative_path.strip_prefix(workdir) else {
                bail!(
                    "Path to read from at '{}' isn't in the worktree directory '{}'",
                    probably_relative_path.display(),
                    workdir.display()
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
                    None => self.read_file_from_commit(repo.head()?.peel_to_commit()?.id(), &relative_path)?,
                }
            }
            Err(err) => return Err(err.into()),
        })
    }

    fn find_files(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        static FAIR_QUEUE: Mutex<()> = Mutex::new(());
        let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();

        let workdir = self.workdir_or_fail()?;

        // Matcher configured for path matching: bonuses after '/', at word
        // boundaries, and camelCase transitions.
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());

        // Pattern::parse supports multi-word matching (space-separated) and
        // smart case (lowercase = case-insensitive, any uppercase = case-sensitive).
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let pattern_is_empty = pattern.atoms.is_empty();

        // Reusable buffer for Utf32Str conversion.
        let mut buf = Vec::new();

        let scored_files = WalkBuilder::new(&workdir)
            .git_exclude(true)
            .git_global(true)
            .git_ignore(true)
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter_map(|entry| {
                let relative_path = entry.path().strip_prefix(&workdir).ok()?;
                let path_str = relative_path.to_string_lossy();
                if pattern_is_empty {
                    return Some((0u32, path_str.to_string()));
                }
                let score = compute_match_score(&pattern, &mut matcher, &path_str, relative_path, &mut buf)?;
                Some((score, path_str.to_string()))
            })
            .sorted_by(|a, b| b.0.cmp(&a.0))
            .take(limit)
            .map(|(_, path)| path)
            .collect();

        Ok(scored_files)
    }
}
