use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time,
    time::SystemTime,
};

use gitbutler_diff::{GitHunk, Hunk, HunkHash};
use gitbutler_hunk_dependency::locks::HunkLock;
use gitbutler_serde::BStringForFrontend;
use itertools::Itertools;
use md5::Digest;
use serde::{Deserialize, Serialize};

// this struct is a mapping to the view `Hunk` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of one entry of
// each hunk in one `Branch.ownership` vector entry in Rust.
// an array of them are returned as part of the `VirtualBranchFile` struct
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchHunk {
    pub id: String,
    pub diff: BStringForFrontend,
    pub modified_at: u128,
    pub file_path: PathBuf,
    #[serde(serialize_with = "gitbutler_branch::serde::hash_to_hex")]
    pub hash: HunkHash,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub binary: bool,
    pub locked: bool,
    pub locked_to: Option<Box<[HunkLock]>>,
    pub change_type: gitbutler_diff::ChangeType,
    /// Indicates that the hunk depends on multiple branches. In this case the hunk cant be moved or comitted.
    pub poisoned: bool,
}

/// Lifecycle
impl VirtualBranchHunk {
    pub(crate) fn gen_id(new_start: u32, new_lines: u32) -> String {
        format!("{}-{}", new_start, new_start + new_lines)
    }
    fn from_diff_hunk(
        project_path: &Path,
        file_path: PathBuf,
        hunk: GitHunk,
        hash: Digest,
        mtimes: &mut MTimeCache,
        locked_to: &[HunkLock],
    ) -> Self {
        // Get the unique branch ids (lock.branch_id) from hunk.locked_to that a hunk is locked to (if any)
        let branch_deps_count = locked_to.iter().map(|lock| lock.branch_id).unique().count();

        Self {
            id: Self::gen_id(hunk.new_start, hunk.new_lines),
            modified_at: mtimes.mtime_by_path(project_path.join(&file_path)),
            file_path,
            diff: hunk.diff_lines,
            old_start: hunk.old_start,
            old_lines: hunk.old_lines,
            new_start: hunk.new_start,
            new_lines: hunk.new_lines,
            binary: hunk.binary,
            hash,
            locked: !locked_to.is_empty(),
            locked_to: Some(locked_to.into()),
            change_type: hunk.change_type,
            poisoned: branch_deps_count > 1,
        }
    }
}

impl From<VirtualBranchHunk> for GitHunk {
    fn from(val: VirtualBranchHunk) -> Self {
        GitHunk {
            old_start: val.old_start,
            old_lines: val.old_lines,
            new_start: val.new_start,
            new_lines: val.new_lines,
            diff_lines: val.diff,
            binary: val.binary,
            change_type: val.change_type,
        }
    }
}

/// Takes an iterator with a tuple of a file path and it's corresponding diffs vector
/// and returns the same structure but with VirtualBranchHunks instead of GitHunks,
/// adding things like locks and other virtual branch metadata.
pub(crate) fn file_hunks_from_diffs<'a>(
    project_path: &'a Path,
    diff: impl IntoIterator<Item = (PathBuf, Vec<gitbutler_diff::GitHunk>)> + 'a,
    locks: Option<&'a HashMap<Digest, Vec<HunkLock>>>,
) -> HashMap<PathBuf, Vec<VirtualBranchHunk>> {
    let mut mtimes = MTimeCache::default();
    diff.into_iter()
        .map(move |(file_path, hunks)| {
            let binding = HashMap::new();
            let locks = locks.unwrap_or(&binding);
            let hunks = hunks
                .into_iter()
                .map(|hunk| {
                    let hash = Hunk::hash_diff(&hunk.diff_lines);
                    let binding = Vec::new();
                    let locked_to = locks.get(&hash).unwrap_or(&binding);

                    VirtualBranchHunk::from_diff_hunk(
                        project_path,
                        file_path.clone(),
                        hunk,
                        hash,
                        &mut mtimes,
                        locked_to,
                    )
                })
                .collect::<Vec<_>>();
            (file_path, hunks)
        })
        .collect()
}

#[derive(Default)]
pub struct MTimeCache(HashMap<PathBuf, u128>);

impl MTimeCache {
    pub fn mtime_by_path<P: AsRef<Path>>(&mut self, path: P) -> u128 {
        let path = path.as_ref();

        if let Some(mtime) = self.0.get(path) {
            return *mtime;
        }

        let mtime = path
            .metadata()
            .map_or_else(
                |_| SystemTime::now(),
                |metadata| {
                    metadata
                        .modified()
                        .or(metadata.created())
                        .unwrap_or_else(|_| SystemTime::now())
                },
            )
            .duration_since(time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        self.0.insert(path.into(), mtime);
        mtime
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VirtualBranchHunkRange {
    /// The old line number, if any.
    pub old: Option<u32>,
    /// The new line number, if any.
    pub new: Option<u32>,
}

pub type VirtualBranchHunkRangeMap = HashMap<String, Vec<VirtualBranchHunkRange>>;
