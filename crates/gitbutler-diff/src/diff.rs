use std::{borrow::Cow, collections::HashMap, path::PathBuf, str};

use anyhow::{Context, Result};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use gitbutler_cherry_pick::RepositoryExt;
use gitbutler_command_context::RepositoryExtLite;
use gitbutler_serde::BStringForFrontend;
use serde::{Deserialize, Serialize};
use tracing::instrument;

pub type DiffByPathMap = HashMap<PathBuf, FileDiff>;

/// The type of change
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// Entry does not exist in old version
    Added,
    /// Entry does not exist in new version
    Deleted,
    /// Entry content changed between old and new
    Modified,
}
impl From<git2::Delta> for ChangeType {
    fn from(v: git2::Delta) -> Self {
        use git2::Delta as D;
        use ChangeType as C;
        match v {
            D::Untracked | D::Added => C::Added,
            D::Modified
            | D::Unmodified
            | D::Renamed
            | D::Copied
            | D::Typechange
            | D::Conflicted => C::Modified,
            D::Ignored | D::Unreadable | D::Deleted => C::Deleted,
        }
    }
}

/// A description of a hunk, as identified by its line number and the amount of lines it spans
/// before and after the change.
#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// The `+`, `-` or ` ` prefixed lines of the diff produced by `git2`, along with their line separator.
    #[serde(rename = "diff")]
    pub diff_lines: BStringForFrontend,
    pub binary: bool,
    #[serde(rename = "changeType")]
    pub change_type: ChangeType,
}

/// Lifecycle
impl GitHunk {
    /// A special hunk that signals a binary file whose complete content is a blob under `hex_id` in Git.
    /// `changetype` is tells us what happened with the file.
    fn binary_marker(hex_id: String, change_type: ChangeType) -> Self {
        GitHunk {
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: 0,
            diff_lines: hex_id.into(),
            binary: true,
            change_type,
        }
    }

    /// Return a hunk that represents a new file by convention.
    fn generic_new_file() -> Self {
        Self {
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: 0,
            diff_lines: Default::default(),
            binary: false,
            change_type: ChangeType::Added,
        }
    }
}

/// Access
impl GitHunk {
    pub(crate) fn contains(&self, line: u32) -> bool {
        self.new_start <= line && self.new_start + self.new_lines >= line
    }
}

/// Comparison
impl GitHunk {
    /// workspace_intersects_unapplied is used to determine if a hunk from a diff between workspace
    /// and the trunk intersects with an unapplied hunk. We want to use the new start/end for the
    /// integration hunk and the old start/end for the unapplied hunk.
    pub fn workspace_intersects_unapplied(
        workspace_hunk: &GitHunk,
        unapplied_hunk: &GitHunk,
    ) -> bool {
        let unapplied_old_end = unapplied_hunk.old_start + unapplied_hunk.old_lines;
        let workspace_new_end = workspace_hunk.new_start + workspace_hunk.new_lines;

        unapplied_hunk.old_start <= workspace_new_end
            && workspace_hunk.new_start <= unapplied_old_end
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: PathBuf,
    /// Hunks might be empty if nothing about the files content is known, which happens
    /// if the content is skipped due to it being a large file.
    pub hunks: Vec<GitHunk>,
    pub skipped: bool,
    /// This is `true` if this is a file with undiffable content. Then, `hunks` might be a single
    /// hunk that is the hash of the binary blob in Git.
    pub binary: bool,
    pub old_size_bytes: u64,
    pub new_size_bytes: u64,
}

#[instrument(level = tracing::Level::DEBUG, skip(repo))]
pub fn workdir(repo: &git2::Repository, commit_oid: git2::Oid) -> Result<DiffByPathMap> {
    let commit = repo
        .find_commit(commit_oid)
        .context("failed to find commit")?;
    let old_tree = repo.find_real_tree(&commit, Default::default())?;

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts
        .recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_binary(true)
        .show_untracked_content(true)
        .ignore_submodules(true)
        .context_lines(3);

    let mut index = repo.index()?;
    // Just a hack to resolve conflicts, which don't get diffed.
    // Diffed conflicts are something we need though.
    // For now, it seems easiest to resolve by adding the path forcefully,
    // which will create objects for the diffs at least.
    let paths_to_add: Vec<_> = index
        .conflicts()?
        .filter_map(Result::ok)
        .filter_map(|c| {
            c.our
                .or(c.their)
                .or(c.ancestor)
                .and_then(|c| c.path.into_string().ok())
        })
        .collect();
    for conflict_path_to_resolve in paths_to_add {
        index.add_path(conflict_path_to_resolve.as_ref())?;
    }
    repo.ignore_large_files_in_diffs(50_000_000)?;
    let diff = repo.diff_tree_to_workdir_with_index(Some(&old_tree), Some(&mut diff_opts))?;
    hunks_by_filepath(Some(repo), &diff)
}

pub fn trees(
    repo: &git2::Repository,
    old_tree: &git2::Tree,
    new_tree: &git2::Tree,
    include_context: bool,
) -> Result<DiffByPathMap> {
    let mut diff_opts = git2::DiffOptions::new();
    let context_lines = match include_context {
        true => 3,
        false => 0,
    };
    diff_opts
        .show_binary(true)
        .ignore_submodules(true)
        .context_lines(context_lines);

    let diff = repo.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))?;
    hunks_by_filepath(None, &diff)
}

