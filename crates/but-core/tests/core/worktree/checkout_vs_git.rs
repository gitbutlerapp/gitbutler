//! Check out every combination of what `HEAD`, the target, the index and the worktree hold at
//! one path, and compare GitButler with Git.
//!
//! - [`checkout_differs_from_git_only_where_listed`] compares `safe_checkout_from_head` with
//!   `git checkout` and `git checkout -m`. GitButler carries uncommitted changes into the target
//!   like the latter, so a case matches Git if it matches either.
//! - [`changes_during_checkout_are_refused_or_carried_like_before_it`] makes each worktree change
//!   while checkout runs, which Git doesn't guard against. GitButler must not lose it: it refuses
//!   and changes nothing, ends as if the change had been made before the checkout, or keeps
//!   everything the change created.
//! - [`checkout_of_paths_differing_in_case_differs_from_git_only_where_listed`] repeats the
//!   comparison with Git where the target names `p` what `HEAD` names `P`, on case-insensitive
//!   filesystems.

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use bstr::ByteSlice as _;
use but_core::worktree::safe_checkout_from_head;
use but_testsupport::{invoke_bash_at_dir, open_repo};

/// What a tree holds at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    Absent,
    File,
    Executable,
    Symlink,
    Directory,
    Submodule,
}

/// What the index holds at the path, compared with `HEAD`.
#[derive(Clone, Copy, Debug)]
enum Index {
    LikeHead,
    Staged,
    StagedRemoval,
}

/// What the worktree holds at the path, compared with the index.
#[derive(Clone, Copy, Debug)]
enum Worktree {
    Clean,
    Edited,
    Deleted,
    File,
    Symlink,
    DirectoryWithUntrackedFile,
    IgnoredFile,
    IgnoredDirectory,
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
const WORKTREES: [Worktree; 8] = [
    Worktree::Clean,
    Worktree::Edited,
    Worktree::Deleted,
    Worktree::File,
    Worktree::Symlink,
    Worktree::DirectoryWithUntrackedFile,
    Worktree::IgnoredFile,
    Worktree::IgnoredDirectory,
];

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
            for target in ENTRIES.into_iter().filter(|target| *target != head) {
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

    fn name(&self) -> String {
        match self.head_path == self.target_path {
            true => format!("HEAD {:?}, target {:?}", self.head, self.target),
            false => format!(
                "HEAD {:?} {}, target {:?} {}",
                self.head, self.head_path, self.target, self.target_path
            ),
        }
    }

    /// A script that builds the repository `base`, where `main` is `HEAD` and `target` the
    /// target. The target also changes `z.f`, so checkout always writes it and runs its filters.
    fn script(&self) -> String {
        let head = put(self.head, self.head_path, "h", "sub-a");
        let target = put(self.target, self.target_path, "m", "sub-b");
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
(cd base && git commit-tree "$empty" -m sub-a) >sub-a
(cd base && git commit-tree "$empty" -m sub-b) >sub-b
cd base
printf 'keep\n' >keep
printf 'z\n' >z.f
{head}
git commit -qm head
git switch -qc target
git rm -rq --cached --ignore-unmatch {head_path} {target_path}
rm -rf {head_path} {target_path}
printf 'z changed\n' >z.f
{target}
git commit -qm target
git switch -q main
git reset -q --hard
"#
        )
    }
}

/// Shell commands, run in `base`, that put `entry` at `path` with `content` and add it. A
/// submodule points to the commit named in the file `submodule` next to `base`.
fn put(entry: Entry, path: &str, content: &str, submodule: &str) -> String {
    match entry {
        Entry::Absent => "git add -A".into(),
        Entry::File => format!("printf '{content}\\n' >{path} && git add -A"),
        Entry::Executable => {
            format!("printf '{content}\\n' >{path} && chmod +x {path} && git add -A")
        }
        Entry::Symlink => format!("ln -s {content} {path} && git add -A"),
        Entry::Directory => {
            format!("mkdir {path} && printf '{content}\\n' >{path}/x && git add -A")
        }
        Entry::Submodule => format!(
            "git add -A && git update-index --add --cacheinfo 160000,$(cat ../{submodule}),{path} && mkdir -p {path}"
        ),
    }
}

