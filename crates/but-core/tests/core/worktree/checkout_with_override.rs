//! Check out a commit of consumed changes with `merge_base_override`, as committing or amending
//! does, for every combination of what the commit changes at one path and what the index holds
//! there, and record what the index holds afterwards.
//!
//! The override is `HEAD^{tree}` with the consumed changes, and the commit has it as its tree, so
//! the checkout only moves the index forward by the consumed changes. An unrelated staged file `q`
//! and an unchanged file `r` show whether the rest of the index keeps its content and stats, and
//! `with stat` whether `p` keeps a stat.
//!
//! Like some callers, the checkout runs on a repository that keeps new objects in memory. The
//! index is read back from disk, so an entry for an object that only exists in memory shows as an
//! id.

use std::path::Path;

use bstr::ByteSlice as _;
use but_core::worktree::{checkout, safe_checkout_from_head};
use but_testsupport::{invoke_bash_at_dir, open_repo};

const LINES: &str = "1\\n2\\n3\\n4\\n5\\n";

/// What the commit changes at `p`.
#[derive(Clone, Copy, Debug)]
enum Change {
    ModifyText,
    ModifyBinary,
    Add,
    Delete,
    MakeExecutable,
    ToSymlink,
    ModifySubmodule,
    AddSubmodule,
    AddEmpty,
}

const CHANGES: [Change; 9] = [
    Change::ModifyText,
    Change::ModifyBinary,
    Change::Add,
    Change::Delete,
    Change::MakeExecutable,
    Change::ToSymlink,
    Change::ModifySubmodule,
    Change::AddSubmodule,
    Change::AddEmpty,
];

/// What the index holds at `p` when the commit is checked out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Index {
    LikeHead,
    LikeOverride,
    /// Staged content with a change far from the consumed one.
    StagedApart,
    /// Staged content with a change on the line next to the consumed one.
    StagedAdjacent,
    /// Staged content with a change on the consumed line.
    StagedOverlapping,
    StagedRemoval,
    Conflicted,
    /// Marked with `git add -N`, which stages an empty placeholder.
    IntentToAdd,
}

const INDEXES: [Index; 8] = [
    Index::LikeHead,
    Index::LikeOverride,
    Index::StagedApart,
    Index::StagedAdjacent,
    Index::StagedOverlapping,
    Index::StagedRemoval,
    Index::Conflicted,
    Index::IntentToAdd,
];

/// An entry as a mode and the `printf` format of its content, or for a submodule, the name of its
/// commit.
type Spec = Option<(&'static str, &'static str)>;

impl Change {
    /// What `HEAD` and the override hold at `p`.
    fn sides(self) -> (Spec, Spec) {
        let lines = Some(("100644", LINES));
        match self {
            Change::ModifyText => (lines, Some(("100644", "1\\nC\\n3\\n4\\n5\\n"))),
            Change::ModifyBinary => (
                Some(("100644", "\\0head\\n")),
                Some(("100644", "\\0override\\n")),
            ),
            Change::Add => (None, Some(("100644", "c\\n"))),
            Change::Delete => (lines, None),
            Change::MakeExecutable => (lines, Some(("100755", LINES))),
            Change::ToSymlink => (lines, Some(("120000", "c"))),
            Change::ModifySubmodule => (Some(("160000", "sub-a")), Some(("160000", "sub-b"))),
            Change::AddSubmodule => (None, Some(("160000", "sub-b"))),
            Change::AddEmpty => (None, Some(("100644", ""))),
        }
    }

    /// What the index holds at `p` in the `index` case, or `None` if another case covers it.
    fn staged(self, index: Index) -> Option<Spec> {
        let (head, overridden) = self.sides();
        let text = matches!(
            self,
            Change::ModifyText | Change::Delete | Change::MakeExecutable
        );
        Some(match index {
            Index::LikeHead => head,
            Index::LikeOverride if overridden.is_none() => return None,
            Index::LikeOverride => overridden,
            Index::StagedApart => Some(self.staged_apart()),
            Index::StagedAdjacent if text => Some(("100644", "1\\n2\\nS\\n4\\n5\\n")),
            Index::StagedOverlapping if text => Some(("100644", "1\\nS\\n3\\n4\\n5\\n")),
            Index::StagedAdjacent | Index::StagedOverlapping => return None,
            Index::StagedRemoval if head.is_none() || overridden.is_none() => return None,
            Index::StagedRemoval => None,
            Index::Conflicted => head,
            Index::IntentToAdd if ![head, overridden].iter().all(is_file_or_absent) => {
                return None;
            }
            Index::IntentToAdd => None,
        })
    }

