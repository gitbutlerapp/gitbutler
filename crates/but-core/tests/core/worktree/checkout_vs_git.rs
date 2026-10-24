//! Check out every combination of what `HEAD`, the target, the index and the worktree hold at
//! one path, `p`, with `git checkout`, with `git checkout -m` and with GitButler, and list the
//! cases where GitButler differs from both.
//!
//! The lists are the snapshots. Nothing reads them: a change to checkout changes them, so their
//! diff shows which cases it changes. Run with `SNAPSHOTS=overwrite` to record them.
//!
//! In the lists, a file holds `head`, `target`, `staged`, `edited` or `worktree`, after where its
//! content comes from, and a state says what the index and the worktree hold.
//!
//! - [`checkout_differs_from_git_only_where_listed`] compares the checkouts. GitButler carries
//!   uncommitted changes into the target like `git checkout -m`, so a case matches Git if it
//!   matches either.
//! - [`checkout_of_paths_differing_in_case_differs_from_git_only_where_listed`] repeats the
//!   comparison where the target names `p` what `HEAD` names `P`, on case-insensitive
//!   filesystems.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

use bstr::ByteSlice as _;
use but_core::worktree::safe_checkout_from_head;
use but_testsupport::{invoke_bash_at_dir, open_repo};

#[test]
fn checkout_differs_from_git_only_where_listed() -> anyhow::Result<()> {
    let cases = in_parallel(&Pair::all(), compare_with_git)?;
    snapbox::assert_data_eq!(
        report(cases.into_iter().flatten().collect()),
        snapbox::file!["snapshots/checkout_vs_git.txt"]
    );
    Ok(())
}

#[test]
fn checkout_of_paths_differing_in_case_differs_from_git_only_where_listed() -> anyhow::Result<()> {
    let probe = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    if !gix::fs::Capabilities::probe_dir(probe.path()).ignore_case {
        return Ok(());
    }
    let cases = in_parallel(&Pair::differing_in_case(), compare_with_git)?;
    snapbox::assert_data_eq!(
        report(cases.into_iter().flatten().collect()),
        snapbox::file!["snapshots/checkout_vs_git_differing_in_case.txt"]
    );
    Ok(())
}

/// What a tree holds at the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    Absent,
    File,
    Executable,
    Symlink,
    Directory,
    Submodule,
}

/// What the index holds at the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Index {
    LikeHead,
    /// A file with other content than `HEAD`'s, also in the worktree.
    Staged,
    /// Nothing, while `HEAD` has something.
    StagedRemoval,
}

/// What the worktree holds at the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Worktree {
    /// What the index holds.
    Clean,
    /// The index's file with other content.
    Edited,
    Deleted,
    /// An untracked file.
    File,
    /// An untracked symlink.
    Symlink,
    /// A symlink to an existing directory, which checkout must not write through.
    SymlinkToDirectory,
    /// The directory at the path moved elsewhere, and a symlink to it in its place.
    DirectoryMovedBehindSymlink,
    DirectoryWithUntrackedFile,
    IgnoredFile,
    IgnoredDirectory,
    /// An ignored file next to what the directory at the path tracks.
    IgnoredFileInside,
    /// An empty directory next to what the directory at the path tracks.
    EmptyDirectoryInside,
}

const ENTRIES: [Entry; 6] = [
    Entry::Absent,
    Entry::File,
    Entry::Executable,
    Entry::Symlink,
    Entry::Directory,
    Entry::Submodule,
];
const INDEXES: [Index; 3] = [Index::LikeHead, Index::Staged, Index::StagedRemoval];
const WORKTREES: [Worktree; 12] = [
    Worktree::Clean,
    Worktree::Edited,
    Worktree::Deleted,
    Worktree::File,
    Worktree::Symlink,
    Worktree::SymlinkToDirectory,
    Worktree::DirectoryMovedBehindSymlink,
    Worktree::DirectoryWithUntrackedFile,
    Worktree::IgnoredFile,
    Worktree::IgnoredDirectory,
    Worktree::IgnoredFileInside,
    Worktree::EmptyDirectoryInside,
];

impl Entry {
    fn describe(self) -> &'static str {
        match self {
            Entry::Absent => "nothing",
            Entry::File => "file",
            Entry::Executable => "executable",
            Entry::Symlink => "symlink",
            Entry::Directory => "directory",
            Entry::Submodule => "submodule",
        }
    }
}