/// Shell commands, run in a copy of `base`, that set the index at `p` to `index`. They create
/// `../impossible` if `index` can't exist there.
fn set_index(index: Index) -> &'static str {
    match index {
        Index::LikeHead => "true",
        Index::Staged => "rm -rf p && printf 's\\n' >p && git add -A p",
        Index::StagedRemoval => {
            "if git ls-files --error-unmatch p >/dev/null 2>&1; then git rm -rq --cached p && rm -rf p; else touch ../impossible; fi"
        }
    }
}

/// Shell commands, run in a copy of `base`, that change the worktree at `p` as `worktree` says.
/// They create `../impossible` if `worktree` can't exist there.
fn change_worktree(worktree: Worktree) -> &'static str {
    match worktree {
        Worktree::Clean => "true",
        Worktree::Edited => {
            "if [ -L p ]; then rm p && ln -s w p; elif [ -f p/x ]; then printf 'w\\n' >p/x; elif [ -f p ]; then printf 'w\\n' >p; else touch ../impossible; fi"
        }
        Worktree::Deleted => "if [ -e p ] || [ -L p ]; then rm -rf p; else touch ../impossible; fi",
        Worktree::File => "rm -rf p && printf 'w\\n' >p",
        Worktree::Symlink => "rm -rf p && ln -s w p",
        Worktree::DirectoryWithUntrackedFile => "rm -rf p && mkdir p && printf 'w\\n' >p/y",
        Worktree::IgnoredFile => {
            "if git ls-files --error-unmatch p >/dev/null 2>&1; then touch ../impossible; else rm -rf p && printf 'w\\n' >p && echo /p >>.git/info/exclude; fi"
        }
        Worktree::IgnoredDirectory => {
            "rm -rf p && mkdir p && printf 'w\\n' >p/y && echo /p/ >>.git/info/exclude"
        }
    }
}

/// How a checkout ended, and what HEAD, the index and the worktree hold afterwards.
fn state(dir: &Path, outcome: &str, names: &Names) -> anyhow::Result<String> {
    let repo = open_repo(dir)?;
    let head = names.commit(repo.head_id()?.detach());
    let index = repo.open_index()?;
    let entries: Vec<String> = index
        .entries()
        .iter()
        .filter(|entry| !is_scaffolding(entry.path(&index).to_str_lossy().as_ref()))
        .map(|entry| {
            let stage = match entry.stage() {
                gix::index::entry::Stage::Unconflicted => String::new(),
                stage => format!(" {stage:?}"),
            };
            let content = names.object(&repo, entry.id, entry.mode);
            format!("{} {:?} {content}{stage}", entry.path(&index), entry.mode)
        })
        .collect();
    Ok(format!(
        "{outcome}, HEAD {head}, index [{}], disk [{}]",
        entries.join("; "),
        disk(dir)?.join("; ")
    ))
}

/// Files every case has, which say nothing about it.
fn is_scaffolding(rela_path: &str) -> bool {
    matches!(rela_path, ".git" | "keep" | "z.f")
}

/// Every path below `dir` except the scaffolding, sorted, without following symlinks.
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
            out.push(format!(
                "{rela} -> {}",
                std::fs::read_link(&path)?.display()
            ));
        } else if md.is_dir() {
            out.push(format!("{rela}/"));
            walk(&path, root, out)?;
        } else {
            let executable = if gix::fs::is_executable(&md) {
                " (x)"
            } else {
                ""
            };
            let content = std::fs::read(&path)?;
            out.push(format!("{rela} {:?}{executable}", content.trim().as_bstr()));
        }
    }
    Ok(())
}

