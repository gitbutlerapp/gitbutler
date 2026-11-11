use bstr::{BStr, ByteSlice as _};

use crate::{ChangeState, HunkHeader, UnifiedPatch, unified_diff::DiffHunk};

/// A file that changes were merged into, by its versions: as the changes found it in `base`,
/// as they `changed` it, at the `destination` they were merged onto, and as `merged` there.
/// A version is `None` where the file isn't there.
#[derive(Debug, Clone, Copy)]
pub struct MergedFile<'a> {
    /// Where the file is, which decides how it's filtered for diffing.
    pub path: &'a BStr,
    /// The file as the changes found it.
    pub base: Option<ChangeState>,
    /// The file as the changes left it.
    pub changed: Option<ChangeState>,
    /// The file where the changes were merged onto.
    pub destination: Option<ChangeState>,
    /// The file as the merge left it there.
    pub merged: Option<ChangeState>,
}

/// The hunks of a change, without context, split by whether a merge took them.
#[derive(Debug, Default, PartialEq)]
pub struct MergedHunks {
    /// The hunks the merge took, numbered from `base` to `changed`.
    pub taken: Vec<HunkHeader>,
    /// The hunks the merge left out, numbered from `base` to `changed`.
    pub left_out: Vec<HunkHeader>,
}

impl MergedFile<'_> {
    /// Split the change from `base` to `changed` into the hunks the merge took, which show up line
    /// for line among the changes from `destination` to `merged`, and those it left out. `None` if
    /// the file can't be diffed as text.
    pub fn hunks(&self, repo: &gix::Repository) -> anyhow::Result<Option<MergedHunks>> {
        let changed = hunks_between(repo, self.path, self.base, self.changed)?;
        let landed = hunks_between(repo, self.path, self.destination, self.merged)?;
        let (Some(changed), Some(landed)) = (changed, landed) else {
            return Ok(None);
        };
        let landed: Vec<&BStr> = landed.iter().map(lines).collect();
        let mut hunks = MergedHunks::default();
        for hunk in &changed {
            match landed.contains(&lines(hunk)) {
                true => hunks.taken.push(hunk.into()),
                false => hunks.left_out.push(hunk.into()),
            }
        }
        Ok(Some(hunks))
    }
}

/// The hunks without context that turn `from` into `to`, or `None` if they can't be diffed as
/// text.
fn hunks_between(
    repo: &gix::Repository,
    path: &BStr,
    from: Option<ChangeState>,
    to: Option<ChangeState>,
) -> anyhow::Result<Option<Vec<DiffHunk>>> {
    if from.map(|state| state.id) == to.map(|state| state.id) {
        return Ok(Some(Vec::new()));
    }
    Ok(
        match UnifiedPatch::compute(repo, path, None, to, from, 0)? {
            Some(UnifiedPatch::Patch {
                hunks,
                is_result_of_binary_to_text_conversion: false,
                ..
            }) => Some(hunks),
            Some(
                UnifiedPatch::Patch { .. } | UnifiedPatch::Binary | UnifiedPatch::TooLarge { .. },
            )
            | None => None,
        },
    )
}

/// The lines `hunk` removes and adds, without its `@@` header.
fn lines(hunk: &DiffHunk) -> &BStr {
    let diff = hunk.diff.as_bstr();
    diff.split_once_str("\n")
        .map_or(diff, |(_, lines)| lines.as_bstr())
}
