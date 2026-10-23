//! Check out a commit of consumed changes with `merge_base_override`, as committing or amending
//! does, for every combination of what the commit changes at one path and what the index holds
//! there, and record what the index holds afterwards.
//!
//! The override is `HEAD^{tree}` with the consumed changes, and the commit has it as its tree, so
//! the checkout only moves the index forward by the consumed changes. An unrelated staged file `q`
//! and an unchanged file `r` show whether the rest of the index keeps its content and stats, and
//! `with stat` whether `p` keeps a stat.

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
}

const CHANGES: [Change; 7] = [
    Change::ModifyText,
    Change::ModifyBinary,
    Change::Add,
    Change::Delete,
    Change::MakeExecutable,
    Change::ToSymlink,
    Change::ModifySubmodule,
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
}

const INDEXES: [Index; 7] = [
    Index::LikeHead,
    Index::LikeOverride,
    Index::StagedApart,
    Index::StagedAdjacent,
    Index::StagedOverlapping,
    Index::StagedRemoval,
    Index::Conflicted,
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
        })
    }

    fn staged_apart(self) -> (&'static str, &'static str) {
        match self {
            Change::ModifyBinary => ("100644", "\\0staged\\n"),
            Change::Add => ("100644", "s\\n"),
            // Staged as executable, like the commit makes it.
            Change::MakeExecutable => ("100755", "1\\n2\\n3\\n4\\nS\\n"),
            Change::ModifySubmodule => ("160000", "sub-c"),
            _ => ("100644", "1\\n2\\n3\\n4\\nS\\n"),
        }
    }
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
        set_override = set_p(overridden),
        write_override = write_p(overridden),
    )
}

fn read_id(dir: &Path, name: &str) -> anyhow::Result<gix::ObjectId> {
    let hex = std::fs::read_to_string(dir.join(".git").join(name))?;
    Ok(gix::ObjectId::from_hex(hex.trim().as_bytes())?)
}

/// Check out the commit of consumed changes in `dir` and describe the outcome and the index.
fn check_out(dir: &Path) -> anyhow::Result<String> {
    let repo = open_repo(dir)?;
    let options = checkout::Options {
        merge_base_override: Some(read_id(dir, "override-tree")?),
        ..Default::default()
    };
    let outcome = match safe_checkout_from_head(read_id(dir, "consumed-commit")?, &repo, options) {
        Ok(_) => "done".to_owned(),
        Err(err) => format!("refused: {err}"),
    };
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
                at_p.push(format!("{:?} {content}{stage}{stat}", entry.mode));
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