/// Transform `diff` into a mapping of `worktree-relative path -> FileDiff`, where `FileDiff` is
/// all the diff-related information one could ask for. This is mainly to workaround `git2`
/// which doesn't provide a format that is easy to use or hunk-based, but it's line-by-line only.
///
/// `repository` should be `None` if there is no reason to access the workdir, which it will do to
/// keep the binary data in the object database, which otherwise would be lost to the system
/// (it's not reconstructable from the delta, or it's not attempted).
pub fn hunks_by_filepath(
    repo: Option<&git2::Repository>,
    diff: &git2::Diff,
) -> Result<DiffByPathMap> {
    enum LineOrHexHash<'a> {
        Line(Cow<'a, BStr>),
        HexHashOfBinaryBlob(String),
    }
    // find all the hunks
    let mut diff_files = HashMap::new();
    let mut err = None;

    diff.print(
        git2::DiffFormat::Patch,
        |delta, hunk, line: git2::DiffLine<'_>| {
            let change_type: ChangeType = delta.status().into();
            let file_path = delta.new_file().path().unwrap_or_else(|| {
                delta
                    .old_file()
                    .path()
                    .expect("failed to get file name from diff")
            });

            let new_start = hunk.as_ref().map_or(0, git2::DiffHunk::new_start);
            let new_lines = hunk.as_ref().map_or(0, git2::DiffHunk::new_lines);
            let old_start = hunk.as_ref().map_or(0, git2::DiffHunk::old_start);
            let old_lines = hunk.as_ref().map_or(0, git2::DiffHunk::old_lines);

            use git2::DiffLineType as D;
            let line = match line.origin_value() {
                D::Addition | D::Deletion | D::Context => {
                    let mut buf = BString::new(Vec::with_capacity(line.content().len() + 1));
                    buf.push_char(line.origin());
                    buf.push_str(line.content());
                    Some(LineOrHexHash::Line(buf.into()))
                }
                D::Binary => {
                    if let Some((full_path, repo)) = repo
                        .and_then(|repo| repo.workdir())
                        .map(|workdir| workdir.join(file_path))
                        .zip(repo)
                    {
                        if !delta.new_file().id().is_zero() && full_path.exists() {
                            let oid = repo.blob_path(full_path.as_path()).unwrap();
                            if delta.new_file().id() != oid {
                                err = Some(format!("we only store the file which is already known by the diff system, but it was different: {} != {}", delta.new_file().id(), oid));
                                return false
                            }
                        }
                    }
                    Some(LineOrHexHash::HexHashOfBinaryBlob(delta.new_file().id().to_string()))
                }
                D::FileHeader => None,
                D::HunkHeader | D::ContextEOFNL | D::AddEOFNL | D::DeleteEOFNL => {
                    Some(LineOrHexHash::Line(line.content().as_bstr().into()))
                }
            };

            match line {
                None => {
                    let existing = diff_files
                        .insert(file_path.to_path_buf(),
                            FileDiff {
                                path: file_path.to_path_buf(),
                                hunks: Vec::new(),
                                skipped: false,
                                binary: delta.new_file().is_binary(),
                                old_size_bytes: delta.old_file().size(),
                                new_size_bytes: delta.new_file().size(),
                        });
                    if existing.is_some() {
                        err = Some(format!("Encountered an invalid internal state related to the diff: {existing:?}"));
                        return false;
                    }
                }
                Some(line) => {
                    let hunks = &mut diff_files.get_mut(file_path).expect("File header inserts the hunk-list").hunks;
                    let same_hunk = hunks.last_mut().filter(|previous_hunk| {
                        previous_hunk.old_start == old_start
                            && previous_hunk.old_lines == old_lines
                            && previous_hunk.new_start == new_start
                            && previous_hunk.new_lines == new_lines
                    });
                    match same_hunk {
                        Some(hunk) => match line {
                            LineOrHexHash::Line(line) => {
                                hunk.diff_lines.push_str(line.as_ref());
                            }
                            LineOrHexHash::HexHashOfBinaryBlob(id) => {
                                let marker =  GitHunk::binary_marker(id, hunk.change_type) ;
                                *hunk = marker;
                            }
                        },
                        None => {
                            let new_hunk = match line {
                                LineOrHexHash::Line(line) => {
                                    GitHunk {
                                        old_start,
                                        old_lines,
                                        new_start,
                                        new_lines,
                                        diff_lines: line.into_owned().into(),
                                        binary: false,
                                        change_type,
                                    }
                                }
                                LineOrHexHash::HexHashOfBinaryBlob(id) => {
                                    GitHunk::binary_marker(id, change_type)
                                }
                            };
                            hunks.push(new_hunk);
                        }
                    }
                }
            }
            true
        },
    )
    .with_context(|| format!("failed to print diff: {err:?}"))?;

    for file in diff_files.values_mut() {
        if let Some(binary_hunk) = file
            .hunks
            .iter()
            .find_map(|hunk| hunk.binary.then(|| hunk.clone()))
        {
            if file.hunks.len() > 1 {
                // TODO(ST): needs tests, this code isn't executed yet.
                // if there are multiple hunks with binary among them, we replace it with a single marker.
                file.hunks = vec![binary_hunk];
            }
        } else if file.hunks.is_empty() {
            file.hunks = vec![GitHunk::generic_new_file()];
        }
    }

    Ok(diff_files)
}