impl Index {
    fn describe(self) -> &'static str {
        match self {
            Index::LikeHead => "as in HEAD",
            Index::Staged => "staged file",
            Index::StagedRemoval => "staged removal",
        }
    }
}

impl Worktree {
    fn describe(self) -> &'static str {
        match self {
            Worktree::Clean => "clean",
            Worktree::Edited => "edited",
            Worktree::Deleted => "deleted",
            Worktree::File => "untracked file",
            Worktree::Symlink => "untracked symlink",
            Worktree::SymlinkToDirectory => "symlink to a directory",
            Worktree::DirectoryMovedBehindSymlink => "directory moved behind a symlink",
            Worktree::DirectoryWithUntrackedFile => "directory with an untracked file",
            Worktree::IgnoredFile => "ignored file",
            Worktree::IgnoredDirectory => "ignored directory",
            Worktree::IgnoredFileInside => "ignored file inside the directory",
            Worktree::EmptyDirectoryInside => "empty directory inside the directory",
        }
    }
}

/// What the worktree has at the path once the index is set, before the worktree changes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OnDisk {
    Nothing,
    File,
    Symlink,
    /// A directory with the files the index tracks.
    Directory,
    /// A submodule's directory.
    EmptyDirectory,
}

fn on_disk(head: Entry, index: Index) -> OnDisk {
    match (index, head) {
        (Index::Staged, _) => OnDisk::File,
        (Index::StagedRemoval, _) | (Index::LikeHead, Entry::Absent) => OnDisk::Nothing,
        (Index::LikeHead, Entry::File | Entry::Executable) => OnDisk::File,
        (Index::LikeHead, Entry::Symlink) => OnDisk::Symlink,
        (Index::LikeHead, Entry::Directory) => OnDisk::Directory,
        (Index::LikeHead, Entry::Submodule) => OnDisk::EmptyDirectory,
    }
}

/// Whether the index can be as `index` says, and the worktree then as `worktree` says, for
/// `pair`.
fn applies(pair: Pair, index: Index, worktree: Worktree) -> bool {
    if index == Index::StagedRemoval && (pair.head == Entry::Absent || pair.head_path != "p") {
        return false;
    }
    let on_disk = on_disk(pair.head, index);
    let tracked = match index {
        Index::LikeHead => pair.head != Entry::Absent,
        Index::Staged => true,
        Index::StagedRemoval => false,
    };
    match worktree {
        Worktree::Edited => matches!(on_disk, OnDisk::File | OnDisk::Symlink | OnDisk::Directory),
        Worktree::Deleted => on_disk != OnDisk::Nothing,
        Worktree::IgnoredFile => !tracked,
        Worktree::DirectoryMovedBehindSymlink
        | Worktree::IgnoredFileInside
        | Worktree::EmptyDirectoryInside => {
            matches!(on_disk, OnDisk::Directory | OnDisk::EmptyDirectory)
        }
        _ => true,
    }
}

/// What `HEAD` holds at `head_path`, and what the target holds at `target_path`.
#[derive(Clone, Copy, Debug)]
struct Pair {
    head: Entry,
    head_path: &'static str,
    target: Entry,
    target_path: &'static str,
}

impl Pair {
    fn all() -> Vec<Pair> {
        let mut pairs = Vec::new();
        for head in ENTRIES {
            // Nothing changes from nothing to nothing.
            for target in ENTRIES
                .into_iter()
                .filter(|target| *target != head || head != Entry::Absent)
            {
                pairs.push(Pair {
                    head,
                    head_path: "p",
                    target,
                    target_path: "p",
                });
            }
        }
        pairs
    }

    /// `HEAD` names the path `P` and the target `p`.
    fn differing_in_case() -> Vec<Pair> {
        let kinds = [Entry::File, Entry::Directory];
        let mut pairs = Vec::new();
        for head in kinds {
            for target in kinds {
                pairs.push(Pair {
                    head,
                    head_path: "P",
                    target,
                    target_path: "p",
                });
            }
        }
        pairs
    }

