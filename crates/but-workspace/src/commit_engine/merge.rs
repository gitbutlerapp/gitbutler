//! Merging changes made on `HEAD` onto where they're committed, and telling what went in.

use bstr::{BStr, BString, ByteSlice as _};
use but_core::{
    ChangeState, DiffSpec, HunkHeader, RepositoryExt as _,
    commit::{
        ConflictEntries,
        tree_expression::{MergedTree, TreeExpression},
    },
    tree::{MergedFile, create_tree::RejectionReason},
};

/// The trees committing changes merges: `head`, which the changes were made on, `changed`, which
/// has them, and `onto`, where they're committed.
pub(super) struct Merge {
    pub head: gix::ObjectId,
    pub changed: gix::ObjectId,
    pub onto: gix::ObjectId,
}

/// Changes split by whether a merge took them.
#[derive(Default)]
pub(super) struct Split {
    pub taken: Vec<DiffSpec>,
    pub left_out: Vec<(RejectionReason, DiffSpec)>,
}

impl Merge {
    /// `onto` with the changes from `head` to `changed` merged in, keeping `onto`'s side where
    /// they conflict, along with that conflict.
    pub fn merged_tree(&self, repo: &gix::Repository) -> anyhow::Result<MergedTree> {
        let mut expression = TreeExpression::from(self.onto);
        expression.subtract_add(&self.head.into(), &self.changed.into());
        let merged = expression.merge(repo, repo.merge_options_force_ours()?)?;
        merged.into_merged_tree(repo)
    }

    /// Split `applied`, the changes that make up `changed`, by whether the merge into `merged`
    /// took them, where its `conflicted` paths say it might not have. A file whose whole change
    /// was asked for splits into the hunks it took and those it left out. A file with chosen
    /// hunks is left out whole if any of it is, as its hunks are numbered against the worktree,
    /// not `changed`.
    pub fn split(
        &self,
        repo: &gix::Repository,
        merged: gix::ObjectId,
        conflicted: &ConflictEntries,
        applied: Vec<DiffSpec>,
    ) -> anyhow::Result<Split> {
        let conflicted = (conflicted.ancestor_entries.iter())
            .chain(&conflicted.our_entries)
            .chain(&conflicted.their_entries)
            .map(|path| Ok(gix::path::into_bstr(path.as_path())?.into_owned()))
            .collect::<anyhow::Result<Vec<BString>>>()?;
        let mut split = Split::default();
        for spec in applied {
            let touched = (conflicted.iter())
                .any(|path| *path == spec.path || Some(path) == spec.previous_path.as_ref());
            if !touched {
                split.taken.push(spec);
                continue;
            }
            let hunks = match spec.hunk_headers.is_empty() {
                true => self.file(repo, merged, &spec)?.hunks(repo)?,
                false => None,
            };
            match hunks {
                Some(hunks) if hunks.left_out.is_empty() => split.taken.push(spec),
                Some(hunks) if !hunks.taken.is_empty() => {
                    split.taken.push(selecting(&spec, &hunks.taken));
                    let left_out = selecting(&spec, &hunks.left_out);
                    (split.left_out).push((RejectionReason::CherryPickMergeConflict, left_out));
                }
                Some(_) | None => {
                    (split.left_out).push((RejectionReason::CherryPickMergeConflict, spec))
                }
            }
        }
        Ok(split)
    }

    /// The file `spec` changes, in each tree of the merge into `merged`.
    fn file<'a>(
        &self,
        repo: &gix::Repository,
        merged: gix::ObjectId,
        spec: &'a DiffSpec,
    ) -> anyhow::Result<MergedFile<'a>> {
        let path = spec.path.as_bstr();
        let base_path = spec
            .previous_path
            .as_ref()
            .map_or(path, |path| path.as_bstr());
        Ok(MergedFile {
            path,
            base: state_in(repo, self.head, base_path)?,
            changed: state_in(repo, self.changed, path)?,
            destination: state_in(repo, self.onto, path)?,
            merged: state_in(repo, merged, path)?,
        })
    }
}

/// The file at `path` in `tree`, if it's there.
fn state_in(
    repo: &gix::Repository,
    tree: gix::ObjectId,
    path: &BStr,
) -> anyhow::Result<Option<ChangeState>> {
    let entry = (repo.find_tree(tree)?).lookup_entry_by_path(gix::path::from_bstr(path)?)?;
    Ok(entry.map(|entry| ChangeState {
        id: entry.object_id(),
        kind: entry.mode().kind(),
    }))
}

/// `spec` with just `hunks` of its file, as selections of their lines.
fn selecting(spec: &DiffSpec, hunks: &[HunkHeader]) -> DiffSpec {
    DiffSpec {
        hunk_headers: hunks.iter().flat_map(selections).collect(),
        ..spec.clone()
    }
}

/// `hunk` as a selection of the lines it removes and one of those it adds, which select it
/// whatever the context lines around it.
fn selections(hunk: &HunkHeader) -> impl Iterator<Item = HunkHeader> {
    let removed = (hunk.old_lines > 0).then_some(HunkHeader {
        old_start: hunk.old_start,
        old_lines: hunk.old_lines,
        new_start: 0,
        new_lines: 0,
    });
    let added = (hunk.new_lines > 0).then_some(HunkHeader {
        old_start: 0,
        old_lines: 0,
        new_start: hunk.new_start,
        new_lines: hunk.new_lines,
    });
    removed.into_iter().chain(added)
}