// returns None if it cannot reverse the patch header
fn reverse_patch_header(header: &BStr) -> Option<BString> {
    let mut parts = header.split(|b| b.is_ascii_whitespace());

    match parts.next() {
        Some(b"@@") => {}
        _ => return None,
    };

    let old_range = parts.next()?;
    let new_range = parts.next()?;

    if parts.next() != Some(b"@@") {
        return None;
    };

    let mut buf: BString = "@@ ".into();
    buf.extend_from_slice(&new_range.replace(b"+", b"-"));
    buf.push(b' ');
    buf.extend_from_slice(&old_range.replace(b"-", b"+"));
    buf.push_str(b" @@ ");

    let mut at_least_one_part = false;
    for part in parts {
        buf.extend_from_slice(part);
        buf.push(b' ');
        at_least_one_part = true;
    }
    if at_least_one_part {
        buf.pop();
    }
    Some(buf)
}

fn reverse_patch(patch: &BStr) -> Option<BString> {
    let mut reversed = BString::default();
    for line in patch.lines() {
        if line.starts_with(b"@@") {
            if let Some(header) = reverse_patch_header(line.as_ref()) {
                reversed.push_str(&header);
                reversed.push(b'\n');
            } else {
                return None;
            }
        } else if line.starts_with(b"+") {
            reversed.push_str(line.replacen(b"+", b"-", 1));
            reversed.push(b'\n');
        } else if line.starts_with(b"-") {
            reversed.push_str(line.replacen(b"-", b"+", 1));
            reversed.push(b'\n');
        } else {
            reversed.push_str(line);
            reversed.push(b'\n');
        }
    }
    Some(reversed)
}

