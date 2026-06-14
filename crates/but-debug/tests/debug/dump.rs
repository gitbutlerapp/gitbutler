use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use but_testsupport::gix_testtools::{
    scripted_fixture_read_only, scripted_fixture_writable, tempfile::TempDir,
};
use but_testsupport::{CommandExt, git_at_dir, visualize_disk_tree_skip_dot_git};

#[test]
fn normal_repo_includes_git_state_and_unignored_worktree() -> anyhow::Result<()> {
    let repo = read_only_repo("dump-normal-repo-with-worktree-changes.sh", "你好 repo")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, @r"
.
├── .git:40755
├── .gitignore:100644
├── executable.sh:100755
├── ignored-dir:40755
│   └── file.txt:100644
├── ignored.ignored:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&repo, &output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, @"
    你好-repo-dump/
    ├── .git/
    │   ├── HEAD:100644
    │   ├── config:100644
    │   ├── gitbutler:40755/
    │   │   └── vb.toml:100644
    │   └── ... 11 files not shown
    ├── .gitignore:100644
    ├── executable.sh:100755
    ├── tracked.ignored:100644
    └── visible.txt:100644
    ");

    Ok(())
}

#[test]
fn git_only_skips_worktree_files() -> anyhow::Result<()> {
    let repo = read_only_repo("dump-normal-repo-with-worktree-changes.sh", "你好 repo")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, @r"
.
├── .git:40755
├── .gitignore:100644
├── executable.sh:100755
├── ignored-dir:40755
│   └── file.txt:100644
├── ignored.ignored:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&repo, &output, true)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, @"
    你好-repo-dump/
    └── .git/
        ├── HEAD:100644
        ├── config:100644
        ├── gitbutler:40755/
        │   └── vb.toml:100644
        └── ... 11 files not shown
    ");

    Ok(())
}

#[test]
fn diagnostics_archive_contains_graph_and_workspace_projection() -> anyhow::Result<()> {
    let fixture = writable_fixture("dump-normal-repo-with-worktree-changes.sh")?;
    let repo = fixture.path().join("你好 repo");
    let output_dir = TempDir::new()?;
    let output = output_dir.path().join("diagnostics.zip");

    let dump_output = run_dump_diagnostics(&repo, &output)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Running: dot -Tsvg [dot path] -o [svg path]
    Archive at: [output path]
    stderr:
    ");

    let mut archive = zip::ZipArchive::new(File::open(&output)?)?;
    let root = "你好-repo-diagnostics";
    let mut dot = String::new();
    archive
        .by_name(&format!("{root}/graph.dot"))?
        .read_to_string(&mut dot)?;
    assert!(
        dot.contains("digraph"),
        "graph dot diagnostics should contain a dot graph"
    );

    let mut workspace = String::new();
    archive
        .by_name(&format!("{root}/workspace.ron.txt"))?
        .read_to_string(&mut workspace)?;
    assert!(
        workspace.contains("Workspace("),
        "workspace diagnostics should contain the debug workspace projection"
    );

    if let Ok(mut svg) = archive.by_name(&format!("{root}/graph.svg")) {
        let mut header = String::new();
        svg.read_to_string(&mut header)?;
        assert!(
            header.contains("<svg"),
            "generated graph svg diagnostics should contain an SVG document"
        );
    } else {
        eprintln!(
            "The graph.svg wasn't generated within 10 seconds, or 'dot' didn't exist here. Should be fine."
        )
    }

    Ok(())
}