    fn staged_apart(self) -> (&'static str, &'static str) {
        match self {
            Change::ModifyBinary => ("100644", "\\0staged\\n"),
            Change::Add | Change::AddEmpty => ("100644", "s\\n"),
            // Staged as executable, like the commit makes it.
            Change::MakeExecutable => ("100755", "1\\n2\\n3\\n4\\nS\\n"),
            Change::ModifySubmodule | Change::AddSubmodule => ("160000", "sub-c"),
            _ => ("100644", "1\\n2\\n3\\n4\\nS\\n"),
        }
    }
}

/// Whether `spec` is absent or a regular file, which `git add -N` can mark.
fn is_file_or_absent(spec: &Spec) -> bool {
    matches!(spec, None | Some(("100644" | "100755", _)))
}

/// Set `p` in the index `GIT_INDEX_FILE` names, or the repository's, to `spec`.
fn set_p(spec: Spec) -> String {
    match spec {
        None => "git update-index --force-remove p".into(),
        Some(("160000", name)) => {
            format!("git update-index --add --cacheinfo 160000,$(cat .git/{name}),p")
        }
        Some((mode, content)) => format!(
            "git update-index --add --cacheinfo {mode},$(printf '{content}' | git hash-object -w --stdin),p"
        ),
    }
}

/// Write `spec` as the file `p` in the worktree.
fn write_p(spec: Spec) -> String {
    match spec {
        None => "rm -f p".into(),
        Some(("160000", _)) => "rm -rf p && mkdir p".into(),
        Some(("120000", target)) => format!("rm -f p && ln -s {target} p"),
        Some(("100755", content)) => format!("rm -f p && printf '{content}' >p && chmod +x p"),
        Some((_, content)) => format!("rm -f p && printf '{content}' >p"),
    }
}

fn script(change: Change, index: Index, staged: Spec) -> String {
    let (head, overridden) = change.sides();
    let set_index = match index {
        Index::Conflicted => {
            let base = match head {
                Some(_) => {
                    "printf '100644 %s 1\\tp\\n' $(printf 'base\\n' | git hash-object -w --stdin)\n"
                }
                None => "",
            };
            format!(
                "git update-index --force-remove p\n{{ {base}printf '100644 %s 2\\tp\\n' $(printf 'ours\\n' | git hash-object -w --stdin); printf '100644 %s 3\\tp\\n' $(printf 'theirs\\n' | git hash-object -w --stdin); }} | git update-index --index-info"
            )
        }
        // A file staged by `git add` has a stat, which `--cacheinfo` doesn't record.
        Index::LikeOverride if !matches!(overridden, Some(("160000", _))) => "git add p".into(),
        // Like `git rm --cached p; git add -N p`, also for a `p` the commit deletes.
        Index::IntentToAdd => "created=\n[ -e p ] || { : >p; created=1; }\ngit rm -q --cached --ignore-unmatch p\ngit add -N p\n[ -z \"$created\" ] || rm p".into(),
        _ => set_p(staged),
    };
    format!(
        r#"set -eu
git init -q -b main .
empty=$(git hash-object -t tree /dev/null)
for name in sub-a sub-b sub-c; do git commit-tree "$empty" -m $name >.git/$name; done
printf 'q\n' >q
printf 'r\n' >r
{write_head}
git add -A
{set_head}
git commit -qm head
export GIT_INDEX_FILE="$PWD/.git/override-index"
git read-tree HEAD
{set_override}
git write-tree >.git/override-tree
unset GIT_INDEX_FILE
rm .git/override-index
git commit-tree $(cat .git/override-tree) -p HEAD -m consumed >.git/consumed-commit
{write_override}
{set_index}
printf 'q staged\n' >q
git add q
"#,
        write_head = write_p(head),
        set_head = set_p(head),
        set_override = set_p(overridden),
        write_override = write_p(overridden),
    )
}