struct PatchHeaderInfo {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    rest: BString,
}

/// Write a patch header from patch header information
fn write_patch_header(parsed_header: &PatchHeaderInfo) -> BString {
    let PatchHeaderInfo {
        old_start,
        old_lines,
        new_start,
        new_lines,
        rest,
    } = parsed_header;

    let mut buf = BString::default();
    buf.push_str("@@ ");
    buf.push_str(format!("-{},{}", old_start, old_lines));
    buf.push_str(" +");
    buf.push_str(format!("{},{}", new_start, new_lines));
    buf.push_str(" @@");
    if !rest.is_empty() {
        buf.push(b' ');
        buf.push_str(rest);
    }
    buf.push(b'\n');
    buf
}

/// Extract the patch range information from a patch header line
fn parse_patch_header(header_line: &[u8]) -> Option<PatchHeaderInfo> {
    let mut parts = header_line.split(|b| b.is_ascii_whitespace());
    if parts.next() != Some(b"@@") {
        return None;
    }
    let old_range = parts.next()?;
    let new_range = parts.next()?;
    let mut old_range = old_range.split(|b| b == &b',');
    let mut new_range = new_range.split(|b| b == &b',');

    let old_start = old_range.next()?;
    let old_start = str::from_utf8(old_start).ok()?.replacen("-", "", 1);
    let old_start = old_start.parse::<u32>().ok()?;

    let old_lines = old_range.next()?;
    let old_lines = str::from_utf8(old_lines).ok()?.parse::<u32>().ok()?;

    let new_start = new_range.next()?;
    let new_start = str::from_utf8(new_start).ok()?.replacen("+", "", 1);
    let new_start = new_start.parse::<u32>().ok()?;

    let new_lines = new_range.next()?;
    let new_lines = str::from_utf8(new_lines).ok()?.parse::<u32>().ok()?;

    let mut rest = BString::default();
    let mut at_least_one_part = false;
    for part in parts {
        if part.is_empty() || part == b"@@" || part.trim_ascii() == b"" {
            continue;
        }
        rest.extend_from_slice(part);
        rest.push(b' ');
        at_least_one_part = true;
    }

    if at_least_one_part {
        rest.pop();
    }

    Some(PatchHeaderInfo {
        old_start,
        old_lines,
        new_start,
        new_lines,
        rest,
    })
}

type PatchLineMapKey = (Option<u32>, Option<u32>);

fn create_patch_line_map_key(old_line: Option<u32>, new_line: Option<u32>) -> PatchLineMapKey {
    (old_line, new_line)
}

/// Build a map of old and new line numbers to the patch lines
fn build_patch_line_map(
    patch_lines: bstr::Lines<'_>,
    new_start: u32,
    old_start: u32,
) -> HashMap<PatchLineMapKey, &[u8]> {
    let mut lines_map: HashMap<PatchLineMapKey, &[u8]> = HashMap::new();

    let mut new_line_number = new_start;
    let mut old_line_number = old_start;

    for patch_line in patch_lines {
        if patch_line.starts_with(b"+") {
            let key = create_patch_line_map_key(None, Some(new_line_number));
            lines_map.insert(key, patch_line);
            new_line_number += 1;
            continue;
        }

        if patch_line.starts_with(b"-") {
            let key = create_patch_line_map_key(Some(old_line_number), None);
            lines_map.insert(key, patch_line);
            old_line_number += 1;
            continue;
        }

        let key = create_patch_line_map_key(Some(old_line_number), Some(new_line_number));
        lines_map.insert(key, patch_line);

        // It's a context line
        old_line_number += 1;
        new_line_number += 1;
    }
    lines_map
}