#[test]
fn repo_dump_with_diagnostics_injects_files_at_dump_root() -> anyhow::Result<()> {
    let fixture = writable_fixture("dump-normal-repo-with-worktree-changes.sh")?;
    let repo = fixture.path().join("你好 repo");
    let output_dir = TempDir::new()?;
    let output = output_dir.path().join("repo-with-diagnostics.zip");

    let dump_output = run_dump_with_options(&repo, &output, false, true)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Running: dot -Tsvg [dot path] -o [svg path]
    Archive at: [output path]
    stderr:
    ");

    if dot_is_available() {
        insta::assert_snapshot!(archive_tree(&output)?, "diagnostic files are injected right away", @"
        你好-repo-dump/
        ├── .git/
        │   ├── HEAD:100644
        │   ├── config:100644
        │   ├── gitbutler:40755/
        │   │   └── vb.toml:100644
        │   └── ... 13 files not shown
        ├── .gitignore:100644
        ├── executable.sh:100755
        ├── graph.dot:100644
        ├── graph.svg:100644
        ├── tracked.ignored:100644
        ├── visible.txt:100644
        └── workspace.ron.txt:100644
        ");
    }

    Ok(())
}

#[test]
fn repo_dump_diagnostics_override_worktree_files() -> anyhow::Result<()> {
    let fixture = writable_fixture("dump-normal-repo-with-worktree-changes.sh")?;
    let repo = fixture.path().join("你好 repo");
    std::fs::write(repo.join("graph.dot"), "worktree graph")?;

    let output_dir = TempDir::new()?;
    let output = output_dir.path().join("repo-with-diagnostics.zip");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(&repo)?, @"
    .
    ├── .git:40755
    ├── .gitignore:100644
    ├── executable.sh:100755
    ├── graph.dot:100644
    ├── ignored-dir:40755
    │   └── file.txt:100644
    ├── ignored.ignored:100644
    ├── tracked.ignored:100644
    └── visible.txt:100644
    ");

    let dump_output = run_dump_with_options(&repo, &output, false, true)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Running: dot -Tsvg [dot path] -o [svg path]
    Archive at: [output path]
    stderr:
    ");

    if dot_is_available() {
        insta::assert_snapshot!(archive_tree(&output)?, @"
        你好-repo-dump/
        ├── .git/
        │   ├── HEAD:100644
        │   ├── config:100644
        │   ├── gitbutler:40755/
        │   │   └── vb.toml:100644
        │   └── ... 13 files not shown
        ├── .gitignore:100644
        ├── executable.sh:100755
        ├── graph.dot:100644
        ├── graph.svg:100644
        ├── tracked.ignored:100644
        ├── visible.txt:100644
        └── workspace.ron.txt:100644
        ");
    }

    let mut archive = zip::ZipArchive::new(File::open(&output)?)?;
    let mut graph = String::new();
    archive
        .by_name("你好-repo-dump/graph.dot")?
        .read_to_string(&mut graph)?;
    assert!(
        graph.contains("digraph"),
        "diagnostic graph.dot should override the colliding worktree file"
    );

    Ok(())
}

#[test]
fn bare_repo_extracts_as_dump_git_directory() -> anyhow::Result<()> {
    let repo = read_only_repo("dump-bare-repo.sh", "sample.git")?;
    let output_dir = TempDir::new()?;

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&repo, &output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, @"
    sample-dump.git/
    ├── HEAD:100644
    ├── config:100644
    └── ... 2 files not shown
    ");

    Ok(())
}

#[test]
fn linked_worktree_is_unlinked_into_real_git_directory() -> anyhow::Result<()> {
    let linked = read_only_repo("dump-linked-worktree.sh", "linked")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&linked)?;

    insta::assert_snapshot!(initial, @r"
.
├── .git:100644
├── .gitignore:100644
├── linked-worktree-added-to-index.txt:100644
├── linked-worktree-untracked.txt:100644
└── tracked.ignored:100644
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&linked, &output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, ".git is now a directory", @"
    linked-dump/
    ├── .git/
    │   ├── HEAD:100644
    │   ├── config:100644
    │   └── ... 16 files not shown
    ├── .gitignore:100644
    ├── linked-worktree-added-to-index.txt:100644
    ├── linked-worktree-untracked.txt:100644
    └── tracked.ignored:100644
    ");

    let extraction = TempDir::new()?;
    unzip(&output, extraction.path())?;
    let unpacked = extraction.path().join("linked-dump");
    git_at_dir(&unpacked).arg("status").run();
    insta::assert_snapshot!(git_status(&unpacked)?, "the unpacked repository keeps the linked worktree HEAD and index", @r"
## linked
A  linked-worktree-added-to-index.txt
 M tracked.ignored
?? linked-worktree-untracked.txt
");

    Ok(())
}

#[test]
fn current_output_inside_worktree_is_not_archived() -> anyhow::Result<()> {
    let fixture = writable_fixture("dump-normal-repo-with-worktree-changes.sh")?;
    let repo = fixture.path().join("你好 repo");
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, "before any dump output exists in the worktree", @r"
.
├── .git:40755
├── .gitignore:100644
├── executable.sh:100755
├── ignored-dir:40755
│   └── file.txt:100644
├── ignored.ignored:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    let first_output = repo.join("nested/output/sample-dump.zip");
    let dump_output = run_dump(&repo, &first_output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&first_output), "first dump output", @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&first_output)?, "the archive being written is not included in itself", @"
    你好-repo-dump/
    ├── .git/
    │   ├── HEAD:100644
    │   ├── config:100644
    │   ├── gitbutler:40755/
    │   │   └── vb.toml:100644
    │   └── ... 11 files not shown
    ├── .gitignore:100644
    ├── executable.sh:100755
    ├── nested:40755/
    │   └── output:40755/
    ├── tracked.ignored:100644
    └── visible.txt:100644
    ");

    let second_output = repo.join("sample-second-dump.zip");
    let dump_output = run_dump(&repo, &second_output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&second_output), "second dump output", @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&second_output)?, "a previous dump archive in the worktree is included like any other visible file", @"
    你好-repo-dump/
    ├── .git/
    │   ├── HEAD:100644
    │   ├── config:100644
    │   ├── gitbutler:40755/
    │   │   └── vb.toml:100644
    │   └── ... 11 files not shown
    ├── .gitignore:100644
    ├── executable.sh:100755
    ├── nested:40755/
    │   └── output:40755/
    │       └── sample-dump.zip:100644
    ├── tracked.ignored:100644
    └── visible.txt:100644
    ");

    std::fs::create_dir_all(repo.join("nested"))?;
    let denormalized_output = repo.join("nested/../denormalized-dump.zip");
    let dump_output = run_dump(&repo, &denormalized_output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&denormalized_output), "denormalized dump output", @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&repo.join("denormalized-dump.zip"))?, "the denormalized output path is still skipped", @"
    你好-repo-dump/
    ├── .git/
    │   ├── HEAD:100644
    │   ├── config:100644
    │   ├── gitbutler:40755/
    │   │   └── vb.toml:100644
    │   └── ... 11 files not shown
    ├── .gitignore:100644
    ├── executable.sh:100755
    ├── nested:40755/
    │   └── output:40755/
    │       └── sample-dump.zip:100644
    ├── sample-second-dump.zip:100644
    ├── tracked.ignored:100644
    └── visible.txt:100644
    ");

    Ok(())
}