    fn describe(&self) -> String {
        match self.head_path == self.target_path {
            true => format!(
                "HEAD {}, target {}",
                self.head.describe(),
                self.target.describe()
            ),
            false => format!(
                "HEAD {} {}, target {} {}",
                self.head.describe(),
                self.head_path,
                self.target.describe(),
                self.target_path
            ),
        }
    }

    /// A script that builds the repository `base`, where `main` is `HEAD` and `target` the
    /// target.
    fn script(&self) -> String {
        let head = put(self.head, self.head_path, "head");
        let target = put(self.target, self.target_path, "target");
        let (head_path, target_path) = (self.head_path, self.target_path);
        // Git and gix decide some cases by `core.ignorecase`, which `git init` sets where the
        // filesystem ignores case. Only the pairs naming the path in different case depend on it.
        let ignore_case = match head_path == target_path {
            true => "git -C base config core.ignorecase false",
            false => "true",
        };
        format!(
            r#"set -eu
empty=$(git hash-object -t tree /dev/null)
git init -q -b main base
{ignore_case}
(cd base && git commit-tree "$empty" -m head) >submodule-head
(cd base && git commit-tree "$empty" -m target) >submodule-target
cd base
printf 'keep\n' >keep
{head}
git commit -qm head
git switch -qc target
git rm -rq --cached --ignore-unmatch {head_path} {target_path}
rm -rf {head_path} {target_path}
{target}
git commit -qm target
git switch -q main
git reset -q --hard
cd ..
"#
        )
    }
}

/// Shell commands, run in `base`, that put `entry` at `path` with `content` and add it. A file
/// holds `content`, a symlink points to it, a directory has the files `x` and `x2`, and a
/// submodule points to the commit named in the file `submodule-{content}` next to `base`.
fn put(entry: Entry, path: &str, content: &str) -> String {
    match entry {
        Entry::Absent => "git add -A".into(),
        Entry::File => format!("printf '{content}\\n' >{path} && git add -A"),
        Entry::Executable => {
            format!("printf '{content}\\n' >{path} && chmod +x {path} && git add -A")
        }
        Entry::Symlink => format!("ln -s {content} {path} && git add -A"),
        Entry::Directory => format!(
            "mkdir {path} && printf '{content}\\n' >{path}/x && printf '{content}2\\n' >{path}/x2 && git add -A"
        ),
        Entry::Submodule => format!(
            "git add -A && git update-index --add --cacheinfo 160000,$(cat ../submodule-{content}),{path} && mkdir -p {path}"
        ),
    }
}

/// Shell commands that set the index at `p` as `index` says for `pair`, under the name `HEAD` tracks
/// it by. Where `HEAD` names its file `P` on a case-insensitive filesystem, the staged file keeps
/// that name, and where `HEAD` has a directory `P`, its entries are removed first. The staged file
/// is checked to be staged.
fn set_index(index: Index, pair: Pair) -> String {
    match index {
        Index::LikeHead => "true".into(),
        Index::Staged => {
            let head_has_a_file =
                matches!(pair.head, Entry::File | Entry::Executable | Entry::Symlink);
            let (forget, tracked) = match (pair.head_path, head_has_a_file) {
                ("p", _) => (String::new(), "p"),
                (head_path, true) => (String::new(), head_path),
                (head_path, false) => (
                    format!("git rm -rq --cached --ignore-unmatch {head_path} && "),
                    "p",
                ),
            };
            format!(
                "rm -rf p && printf 'staged\\n' >p && {forget}git add -A {tracked} && test \"$(git show :{tracked})\" = staged"
            )
        }
        Index::StagedRemoval => "git rm -rq --cached p && rm -rf p".into(),
    }
}