fn read_id(dir: &Path, name: &str) -> anyhow::Result<gix::ObjectId> {
    let hex = std::fs::read_to_string(dir.join(".git").join(name))?;
    Ok(gix::ObjectId::from_hex(hex.trim().as_bytes())?)
}

/// Assert that `HEAD` in `dir` holds `p` as `change` declares, so each case starts where it claims.
fn assert_head_holds_p(dir: &Path, change: Change) -> anyhow::Result<()> {
    let repo = open_repo(dir)?;
    let actual = repo
        .head_tree()?
        .lookup_entry_by_path("p")?
        .map(|entry| format!("{:o}", entry.mode().value()));
    let (head, _) = change.sides();
    assert_eq!(
        actual.as_deref(),
        head.map(|(mode, _)| mode),
        "{change:?}: HEAD must hold p as declared"
    );
    Ok(())
}

/// Check out the commit of consumed changes in `dir` and describe the outcome and the index.
fn check_out(dir: &Path) -> anyhow::Result<String> {
    let repo = open_repo(dir)?.with_object_memory();
    let options = checkout::Options {
        merge_base_override: Some(read_id(dir, "override-tree")?),
        ..Default::default()
    };
    let outcome = match safe_checkout_from_head(read_id(dir, "consumed-commit")?, &repo, options) {
        Ok(_) => "done".to_owned(),
        Err(err) => format!("refused: {err}"),
    };
    let repo = open_repo(dir)?;
    let index = repo.open_index()?;
    let submodule = |id: gix::ObjectId| -> anyhow::Result<Option<&str>> {
        for name in ["sub-a", "sub-b", "sub-c"] {
            if read_id(dir, name)? == id {
                return Ok(Some(name));
            }
        }
        Ok(None)
    };
    let mut at_p = Vec::new();
    let (mut q, mut stats) = (String::new(), "kept");
    for entry in index.entries() {
        let path = entry.path(&index);
        let content = match repo.find_blob(entry.id) {
            Err(_) if entry.mode == gix::index::entry::Mode::COMMIT => submodule(entry.id)?
                .map_or_else(|| entry.id.to_hex_with_len(7).to_string(), Into::into),
            Ok(blob) => blob
                .data
                .to_str_lossy()
                .replace('\n', "|")
                .replace('\0', "^@"),
            Err(_) => entry.id.to_hex_with_len(7).to_string(),
        };
        if entry.stat == Default::default() && path != "p" {
            stats = "cleared";
        }
        match path.to_str_lossy().as_ref() {
            "p" => {
                let stage = match entry.stage() {
                    gix::index::entry::Stage::Unconflicted => String::new(),
                    stage => format!(" {stage:?}"),
                };
                let stat = match entry.stat == Default::default() {
                    true => "",
                    false => " with stat",
                };
                let intent = match entry
                    .flags
                    .contains(gix::index::entry::Flags::INTENT_TO_ADD)
                {
                    true => " intent-to-add",
                    false => "",
                };
                at_p.push(format!("{:?} {content}{stage}{stat}{intent}", entry.mode));
            }
            "q" => q = content,
            _ => {}
        }
    }
    Ok(format!(
        "{outcome}, p [{}], q {q}, stats {stats}",
        at_p.join("; ")
    ))
}

#[test]
fn index_after_checking_out_consumed_changes() -> anyhow::Result<()> {
    let mut report = String::new();
    for change in CHANGES {
        for index in INDEXES {
            let Some(staged) = change.staged(index) else {
                continue;
            };
            let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
            invoke_bash_at_dir(&script(change, index, staged), tmp.path());
            assert_head_holds_p(tmp.path(), change)?;
            report.push_str(&format!(
                "{change:?} at p, index {index:?}\n  {}\n",
                check_out(tmp.path())?
            ));
        }
    }
    snapbox::assert_data_eq!(
        report,
        snapbox::file!["snapshots/checkout_with_override.txt"]
    );
    Ok(())
}

/// A merge driver configured for `p` takes no part: the index moves forward as if the consumed
/// change were applied to the staged content, even if the driver would report a conflict.
#[test]
fn merge_drivers_do_not_run_when_moving_the_index_forward() -> anyhow::Result<()> {
    let (change, index) = (Change::ModifyText, Index::StagedApart);
    let staged = change.staged(index).expect("a covered case");
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        &format!(
            "{}git config merge.conflicting.driver 'printf driver >%A; exit 1'\nprintf 'p merge=conflicting\\n' >.git/info/attributes\n",
            script(change, index, staged)
        ),
        tmp.path(),
    );
    snapbox::assert_data_eq!(
        check_out(tmp.path())?,
        snapbox::str!["done, p [Mode(FILE) 1|C|3|4|S|], q q staged|, stats kept"]
    );
    Ok(())
}