/// Names for the commits of a pair, to keep its states readable.
struct Names {
    head: gix::ObjectId,
    target: gix::ObjectId,
    sub_a: gix::ObjectId,
    sub_b: gix::ObjectId,
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
            sub_a: read_id("sub-a")?,
            sub_b: read_id("sub-b")?,
        })
    }

    fn commit(&self, id: gix::ObjectId) -> String {
        match id {
            id if id == self.head => "head".into(),
            id if id == self.target => "target".into(),
            id => id.to_hex_with_len(7).to_string(),
        }
    }

    fn object(
        &self,
        repo: &gix::Repository,
        id: gix::ObjectId,
        mode: gix::index::entry::Mode,
    ) -> String {
        if mode == gix::index::entry::Mode::COMMIT {
            return match id {
                id if id == self.sub_a => "sub-a".into(),
                id if id == self.sub_b => "sub-b".into(),
                id => id.to_hex_with_len(7).to_string(),
            };
        }
        match repo.find_blob(id) {
            Ok(blob) => format!("{:?}", blob.data.trim().as_bstr()),
            Err(_) => id.to_hex_with_len(7).to_string(),
        }
    }
}

/// Check out `target` in the repository at `dir` with GitButler, and return its state.
fn check_out_with_gitbutler(dir: &Path, names: &Names) -> anyhow::Result<String> {
    let repo = open_repo(dir)?;
    let outcome = match safe_checkout_from_head(names.target, &repo, Default::default()) {
        Ok(_) => "done",
        Err(_) => "refused",
    };
    state(dir, outcome, names)
}