/// Shell commands that change the worktree at `p`, which has `on_disk`, as `worktree` says.
fn change_worktree(worktree: Worktree, on_disk: OnDisk) -> &'static str {
    match worktree {
        Worktree::Clean => "true",
        Worktree::Edited => match on_disk {
            OnDisk::Symlink => "rm p && ln -s edited p",
            OnDisk::Directory => "printf 'edited\\n' >p/x",
            _ => "printf 'edited\\n' >p",
        },
        Worktree::Deleted => "rm -rf p",
        Worktree::File => "rm -rf p && printf 'worktree\\n' >p",
        Worktree::Symlink => "rm -rf p && ln -s worktree p",
        Worktree::SymlinkToDirectory => "rm -rf p && mkdir -p elsewhere && ln -s elsewhere p",
        Worktree::DirectoryMovedBehindSymlink => "mv p elsewhere && ln -s elsewhere p",
        Worktree::DirectoryWithUntrackedFile => "rm -rf p && mkdir p && printf 'worktree\\n' >p/y",
        Worktree::IgnoredFile => {
            "rm -rf p && printf 'worktree\\n' >p && echo /p >>.git/info/exclude"
        }
        Worktree::IgnoredDirectory => {
            "rm -rf p && mkdir p && printf 'worktree\\n' >p/y && echo /p/ >>.git/info/exclude"
        }
        Worktree::IgnoredFileInside => {
            "printf 'worktree\\n' >p/cache && echo /p/cache >>.git/info/exclude"
        }
        Worktree::EmptyDirectoryInside => "mkdir p/empty",
    }
}

/// A combination, checked out three ways.
struct Case {
    name: String,
    before: State,
    git: Checkout,
    git_m: Checkout,
    gitbutler: Checkout,
}

/// Check out each combination of `pair` with Git and with GitButler.
fn compare_with_git(pair: Pair) -> anyhow::Result<Vec<Case>> {
    let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let mut script = pair.script();
    let mut variants = Vec::new();
    for index in INDEXES {
        for worktree in WORKTREES {
            if !applies(pair, index, worktree) {
                continue;
            }
            let at = variants.len();
            let setup = format!(
                "{} && {}",
                set_index(index, pair),
                change_worktree(worktree, on_disk(pair.head, index))
            );
            write!(
                script,
                r#"mkdir {at} && cp -a base {at}/repo
(cd {at}/repo && {setup})
cp -a {at}/repo {at}/git && cp -a {at}/repo {at}/gitbutler
if (cd {at}/git && git checkout -q target) >/dev/null 2>&1; then
  echo checked-out >{at}/git.outcome
else
  echo refused >{at}/git.outcome
  cp -a {at}/repo {at}/git-m
  (cd {at}/git-m && git checkout -q -m target) >/dev/null 2>&1 && echo checked-out >{at}/git-m.outcome || echo refused >{at}/git-m.outcome
fi
"#
            )?;
            variants.push(format!(
                "{}; index {}; worktree {}",
                pair.describe(),
                index.describe(),
                worktree.describe()
            ));
        }
    }
    invoke_bash_at_dir(&script, tmp.path());
    let names = Names::read(tmp.path())?;
    let mut cases = Vec::new();
    for (at, name) in variants.into_iter().enumerate() {
        let dir = tmp.path().join(at.to_string());
        let git = Checkout::by_git(&dir, "git", &names)?;
        // Where `git checkout` succeeds, `git checkout -m` does the same.
        let git_m = match dir.join("git-m").exists() {
            true => Checkout::by_git(&dir, "git-m", &names)?,
            false => git.clone(),
        };
        cases.push(Case {
            name,
            before: State::read(&dir.join("repo"), &names)?,
            git,
            git_m,
            gitbutler: Checkout::by_gitbutler(&dir.join("gitbutler"), &names)?,
        });
    }
    Ok(cases)
}

/// How a checkout ended, and what it left.
#[derive(Clone, PartialEq, Eq)]
struct Checkout {
    checked_out: bool,
    state: State,
}

impl Checkout {
    /// The checkout Git did in the copy `name` below `dir`.
    fn by_git(dir: &Path, name: &str, names: &Names) -> anyhow::Result<Self> {
        let outcome = std::fs::read_to_string(dir.join(format!("{name}.outcome")))?;
        Ok(Checkout {
            checked_out: outcome.trim() == "checked-out",
            state: State::read(&dir.join(name), names)?,
        })
    }

    /// Check out the target in the repository at `dir` with GitButler.
    fn by_gitbutler(dir: &Path, names: &Names) -> anyhow::Result<Self> {
        let repo = open_repo(dir)?;
        let checked_out = safe_checkout_from_head(names.target, &repo, Default::default()).is_ok();
        Ok(Checkout {
            checked_out,
            state: State::read(dir, names)?,
        })
    }