/// Reverse the lines of a patch, given a list of line numbers to reverse
fn reverse_lines(
    patch: &BStr,
    lines: Vec<(Option<u32>, Option<u32>)>,
) -> Option<(BString, PatchHeaderInfo)> {
    let mut trimmed = BString::default();

    let mut patch_lines = patch.lines();

    let header_line = patch_lines.next()?;
    if !header_line.starts_with(b"@@") {
        return None;
    }

    let PatchHeaderInfo {
        old_start,
        new_start,
        rest,
        ..
    } = parse_patch_header(header_line)?;

    let lines_map = build_patch_line_map(patch_lines, new_start, old_start);

    let mut reversed_old_start = None;
    let mut reversed_old_lines = 0_u32;
    let mut reversed_new_start = None;
    let mut reversed_new_lines = 0_u32;

    for (old_line, new_line) in lines {
        let key = create_patch_line_map_key(old_line, new_line);
        if let Some(line) = lines_map.get(&key) {
            if line.starts_with(b"+") {
                if reversed_old_start.is_none() {
                    reversed_old_start = Some(new_line.unwrap());
                }

                trimmed.push_str(line.replacen(b"+", b"-", 1));
                trimmed.push(b'\n');

                reversed_old_lines += 1;
            } else if line.starts_with(b"-") {
                if reversed_new_start.is_none() {
                    reversed_new_start = Some(old_line.unwrap());
                }

                trimmed.push_str(line.replacen(b"-", b"+", 1));
                trimmed.push(b'\n');

                reversed_new_lines += 1;
            } else {
                if reversed_old_start.is_none() {
                    reversed_old_start = Some(new_line.unwrap());
                }

                if reversed_new_start.is_none() {
                    reversed_new_start = Some(old_line.unwrap());
                }

                trimmed.push_str(line);
                trimmed.push(b'\n');

                reversed_old_lines += 1;
                reversed_new_lines += 1;
            }
        }
    }

    if reversed_old_start.is_none() && reversed_new_start.is_none() {
        return None;
    }

    if reversed_new_start.is_none() {
        // Only deleted lines
        reversed_new_start = reversed_old_start.map(|o| if o > 0 { o - 1 } else { 0 });
    }

    if reversed_old_start.is_none() {
        // Only added lines
        reversed_old_start = reversed_new_start.map(|n| if n > 0 { n - 1 } else { 0 });
    }

    let patch_header = PatchHeaderInfo {
        old_start: reversed_old_start?,
        old_lines: reversed_old_lines,
        new_start: reversed_new_start?,
        new_lines: reversed_new_lines,
        rest,
    };

    // Insert reversed header at the top
    let reversed_header = write_patch_header(&patch_header);
    trimmed.insert_str(0, &reversed_header);

    Some((trimmed, patch_header))
}

// returns `None` if the reversal failed
pub fn reverse_hunk(hunk: &GitHunk) -> Option<GitHunk> {
    let new_change_type = match hunk.change_type {
        ChangeType::Added => ChangeType::Deleted,
        ChangeType::Deleted => ChangeType::Added,
        ChangeType::Modified => ChangeType::Modified,
    };
    if hunk.binary {
        None
    } else {
        reverse_patch(hunk.diff_lines.as_ref()).map(|diff| GitHunk {
            old_start: hunk.new_start,
            old_lines: hunk.new_lines,
            new_start: hunk.old_start,
            new_lines: hunk.old_lines,
            diff_lines: diff.into(),
            binary: hunk.binary,
            change_type: new_change_type,
        })
    }
}

/// Reverse the lines of a hunk, given a list of line numbers to reverse
///
/// Returns `None` if the reversal failed
pub fn reverse_hunk_lines(
    hunk: &GitHunk,
    lines: Vec<(Option<u32>, Option<u32>)>,
) -> Option<GitHunk> {
    let new_change_type = match hunk.change_type {
        ChangeType::Added => ChangeType::Deleted,
        ChangeType::Deleted => ChangeType::Added,
        ChangeType::Modified => ChangeType::Modified,
    };

    if hunk.binary {
        None
    } else {
        reverse_lines(hunk.diff_lines.as_ref(), lines).map(|(diff, patch_header)| GitHunk {
            old_start: patch_header.old_start,
            old_lines: patch_header.old_lines,
            new_start: patch_header.new_start,
            new_lines: patch_header.new_lines,
            diff_lines: diff.into(),
            binary: hunk.binary,
            change_type: new_change_type,
        })
    }
}