/// Run `per_pair` for each of `pairs` on all available threads, and return the results in order.
fn for_each_pair<T: Send>(
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

/// The variants of a pair, each in its own directory below the pair's, with its name.
fn variants(pair_dir: &Path, script: String) -> anyhow::Result<Vec<(PathBuf, String)>> {
    invoke_bash_at_dir(&script, pair_dir);
    let mut out = Vec::new();
    for line in std::fs::read_to_string(pair_dir.join("variants"))?.lines() {
        let (dir, name) = line.split_once('\t').expect("written by the script");
        let dir = pair_dir.join(dir);
        if !dir.join("impossible").exists() {
            out.push((dir, name.to_owned()));
        }
    }
    Ok(out)
}

/// Compare GitButler's checkout of each variant of `pair` with Git's, and return the cases as
/// `(name, git, git -m, GitButler)`.
fn compare_with_git(pair: Pair) -> anyhow::Result<Vec<[String; 4]>> {
    let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let mut script = pair.script();
    script.push_str("cd ..\n");
    let mut at = 0;
    for index in INDEXES {
        for worktree in WORKTREES {
            at += 1;
            let name = format!("{}, index {index:?}, worktree {worktree:?}", pair.name());
            let (index, worktree) = (set_index(index), change_worktree(worktree));
            write!(
                script,
                r#"printf '{at}\t{name}\n' >>variants
mkdir {at} && cp -a base {at}/repo
(cd {at}/repo && {index} && {worktree})
if [ ! -e {at}/impossible ]; then
  cp -a {at}/repo {at}/git && cp -a {at}/repo {at}/gb
  if (cd {at}/git && git checkout -q target) >/dev/null 2>&1; then
    echo done >{at}/git.outcome
  else
    echo refused >{at}/git.outcome
    cp -a {at}/repo {at}/git-m
    (cd {at}/git-m && git checkout -q -m target) >/dev/null 2>&1 && echo done >{at}/git-m.outcome || echo refused >{at}/git-m.outcome
  fi
fi
"#
            )?;
        }
    }
    let variants = variants(tmp.path(), script)?;
    let names = Names::read(tmp.path())?;
    let mut cases = Vec::new();
    for (dir, name) in variants {
        let git_outcome = |run: &str| -> anyhow::Result<String> {
            Ok(std::fs::read_to_string(dir.join(format!("{run}.outcome")))?
                .trim()
                .to_owned())
        };
        let git = state(&dir.join("git"), &git_outcome("git")?, &names)?;
        // Where `git checkout` succeeds, `git checkout -m` does the same.
        let git_m = match dir.join("git-m").exists() {
            true => state(&dir.join("git-m"), &git_outcome("git-m")?, &names)?,
            false => git.clone(),
        };
        cases.push([
            name,
            git,
            git_m,
            check_out_with_gitbutler(&dir.join("gb"), &names)?,
        ]);
    }
    Ok(cases)
}

/// A report of the `cases` where GitButler differs from both Git checkouts.
fn differences_from_git(cases: Vec<[String; 4]>) -> anyhow::Result<String> {
    let (mut like_git, mut like_git_m) = (0, 0);
    let mut differences = String::new();
    let total = cases.len();
    for [name, git, git_m, gitbutler] in cases {
        if gitbutler == git {
            like_git += 1;
        } else if gitbutler == git_m {
            like_git_m += 1;
        } else {
            writeln!(
                differences,
                "{name}\n  git:       {git}\n  git -m:    {git_m}\n  GitButler: {gitbutler}\n"
            )?;
        }
    }
    Ok(format!(
        "{total} cases: {like_git} like `git checkout`, {like_git_m} like `git checkout -m`, {} different\n\n{differences}",
        total - like_git - like_git_m
    ))
}

#[test]
fn checkout_differs_from_git_only_where_listed() -> anyhow::Result<()> {
    let cases = for_each_pair(&Pair::all(), compare_with_git)?;
    let report = differences_from_git(cases.into_iter().flatten().collect())?;
    snapbox::assert_data_eq!(report, snapbox::file!["snapshots/checkout_vs_git.txt"]);
    Ok(())
}

#[test]
fn checkout_of_paths_differing_in_case_differs_from_git_only_where_listed() -> anyhow::Result<()> {
    let probe = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    if !gix::fs::Capabilities::probe_dir(probe.path()).ignore_case {
        return Ok(());
    }
    let cases = for_each_pair(&Pair::differing_in_case(), compare_with_git)?;
    let report = differences_from_git(cases.into_iter().flatten().collect())?;
    snapbox::assert_data_eq!(
        report,
        snapbox::file!["snapshots/checkout_vs_git_differing_in_case.txt"]
    );
    Ok(())
}

#[test]
fn changes_during_checkout_are_refused_or_carried_like_before_it() -> anyhow::Result<()> {
    let cases = for_each_pair(&Pair::all(), change_during_checkout)?;
    let total = cases.iter().map(Vec::len).sum::<usize>();
    let mut violations = String::new();
    for case in cases.into_iter().flatten() {
        if !case.is_kept_or_refused() {
            let During {
                name,
                changed,
                checked_out_after,
                checked_out_during,
                ..
            } = case;
            writeln!(
                violations,
                "{name}\n  the change:            {changed}\n  checkout after it:     {checked_out_after}\n  checkout during it:    {checked_out_during}\n"
            )?;
        }
    }
    snapbox::assert_data_eq!(
        format!("{total} cases\n\n{violations}"),
        snapbox::file!["snapshots/checkout_changes_during.txt"]
    );
    Ok(())
}

/// A worktree change made while checkout runs.
struct During {
    name: String,
    /// The worktree with the change, before any checkout.
    changed: String,
    /// GitButler's checkout of the worktree with the change made before it.
    checked_out_after: String,
    /// GitButler's checkout during which the change was made.
    checked_out_during: String,
    /// Whether every file the change created is still on disk after the checkout during it.
    change_kept: bool,
}

impl During {
    /// The change must not be lost: checkout refuses and changes nothing, ends as if the change
    /// had been made before it, or keeps everything the change created.
    fn is_kept_or_refused(&self) -> bool {
        let refused_without_changes =
            self.checked_out_during == self.changed.replacen("unchanged", "refused", 1);
        refused_without_changes
            || self.checked_out_during == self.checked_out_after
            || (self.checked_out_during.starts_with("done") && self.change_kept)
    }
}

/// When during checkout a change is made.
#[derive(Clone, Copy, Debug)]
enum When {
    /// While checkout writes the target's files to its staging directory, through the smudge
    /// filter of `z.f`.
    Staging,
    /// While checkout checks what it removes against the snapshot, through the clean filter of
    /// `z.f`, whose modification time no longer matches the index so the check must clean it.
    /// `p` sorts first, so it's already checked by then.
    Checking,
}

/// Make each worktree change in `pair` while checkout runs, at each time a filter runs.
fn change_during_checkout(pair: Pair) -> anyhow::Result<Vec<During>> {
    let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let mut script = pair.script();
    script.push_str("cd ..\n");
    let mut at = 0;
    let mut timings = Vec::new();
    for when in [When::Staging, When::Checking] {
        for worktree in WORKTREES.into_iter().skip(1) {
            at += 1;
            timings.push(when);
            let name = format!("{}, worktree {worktree:?} while {when:?}", pair.name());
            let change = change_worktree(worktree);
            let edit_z = match when {
                When::Staging => "true",
                When::Checking => "touch -m -t 200001010000 z.f",
            };
            write!(
                script,
                r#"printf '{at}\t{name}\n' >>variants
mkdir {at} && cp -a base {at}/changed
(cd {at}/changed && {edit_z}) && cp -a {at}/changed {at}/during
(cd {at}/changed && {change}) && cp -a {at}/changed {at}/after
cat >{at}/during/.git/meddle.sh <<'EOF'
cd "$GIT_DIR/.." && ({change}) >/dev/null 2>&1
EOF
"#
            )?;
        }
    }
    let variants = variants(tmp.path(), script)?;
    let names = Names::read(tmp.path())?;
    let unchanged = disk(&tmp.path().join("base"))?;
    let mut cases = Vec::new();
    for (dir, name) in variants {
        let at: usize = dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("numbered")
            .parse()?;
        let during = dir.join("during");
        configure_meddling(&during, timings[at - 1])?;
        let created: Vec<String> = disk(&dir.join("changed"))?
            .into_iter()
            .filter(|entry| !unchanged.contains(entry))
            .collect();
        let checked_out_during = check_out_with_gitbutler(&during, &names)?;
        let after_during = disk(&during)?;
        cases.push(During {
            name,
            changed: state(&dir.join("changed"), "unchanged", &names)?,
            checked_out_after: check_out_with_gitbutler(&dir.join("after"), &names)?,
            checked_out_during,
            change_kept: created.iter().all(|entry| after_during.contains(entry)),
        });
    }
    Ok(cases)
}

/// Make `z.f`'s filter run `.git/meddle.sh` in the repository at `dir`, once, at `when`.
fn configure_meddling(dir: &Path, when: When) -> anyhow::Result<()> {
    std::fs::write(dir.join(".git/info/attributes"), "*.f filter=meddle\n")?;
    let meddle = r#". "$GIT_DIR/meddle.sh" >&2"#;
    let (smudge, clean) = match when {
        When::Staging => (format!("sh -c '{meddle}; cat'"), "cat".to_owned()),
        // The snapshot and the merge clean `z.f` too, but only before checkout creates its
        // staging directory below `.git/gitbutler`, which exists while the check runs.
        When::Checking => (
            "cat".to_owned(),
            format!(
                r#"sh -c 'if [ ! -e "$GIT_DIR/meddled" ] && ls -d "$GIT_DIR"/gitbutler/.gitbutler-checkout-* >/dev/null 2>&1; then touch "$GIT_DIR/meddled"; {meddle}; fi; cat'"#
            ),
        ),
    };
    for (key, value) in [
        ("filter.meddle.smudge", smudge),
        ("filter.meddle.clean", clean),
    ] {
        but_testsupport::git_at_dir(dir)
            .args(["config", key, &value])
            .status()?;
    }
    Ok(())
}
