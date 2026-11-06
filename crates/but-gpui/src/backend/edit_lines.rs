//! Editing an uncommitted file where its diff shows it: both of its sides read to start from,
//! the diff between them worked out again as the new side is typed, and the file written back.

use std::ops::Range;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use bstr::{BString, ByteSlice as _};
use but_core::DryRun;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use gix::diff::blob::{Algorithm, Diff, InternedInput};

use super::diff::{display_text, read_side};
use super::intent::Run;
use super::{
    Checkout, DiffBody, DiffLine, FileChange, Hunk, HunkKey, Intent, LineKind, checkout, columns,
    highlight, open,
};

/// An uncommitted file's two sides, read to edit its new side.
pub struct Sides {
    /// The file as it was read, which saving has to find there still.
    pub text: String,
    /// The old side's lines as they are, without their endings, and as the diff shows them.
    pub old: Vec<String>,
    pub old_shown: Vec<DiffLine>,
    /// The new side's lines as the diff shows them, highlighted.
    pub new_shown: Vec<DiffLine>,
}

/// Both sides of `file`, uncommitted in `worktree` or the main checkout of the project at
/// `path`. `None` for a file that isn't text there, as an image or a deletion.
pub fn read_sides(
    path: &Path,
    worktree: Option<&bstr::BStr>,
    file: &FileChange,
) -> Result<Option<Sides>> {
    let Some(state) = file.change.status.state() else {
        return Ok(None);
    };
    let repo = checkout(&open(path)?, worktree)?;
    let bytes = read_side(&repo, state, file.change.path.as_bstr())?;
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(None);
    };
    if text.contains('\0') {
        return Ok(None);
    }
    let old_text = match file.change.status.previous_state_and_path() {
        Some((state, previous)) => {
            let at = previous.unwrap_or(file.change.path.as_bstr());
            read_side(&repo, state, at)?
        }
        None => Vec::new(),
    };
    let old_text = String::from_utf8_lossy(&old_text);
    let old: Vec<String> = lines(&old_text).map(str::to_owned).collect();
    Ok(Some(Sides {
        old_shown: shown(&file.path, old.iter().map(String::as_str)),
        new_shown: shown(&file.path, lines(&text)),
        old,
        text,
    }))
}

/// The plain files `worktree` or the main checkout of the project at `path` tracks, by their
/// paths as git names them, which picking one to edit lists.
pub fn tracked_files(path: &Path, worktree: Option<&bstr::BStr>) -> Result<Vec<String>> {
    use gix::index::entry::{Mode, Stage};
    // The repository alone, not GitButler's project, which takes far longer to open: the
    // picker waits for this.
    let main = gix::open(path)?;
    let repo = match worktree {
        Some(name) => but_workspace::worktrees::open_worktree_repo(&main, name)?,
        None => main,
    };
    let index = repo.index_or_empty()?;
    Ok((index.entries().iter())
        .filter(|entry| entry.stage() == Stage::Unconflicted)
        .filter(|entry| matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE))
        .map(|entry| entry.path(&index).to_str_lossy().into_owned())
        .collect())
}

/// `file`, tracked in `worktree` or the main checkout of the project at `path` with no
/// changes, as a change of nothing yet, to edit as an uncommitted one is, with both of its
/// sides. `None` when it isn't tracked there or isn't text.
pub fn read_unchanged(
    path: &Path,
    worktree: Option<&bstr::BStr>,
    file: &str,
) -> Result<Option<(FileChange, Sides)>> {
    let repo = checkout(&open(path)?, worktree)?;
    let index = repo.index_or_empty()?;
    let Some(entry) = index.entry_by_path(file.into()) else {
        return Ok(None);
    };
    let Some(kind) = entry.mode.to_tree_entry_mode().map(|mode| mode.kind()) else {
        return Ok(None);
    };
    let tracked = but_core::ChangeState { id: entry.id, kind };
    let on_disk = but_core::ChangeState {
        id: gix::ObjectId::null(repo.object_hash()),
        kind,
    };
    let change = FileChange {
        path: file.to_owned(),
        previous_path: None,
        kind: super::ChangeKind::Modified,
        modified: None,
        size: None,
        conflicted: false,
        lines: Some((0, 0)),
        converted: false,
        change: but_core::TreeChange {
            path: file.into(),
            status: but_core::TreeStatus::Modification {
                previous_state: tracked,
                state: on_disk,
                flags: None,
            },
        },
    };
    let sides = read_sides(path, worktree, &change)?;
    Ok(sides.map(|sides| (change, sides)))
}