pub fn diff_files_into_hunks(
    files: DiffByPathMap,
) -> impl Iterator<Item = (PathBuf, Vec<GitHunk>)> {
    files.into_iter().map(|(path, file)| (path, file.hunks))
}

#[cfg(test)]

mod test {
    use super::*;

    #[test]
    fn reverse_lines_single_line_change() {
        let patch: BStringForFrontend = "@@ -55,7 +55,7 @@

     1
     2
-    3
+    b
     4
     5
     6
"
        .into();

        let reversed =
            super::reverse_lines(patch.as_ref(), vec![(Some(58), None), (None, Some(58))]);

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -58,1 +58,1 @@
+    3
-    b
"
                .into()
            )
        );
    }

    #[test]
    fn reverse_lines_single_line_change_add_only() {
        let patch: BStringForFrontend = "@@ -278,6 +278,8 @@
     1
     2
     3
+    4
+    b
     5
     6
     7
"
        .into();

        let reversed = super::reverse_lines(patch.as_ref(), vec![(None, Some(282))]);

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -282,1 +281,0 @@
-    b
"
                .into()
            )
        );
    }

    #[test]
    fn reverse_lines_single_line_change_delete_only() {
        let patch: BStringForFrontend = "@@ -278,8 +278,6 @@
     1
     2
     3
-    b
-    4
     5
     6
     7
"
        .into();

        let reversed = super::reverse_lines(patch.as_ref(), vec![(Some(282), None)]);

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -281,0 +282,1 @@
+    4
"
                .into()
            )
        );
    }

    #[test]
    fn reverse_lines_single_line_change_context() {
        let patch: BStringForFrontend = "@@ -278,8 +278,6 @@
     1
     2
     3
-    b
-    b
     4
     5
     6
"
        .into();

        let reversed = super::reverse_lines(patch.as_ref(), vec![(Some(279), Some(279))]);

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -279,1 +279,1 @@
     2
"
                .into()
            )
        );
    }

    #[test]
    fn reverse_lines_multiple_line_change() {
        let patch: BStringForFrontend = "@@ -278,8 +278,6 @@
     1
     2
     3
-    b
-    4
+    4
+    b
     5
     6
     7
"
        .into();

        let reversed = super::reverse_lines(
            patch.as_ref(),
            vec![
                (Some(281), None),
                (Some(282), None),
                (None, Some(281)),
                (None, Some(282)),
            ],
        );

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -281,2 +281,2 @@
+    b
+    4
-    4
-    b
"
                .into()
            )
        );
    }

    #[test]
    fn reverse_lines_multiple_line_change_add_only() {
        let patch: BStringForFrontend = "@@ -278,6 +278,9 @@
     1
     2
     3
+    4
+    5
+    6
     7
     8
     9
"
        .into();

        let reversed = super::reverse_lines(
            patch.as_ref(),
            vec![(None, Some(281)), (None, Some(282)), (None, Some(283))],
        );

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -281,3 +280,0 @@
-    4
-    5
-    6
"
                .into()
            )
        );
    }

    #[test]
    fn reverse_lines_multiple_line_change_delete_only() {
        let patch: BStringForFrontend = "@@ -278,9 +278,6 @@
     1
     2
     3
-    4
-    5
-    6
     7
     8
     9
"
        .into();

        let reversed = super::reverse_lines(
            patch.as_ref(),
            vec![(Some(281), None), (Some(282), None), (Some(283), None)],
        );

        assert_eq!(
            reversed.map(|r| r.0),
            Some(
                "@@ -280,0 +281,3 @@
+    4
+    5
+    6
"
                .into()
            )
        );
    }
}