    /// What this checkout did, starting from `before`. HEAD is only mentioned where it isn't
    /// the target after a checkout, or where it moved on a refusal.
    fn describe(&self, before: &State) -> String {
        match (self.checked_out, self.state.head.as_str()) {
            (false, _) if self.state == *before => "refuses".into(),
            (false, "head") => format!("refuses, but leaves {}", self.state),
            (true, "target") => self.state.to_string(),
            (checked_out, head) => format!(
                "{}, with HEAD at {head}, and leaves {}",
                match checked_out {
                    true => "checks out",
                    false => "refuses",
                },
                self.state
            ),
        }
    }
}

/// What `HEAD`, the index and the worktree hold, apart from what every case has.
#[derive(Clone, PartialEq, Eq)]
struct State {
    head: String,
    index: Vec<String>,
    worktree: Vec<String>,
}

impl State {
    fn read(dir: &Path, names: &Names) -> anyhow::Result<Self> {
        let repo = open_repo(dir)?;
        Ok(State {
            head: names.commit(repo.head_id()?.detach()),
            index: index_entries(&repo, names)?,
            worktree: disk(dir)?,
        })
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let list = |items: &[String]| match items.is_empty() {
            true => "nothing".to_owned(),
            false => items.join(", "),
        };
        write!(
            f,
            "index: {}; worktree: {}",
            list(&self.index),
            list(&self.worktree)
        )
    }
}

/// The index of `repo` by path, like `p file staged`, with each conflict as one entry, like
/// `p conflict (ours file target, theirs file staged)`.
fn index_entries(repo: &gix::Repository, names: &Names) -> anyhow::Result<Vec<String>> {
    let index = repo.open_index()?;
    let mut by_path: BTreeMap<String, Vec<(gix::index::entry::Stage, String)>> = BTreeMap::new();
    for entry in index.entries() {
        let path = entry.path(&index).to_str_lossy().into_owned();
        if !is_scaffolding(&path) {
            let object = names.object(repo, entry.id, entry.mode);
            by_path
                .entry(path)
                .or_default()
                .push((entry.stage(), object));
        }
    }
    Ok(by_path
        .into_iter()
        .map(|(path, stages)| match stages.as_slice() {
            [(gix::index::entry::Stage::Unconflicted, object)] => format!("{path} {object}"),
            _ => {
                let stages: Vec<String> = stages
                    .iter()
                    .map(|(stage, object)| format!("{} {object}", stage_name(*stage)))
                    .collect();
                format!("{path} conflict ({})", stages.join(", "))
            }
        })
        .collect())
}

fn stage_name(stage: gix::index::entry::Stage) -> &'static str {
    match stage {
        gix::index::entry::Stage::Unconflicted => "unconflicted",
        gix::index::entry::Stage::Base => "base",
        gix::index::entry::Stage::Ours => "ours",
        gix::index::entry::Stage::Theirs => "theirs",
    }
}

/// Files every case has, which say nothing about it.
fn is_scaffolding(rela_path: &str) -> bool {
    matches!(rela_path, ".git" | "keep")
}

/// What the worktree at `dir` has, apart from the scaffolding, sorted, without following
/// symlinks, like `p/`, `p/x file head` or `p symlink to worktree`.
fn disk(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let rela = path.strip_prefix(root)?.display().to_string();
        if is_scaffolding(&rela) {
            continue;
        }
        let md = path.symlink_metadata()?;
        if md.is_symlink() {
            let target = std::fs::read_link(&path)?;
            out.push(format!("{rela} symlink to {}", target.display()));
        } else if md.is_dir() {
            out.push(format!("{rela}/"));
            walk(&path, root, out)?;
        } else {
            let kind = match gix::fs::is_executable(&md) {
                true => "executable",
                false => "file",
            };
            out.push(format!("{rela} {kind} {}", content(&std::fs::read(&path)?)));
        }
    }
    Ok(())
}

/// A file's `data`, which is one word unless Git wrote conflict markers.
fn content(data: &[u8]) -> String {
    let text = data.trim().to_str_lossy();
    match text.contains("<<<<<<<") {
        true => "with conflict markers".into(),
        false => text.replace('\n', " | "),
    }
}

/// Names for the commits of a pair, to keep its states readable.
struct Names {
    head: gix::ObjectId,
    target: gix::ObjectId,
    submodule_head: gix::ObjectId,
    submodule_target: gix::ObjectId,
}