/// `text`'s lines without their endings, a carriage return's too.
pub fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(|line| line.trim_end_matches('\r'))
}

/// `lines` as the diff shows them, highlighted as the file at `path` is, numbered from 1.
fn shown<'a>(path: &str, lines: impl Iterator<Item = &'a str>) -> Vec<DiffLine> {
    let mut highlighter = highlight::Highlighter::for_path(path);
    (lines.enumerate())
        .map(|(ix, raw)| {
            let text = display_text(raw);
            let spans = (highlighter.as_mut())
                .map(|h| h.line(&text))
                .unwrap_or_default();
            DiffLine {
                spans,
                ..DiffLine::new(LineKind::Context, None, Some(ix as u32 + 1), &text)
            }
        })
        .collect()
}

/// A line typed, as the diff shows it: unhighlighted until it's read again.
pub fn typed_line(raw: &str) -> DiffLine {
    DiffLine::new(LineKind::Context, None, None, &display_text(raw))
}

/// The diff between `old` and `new`, a file's lines on its two sides, with `context` unchanged
/// lines around each change, as a loaded diff has its hunks. `line` gives each line as shown,
/// by its kind and its numbers on the old and new side. With context for the whole file, as an
/// edit shows it, a file with no changes is one hunk of all its lines.
pub fn diff_between(
    old: &[String],
    new: &[String],
    context: u32,
    line: impl Fn(LineKind, Option<u32>, Option<u32>) -> DiffLine,
) -> DiffBody {
    let joined = |lines: &[String]| (lines.iter()).fold(String::new(), |all, l| all + l + "\n");
    let (before, after) = (joined(old), joined(new));
    let input = InternedInput::new(before.as_str(), after.as_str());
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let mut changes: Vec<(Range<u32>, Range<u32>)> =
        diff.hunks().map(|hunk| (hunk.before, hunk.after)).collect();
    let (old_len, new_len) = (old.len() as u32, new.len() as u32);
    // A change of nothing past the end, which every line is context before.
    if changes.is_empty() && new_len > 0 && context >= new_len {
        changes.push((old_len..old_len, new_len..new_len));
    }
    let mut hunks = Vec::new();
    let mut group = 0;
    while let Some(first) = changes.get(group) {
        // Changes closer than twice the context share a hunk, as git's do.
        let mut last = group;
        while let Some(next) = changes.get(last + 1)
            && next
                .0
                .start
                .saturating_sub(changes.get(last).map_or(0, |c| c.0.end))
                <= 2 * context
        {
            last += 1;
        }
        let lead = first.0.start.min(context);
        let (old_start, new_start) = (first.0.start - lead, first.1.start - lead);
        let mut lines = Vec::new();
        let (mut at_old, mut at_new) = (old_start, new_start);
        for (before, after) in changes.get(group..=last).unwrap_or_default() {
            while at_old < before.start {
                lines.push(line(LineKind::Context, Some(at_old + 1), Some(at_new + 1)));
                (at_old, at_new) = (at_old + 1, at_new + 1);
            }
            lines.extend(
                before
                    .clone()
                    .map(|n| line(LineKind::Removed, Some(n + 1), None)),
            );
            lines.extend(
                after
                    .clone()
                    .map(|n| line(LineKind::Added, None, Some(n + 1))),
            );
            (at_old, at_new) = (before.end, after.end);
        }
        let tail = (old_len - at_old).min(context).min(new_len - at_new);
        for _ in 0..tail {
            lines.push(line(LineKind::Context, Some(at_old + 1), Some(at_new + 1)));
            (at_old, at_new) = (at_old + 1, at_new + 1);
        }
        let (old_lines, new_lines) = (at_old - old_start, at_new - new_start);
        // An empty side starts at the line before it, as git numbers it.
        let start = |start: u32, count: u32| if count == 0 { start } else { start + 1 };
        let header = but_core::HunkHeader {
            old_start: start(old_start, old_lines),
            old_lines,
            new_start: start(new_start, new_lines),
            new_lines,
        };
        hunks.push(Hunk {
            key: HunkKey(header),
            header: format!(
                "@@ -{},{} +{},{} @@",
                header.old_start, old_lines, header.new_start, new_lines
            ),
            lines,
            patch: Default::default(),
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        });
        group = last + 1;
    }
    let count = |kind| {
        hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.kind == kind)
            .count()
    };
    let widest = |not| {
        (hunks.iter().flat_map(|h| &h.lines))
            .filter(|l| l.kind != not)
            .map(|l| columns(&l.text))
            .max()
            .unwrap_or(0)
    };
    DiffBody::Hunks {
        added: count(LineKind::Added) as u32,
        removed: count(LineKind::Removed) as u32,
        widest: [widest(LineKind::Added), widest(LineKind::Removed)],
        hunks,
        conflicts: Vec::new(),
    }
}