fn read_only_repo(fixture: &str, repo_name: &str) -> anyhow::Result<PathBuf> {
    Ok(scripted_fixture_read_only(fixture)
        .map_err(anyhow::Error::from_boxed)?
        .join(repo_name))
}

fn writable_fixture(fixture: &str) -> anyhow::Result<TempDir> {
    scripted_fixture_writable(fixture).map_err(|err| anyhow::anyhow!("{err}"))
}

struct DumpOutput {
    stdout: String,
    stderr: String,
}

impl fmt::Display for DumpOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stdout:\n{}stderr:\n{}", self.stdout, self.stderr)
    }
}

impl DumpOutput {
    fn display_for_snapshot(&self, output: &Path) -> String {
        let output = self
            .to_string()
            .replace(&output.display().to_string(), "[output path]");
        let mut normalized = String::new();
        for line in output.lines() {
            if line.starts_with("Running: dot -Tsvg ") {
                normalized.push_str("Running: dot -Tsvg [dot path] -o [svg path]\n");
            } else {
                normalized.push_str(line);
                normalized.push('\n');
            }
        }
        normalized
    }
}

fn run_dump(repo: &Path, output: &Path, git_only: bool) -> anyhow::Result<DumpOutput> {
    run_dump_with_options(repo, output, git_only, false)
}