impl Names {
    fn read(pair_dir: &Path) -> anyhow::Result<Self> {
        let base = open_repo(&pair_dir.join("base"))?;
        let read_id = |name: &str| -> anyhow::Result<gix::ObjectId> {
            let hex = std::fs::read_to_string(pair_dir.join(name))?;
            Ok(gix::ObjectId::from_hex(hex.trim().as_bytes())?)
        };
        Ok(Names {
            head: base.rev_parse_single("main")?.detach(),
            target: base.rev_parse_single("target")?.detach(),
            submodule_head: read_id("submodule-head")?,
            submodule_target: read_id("submodule-target")?,
        })
    }

    fn commit(&self, id: gix::ObjectId) -> String {
        match id {
            id if id == self.head => "head".into(),
            id if id == self.target => "target".into(),
            id if id == self.submodule_head => "head".into(),
            id if id == self.submodule_target => "target".into(),
            id => id.to_hex_with_len(7).to_string(),
        }
    }

    /// The object `id` with `mode`, like `file head`, `symlink to target` or `submodule at head`.
    fn object(
        &self,
        repo: &gix::Repository,
        id: gix::ObjectId,
        mode: gix::index::entry::Mode,
    ) -> String {
        use gix::index::entry::Mode;
        if mode == Mode::COMMIT {
            return format!("submodule at {}", self.commit(id));
        }
        let content = match repo.find_blob(id) {
            Ok(blob) => content(&blob.data),
            Err(_) => id.to_hex_with_len(7).to_string(),
        };
        match mode {
            Mode::SYMLINK => format!("symlink to {content}"),
            Mode::FILE_EXECUTABLE => format!("executable {content}"),
            _ => format!("file {content}"),
        }
    }
}

/// A report of the `cases` where GitButler differs from both Git checkouts, by how it differs
/// from `git checkout`.
fn report(cases: Vec<Case>) -> String {
    let total = cases.len();
    let like_git = cases.iter().filter(|c| c.gitbutler == c.git).count();
    let like_git_m = cases
        .iter()
        .filter(|c| c.gitbutler != c.git && c.gitbutler == c.git_m)
        .count();
    let mut out = format!(
        "{total} cases: {like_git} like `git checkout`, {like_git_m} like `git checkout -m`, {} different\n",
        total - like_git - like_git_m
    );
    let sections = [
        (
            false,
            true,
            "GitButler checks out where `git checkout` refuses",
        ),
        (
            true,
            false,
            "GitButler refuses where `git checkout` checks out",
        ),
        (true, true, "Both check out, differently"),
        (false, false, "Both refuse, leaving different states"),
    ];
    for (git_checks_out, gitbutler_checks_out, title) in sections {
        let differences: Vec<&Case> = cases
            .iter()
            .filter(|c| c.gitbutler != c.git && c.gitbutler != c.git_m)
            .filter(|c| {
                (c.git.checked_out, c.gitbutler.checked_out)
                    == (git_checks_out, gitbutler_checks_out)
            })
            .collect();
        if differences.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {title} ({})\n", differences.len()));
        for case in differences {
            let before = &case.before;
            out.push_str(&format!(
                "\n{}\n  before:    {before}\n  git:       {}\n  git -m:    {}\n  GitButler: {}\n",
                case.name,
                case.git.describe(before),
                case.git_m.describe(before),
                case.gitbutler.describe(before)
            ));
        }
    }
    out
}

/// Run `per_pair` for each of `pairs` on all available threads, and return the results in order.
fn in_parallel<T: Send>(
    pairs: &[Pair],
    per_pair: impl Fn(Pair) -> anyhow::Result<T> + Sync,
) -> anyhow::Result<Vec<T>> {
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut results: Vec<(usize, T)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| -> anyhow::Result<Vec<(usize, T)>> {
                    let mut out = Vec::new();
                    loop {
                        let at = next.fetch_add(1, Ordering::Relaxed);
                        let Some(pair) = pairs.get(at) else {
                            return Ok(out);
                        };
                        out.push((at, per_pair(*pair)?));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("no panic"))
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|lists| lists.into_iter().flatten().collect())
    })?;
    results.sort_by_key(|(at, _)| *at);
    Ok(results.into_iter().map(|(_, result)| result).collect())
}