/// Every staged file the commit changes merges with its own consumed change, even when one
/// commit changes several of them.
#[test]
fn several_staged_files_merge_with_their_own_consumed_change() -> anyhow::Result<()> {
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"set -eu
git init -q -b main .
printf '1\n2\n3\n4\n5\n' >a
printf '1\n2\n3\n4\n5\n' >b
git add a b
git commit -qm head
blob() { printf "$1" | git hash-object -w --stdin; }
export GIT_INDEX_FILE="$PWD/.git/override-index"
git read-tree HEAD
git update-index --cacheinfo 100644,$(blob '1\nA\n3\n4\n5\n'),a
git update-index --cacheinfo 100644,$(blob '1\n2\n3\nB\n5\n'),b
git write-tree >.git/override-tree
unset GIT_INDEX_FILE
rm .git/override-index
git commit-tree $(cat .git/override-tree) -p HEAD -m consumed >.git/consumed-commit
git update-index --cacheinfo 100644,$(blob '1\n2\n3\n4\na\n'),a
git update-index --cacheinfo 100644,$(blob 'b\n2\n3\n4\n5\n'),b
"#,
        tmp.path(),
    );
    check_out(tmp.path())?;
    let repo = open_repo(tmp.path())?;
    let index = repo.open_index()?;
    let staged = index
        .entries()
        .iter()
        .map(|entry| -> anyhow::Result<String> {
            let blob = repo.find_blob(entry.id)?;
            Ok(format!(
                "{} {}",
                entry.path(&index),
                blob.data.to_str_lossy().replace('\n', "|")
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    snapbox::assert_data_eq!(
        staged.join(", "),
        snapbox::str!["a 1|A|3|4|a|, b b|2|3|B|5|"]
    );
    Ok(())
}

/// With `merge.renormalize`, the staged content still merges as stored, without running the
/// worktree filters, so a failing smudge filter doesn't stop the index from moving forward.
#[test]
fn renormalization_does_not_run_filters_when_moving_the_index_forward() -> anyhow::Result<()> {
    let (change, index) = (Change::ModifyText, Index::StagedApart);
    let staged = change.staged(index).expect("a covered case");
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        &format!(
            "{}git config merge.renormalize true\ngit config filter.failing.clean cat\ngit config filter.failing.smudge false\ngit config filter.failing.required true\nprintf 'p filter=failing\\n' >.git/info/attributes\n",
            script(change, index, staged)
        ),
        tmp.path(),
    );
    snapbox::assert_data_eq!(
        check_out(tmp.path())?,
        snapbox::str!["done, p [Mode(FILE) 1|C|3|4|S|], q q staged|, stats kept"]
    );
    Ok(())
}

/// A file edited within the index's timestamp tick, at the same size and mtime, has a racy stat
/// that can't vouch for its content. Record whether status still shows the edit once the index
/// moves forward.
#[test]
fn racily_clean_edits_after_moving_the_index_forward() -> anyhow::Result<()> {
    let (change, index) = (Change::ModifyText, Index::LikeHead);
    let staged = change.staged(index).expect("a covered case");
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        &format!(
            "{}git config core.trustctime false\ntouch -t 200001010000 r\ngit update-index --really-refresh >/dev/null || true\nprintf 'R\\n' >r\ntouch -t 200001010000 r .git/index\n",
            script(change, index, staged)
        ),
        tmp.path(),
    );
    let status = || -> anyhow::Result<String> {
        let out = std::process::Command::new("git")
            .args(["status", "--porcelain", "--", "r"])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .current_dir(tmp.path())
            .output()?;
        Ok(String::from_utf8(out.stdout)?.trim_end().to_owned())
    };
    assert_eq!(status()?, " M r", "the fixture hides nothing yet");
    check_out(tmp.path())?;
    snapbox::assert_data_eq!(status()?, snapbox::str![""]);
    Ok(())
}