fn run_dump_with_options(
    repo: &Path,
    output: &Path,
    git_only: bool,
    diagnostics: bool,
) -> anyhow::Result<DumpOutput> {
    let mut args = vec![
        OsString::from("but-debug"),
        OsString::from("dump"),
        OsString::from("repo"),
        OsString::from("-C"),
        repo.as_os_str().to_owned(),
        OsString::from("--output"),
        output.as_os_str().to_owned(),
        OsString::from("--no-open-archive-directory"),
    ];
    if git_only {
        args.push(OsString::from("--git-only"));
    }
    if !diagnostics {
        args.push(OsString::from("--no-diagnostics"));
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    but_debug::handle_args(args.into_iter(), &mut stdout, &mut stderr)?;
    Ok(DumpOutput {
        stdout: String::from_utf8(stdout)?,
        stderr: String::from_utf8(stderr)?,
    })
}

fn run_dump_diagnostics(repo: &Path, output: &Path) -> anyhow::Result<DumpOutput> {
    let args = [
        OsString::from("but-debug"),
        OsString::from("dump"),
        OsString::from("diagnostics"),
        OsString::from("-C"),
        repo.as_os_str().to_owned(),
        OsString::from("--output"),
        output.as_os_str().to_owned(),
        OsString::from("--no-open-archive-directory"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    but_debug::handle_args(args.into_iter(), &mut stdout, &mut stderr)?;
    Ok(DumpOutput {
        stdout: String::from_utf8(stdout)?,
        stderr: String::from_utf8(stderr)?,
    })
}

fn git_status(repo: &Path) -> anyhow::Result<String> {
    let output = git_at_dir(repo)
        .args(["status", "--short", "--branch"])
        .output()?;
    assert!(
        output.status.success(),
        "git status failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn dot_is_available() -> bool {
    Command::new("dot")
        .arg("-V")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    explicit_dir: bool,
    explicit_file: bool,
    mode: Option<u32>,
    hidden_files: usize,
    hidden_dirs: usize,
}

fn archive_tree(path: &Path) -> anyhow::Result<termtree::Tree<String>> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut root = Node::default();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = file.name();
        let mode = file.unix_mode();
        // Skip git's hook templates (hooks/*.sample): how many of them `git init` writes differs by
        // git version - e.g. sendemail-validate.sample arrived in git 2.42 - so counting them makes
        // these snapshots flip red on a machine whose git differs from the one that captured them
        // (and the regenerated fixtures are git-version-dependent). They are inert boilerplate
        // irrelevant to what the dump captures, so leaving them out keeps the tree stable.
        if name.ends_with(".sample") {
            continue;
        }
        match classify_entry(name, file.is_dir()) {
            EntryDisplay::Shown => root.insert(name, mode),
            EntryDisplay::Hidden { parent, entry_kind } => root.insert_hidden(&parent, entry_kind),
        }
    }
    Ok(root.into_single_tree())
}

fn unzip(archive: &Path, destination: &Path) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    zip::ZipArchive::new(file)?.extract(destination)?;
    Ok(())
}

impl Node {
    fn insert(&mut self, name: &str, mode: Option<u32>) {
        let is_dir = name.ends_with('/');
        let mut node = self;
        for component in name.trim_end_matches('/').split('/') {
            if component.is_empty() {
                continue;
            }
            node = node.children.entry(component.to_owned()).or_default();
        }
        if is_dir {
            node.explicit_dir = true;
        } else {
            node.explicit_file = true;
        }
        node.mode = mode;
    }

    fn insert_hidden(&mut self, parent: &[String], entry_kind: EntryKind) {
        let mut node = self;
        for component in parent {
            node = node.children.entry(component.to_owned()).or_default();
        }
        match entry_kind {
            EntryKind::File => node.hidden_files += 1,
            EntryKind::Directory => node.hidden_dirs += 1,
        }
    }

    fn into_tree(self, name: String) -> termtree::Tree<String> {
        let mut tree = termtree::Tree::new(self.label(name));
        for (name, child) in self.children {
            tree.push(child.into_tree(name));
        }
        if self.hidden_files != 0 {
            tree.push(hidden_summary(self.hidden_files));
        }
        tree
    }

    fn into_single_tree(self) -> termtree::Tree<String> {
        let mut children = self.children;
        assert_eq!(children.len(), 1, "archive should have exactly one root");
        let (name, child) = children.pop_first().expect("archive root is present");
        child.into_tree(name)
    }

    fn label(&self, name: String) -> String {
        let mode = self
            .mode
            .map(|mode| format!(":{mode:o}"))
            .unwrap_or_default();
        match (
            self.children.is_empty(),
            self.explicit_dir,
            self.explicit_file,
        ) {
            (false, _, true) => format!("{name}{mode} [file+dir]"),
            (false, _, false) | (true, true, false) => format!("{name}{mode}/"),
            _ => format!("{name}{mode}"),
        }
    }
}

#[derive(Clone, Copy)]
enum EntryKind {
    File,
    Directory,
}

enum EntryDisplay {
    Shown,
    Hidden {
        parent: Vec<String>,
        entry_kind: EntryKind,
    },
}

fn classify_entry(name: &str, is_dir: bool) -> EntryDisplay {
    let components: Vec<_> = name
        .trim_end_matches('/')
        .split('/')
        .filter(|component| !component.is_empty())
        .collect();

    if components.contains(&"..") {
        return EntryDisplay::Shown;
    }

    if let Some(git_position) = components.iter().position(|component| *component == ".git") {
        let git_path = &components[git_position + 1..];
        return match git_path {
            []
            | ["HEAD" | "config" | "commondir" | "gitdir"]
            | ["gitbutler"]
            | ["gitbutler", "vb.toml"] => EntryDisplay::Shown,
            ["worktrees", ..] => EntryDisplay::Shown,
            _ => EntryDisplay::Hidden {
                parent: components[..=git_position]
                    .iter()
                    .map(|component| component.to_string())
                    .collect(),
                entry_kind: entry_kind(is_dir),
            },
        };
    }

    if let Some((root, git_path)) = components.split_first()
        && root.ends_with(".git")
    {
        return match git_path {
            [] | ["HEAD" | "config"] | ["gitbutler"] | ["gitbutler", "vb.toml"] => {
                EntryDisplay::Shown
            }
            _ => EntryDisplay::Hidden {
                parent: vec![root.to_string()],
                entry_kind: entry_kind(is_dir),
            },
        };
    }

    EntryDisplay::Shown
}

fn entry_kind(is_dir: bool) -> EntryKind {
    if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn hidden_summary(files: usize) -> termtree::Tree<String> {
    termtree::Tree::new(format!(
        "... {files} {} not shown",
        if files == 1 { "file" } else { "files" },
    ))
}