/// The file at `path` in `worktree` or the main checkout of the project at `path`, as text.
/// `None` when it's gone or isn't text.
pub fn read_text(
    root: &Path,
    worktree: Option<&bstr::BStr>,
    path: &bstr::BStr,
) -> Result<Option<String>> {
    let repo = checkout(&open(root)?, worktree)?;
    let workdir = repo.workdir().context("The checkout has no files")?;
    let on_disk = workdir.join(gix::path::from_bstr(path)?);
    Ok(std::fs::read(on_disk)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok()))
}

/// Write `text` over `path` in `checkout`, if it still reads `read`, as it did when last
/// written or read: refused when something else wrote to it since, as an agent, so nothing it
/// wrote is lost. With `record`, as for an edit's first write, the main checkout's is
/// recorded for undo first, so one undo takes back the whole edit. Done is whether it was
/// written.
pub struct WriteFile {
    pub checkout: Checkout,
    pub path: BString,
    pub read: String,
    pub text: String,
    pub record: bool,
}

impl Intent for WriteFile {
    type Done = bool;

    fn apply(self, run: &mut Run) -> Result<bool> {
        let worktree = self.checkout.linked();
        let unrecorded =
            !self.record || worktree.is_some() || super::edit::editing(run.ctx())?.is_some();
        run.locked(|ctx, perm| {
            let repo = checkout(ctx, worktree)?;
            let workdir = repo.workdir().context("The checkout has no files")?;
            let on_disk = workdir.join(gix::path::from_bstr(self.path.as_bstr())?);
            let plain = on_disk.symlink_metadata().is_ok_and(|m| m.is_file());
            if !plain {
                bail!("{} isn't a file to write to", self.path);
            }
            let now =
                std::fs::read(&on_disk).with_context(|| format!("Couldn't read {}", self.path))?;
            if now != self.read.as_bytes() {
                return Ok(false);
            }
            // One that can't be recorded only logs, as resolving conflicts does: the file is
            // still written, without an undo step.
            if !unrecorded {
                let details = SnapshotDetails::new(OperationKind::FileChanges);
                let snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
                    ctx,
                    details,
                    perm.read_permission(),
                    DryRun::No,
                );
                if let Some(Err(err)) = snapshot.map(|snapshot| snapshot.commit(ctx, perm)) {
                    tracing::warn!(?err, "couldn't record editing the file for undo");
                }
            }
            std::fs::write(&on_disk, self.text)
                .with_context(|| format!("Couldn't write {}", self.path))?;
            Ok(true)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{WriteFile, diff_between};
    use crate::backend::ops::tests::repo_with;
    use crate::backend::{Checkout, DiffBody, DiffLine, LineKind};

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    /// The hunks between `old` and `new` as git prints them.
    fn diffed(old: &str, new: &str, context: u32) -> String {
        let (old, new) = (lines(old), lines(new));
        let body = diff_between(&old, &new, context, |kind, o, n| {
            let text = match kind {
                LineKind::Added => new.get(n.unwrap() as usize - 1),
                _ => old.get(o.unwrap() as usize - 1),
            };
            DiffLine::new(kind, o, n, text.unwrap())
        });
        let DiffBody::Hunks { hunks, .. } = body else {
            panic!("text diffs into hunks")
        };
        let mut out = String::new();
        for hunk in hunks {
            out += &format!("{}\n", hunk.header);
            for line in hunk.lines {
                let sign = match line.kind {
                    LineKind::Context => ' ',
                    LineKind::Added => '+',
                    LineKind::Removed => '-',
                };
                out += &format!("{sign}{}\n", line.text);
            }
        }
        out
    }

    #[test]
    fn typed_lines_diff_as_git_would_show_them() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
        // Two changes far apart are two hunks, each with its context.
        snapbox::assert_data_eq!(
            diffed(old, "1\nTWO\n3\n4\n5\n6\n7\n8\nNINE\n10\n", 1),
            snapbox::str![[r#"
@@ -1,3 +1,3 @@
 1
-2
+TWO
 3
@@ -8,3 +8,3 @@
 8
-9
+NINE
 10

"#]]
        );
        // Close enough to share their context, they're one.
        snapbox::assert_data_eq!(
            diffed(old, "1\nTWO\n3\nFOUR\n5\n6\n7\n8\n9\n10\n", 1),
            snapbox::str![[r#"
@@ -1,5 +1,5 @@
 1
-2
+TWO
 3
-4
+FOUR
 5

"#]]
        );
        // A line added at the end has no context after it, and a new file has no old side.
        snapbox::assert_data_eq!(
            diffed("a\n", "a\nb\n", 3),
            snapbox::str![[r#"
@@ -1,1 +1,2 @@
 a
+b

"#]]
        );
        snapbox::assert_data_eq!(
            diffed("", "a\n", 3),
            snapbox::str![[r#"
@@ -0,0 +1,1 @@
+a

"#]]
        );
        assert_eq!(diffed(old, old, 3), "", "nothing changed has no hunks");
        snapbox::assert_data_eq!(
            diffed("a\nb\n", "a\nb\n", 100),
            snapbox::str![[r#"
@@ -1,2 +1,2 @@
 a
 b

"#]]
        );
    }

    #[test]
    fn an_unchanged_file_opens_with_both_sides_as_committed() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\ntwo\n");
            let path = dir.path();
            assert_eq!(
                super::tracked_files(path, None).unwrap(),
                ["file"],
                "the committed file is listed"
            );

            let (change, sides) = super::read_unchanged(path, None, "file").unwrap().unwrap();
            assert_eq!(change.path, "file");
            assert_eq!(sides.text, "one\ntwo\n", "the new side is the file on disk");
            assert_eq!(sides.old, ["one", "two"], "the old side is as tracked");
            assert!(
                super::read_unchanged(path, None, "missing")
                    .unwrap()
                    .is_none(),
                "an untracked path has nothing to open"
            );
        })
    }

    #[test]
    fn writing_refuses_a_file_written_since_it_was_read() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\ntwo\n");
            let path = dir.path();
            std::fs::write(path.join("file"), "one\nTWO\n").unwrap();
            let write = |read: &str| WriteFile {
                checkout: Checkout::Main,
                path: "file".into(),
                read: read.into(),
                text: "one\n2\n".into(),
                record: true,
            };

            let (written, _) = crate::backend::apply(path, write("one\ntwo\n")).unwrap();
            assert!(!written, "the file isn't what the edit started from");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "one\nTWO\n",
                "nothing is written"
            );

            let (written, _) = crate::backend::apply(path, write("one\nTWO\n")).unwrap();
            assert!(written, "the file is as the edit last saw it");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "one\n2\n",
                "what was typed is the file"
            );
        })
    }
}
