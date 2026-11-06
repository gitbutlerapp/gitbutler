//! Changed files as a tree of their directories. A directory holding nothing but one other
//! directory merges with it into one row (`src/backend`), as in Zed's project panel.

use std::cmp::{Ordering, Reverse};
use std::collections::HashSet;

use indexmap::IndexMap;

use crate::backend::FileChange;
use crate::state::FileSort;

#[derive(Debug, Clone, PartialEq)]
pub enum TreeRow {
    Dir {
        /// The full path, which is what's collapsed.
        path: String,
        /// What the row shows: the directory's name, or several joined by `/`.
        name: String,
        depth: usize,
        collapsed: bool,
        /// How many files are under it, at any depth.
        files: usize,
    },
    /// The file at this position of the paths given.
    File { pos: usize, depth: usize },
}

#[derive(Default)]
struct Node {
    dirs: IndexMap<String, Node>,
    /// What's in it, in the order the paths reach it.
    children: Vec<Child>,
}

enum Child {
    /// By its index in `dirs`.
    Dir(usize),
    /// By its position in the paths given.
    File(usize),
}

impl Node {
    fn count(&self) -> usize {
        let files = (self.children.iter()).filter(|child| matches!(child, Child::File(_)));
        files.count() + self.dirs.values().map(Node::count).sum::<usize>()
    }
}

/// The rows for `paths`, each directory's directories and files in the order `paths` first
/// reaches them, so pass them sorted by [`tree_order`]. Nothing under a directory in
/// `collapsed` is listed.
pub fn tree_rows(paths: &[&str], collapsed: &HashSet<String>) -> Vec<TreeRow> {
    let mut root = Node::default();
    for (pos, path) in paths.iter().enumerate() {
        let mut parts: Vec<&str> = path.split('/').collect();
        parts.pop();
        let node = parts.into_iter().fold(&mut root, |node, part| {
            let entry = node.dirs.entry(part.to_owned());
            if let indexmap::map::Entry::Vacant(_) = entry {
                node.children.push(Child::Dir(entry.index()));
            }
            entry.or_default()
        });
        node.children.push(Child::File(pos));
    }
    let mut rows = Vec::new();
    walk(&root, "", 0, collapsed, &mut rows);
    rows
}

/// A file or directory's name as [`FileSort::DocsFirst`] orders it: a name in capitals before
/// its first `.`, as `README.md` and `CHANGELOG.md`, which projects keep for people, before
/// the others, then alphabetically whatever the case.
#[derive(PartialEq, Eq)]
struct Name<'a>(&'a str);

impl Name<'_> {
    fn in_capitals(&self) -> bool {
        let stem = self.0.split('.').next().unwrap_or_default();
        stem.chars().any(char::is_alphabetic) && !stem.chars().any(char::is_lowercase)
    }
}

impl Ord for Name<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a, b) = (self.0.chars(), other.0.chars());
        (other.in_capitals().cmp(&self.in_capitals()))
            .then_with(|| {
                a.flat_map(char::to_lowercase)
                    .cmp(b.flat_map(char::to_lowercase))
            })
            .then_with(|| self.0.cmp(other.0))
    }
}

impl PartialOrd for Name<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Where an entry goes among those of its directory: a conflicted file first, as it's what
/// needs doing, then the directories before the files, or with `DocsFirst` both by [`Name`].
/// One sort never mixes `Named` with `Folder`.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Place<'a> {
    Conflicted(&'a str),
    Folder(&'a str),
    Named(Name<'a>),
    File(&'a str),
    Written(Reverse<Option<i64>>, &'a str),
}

/// `a` before `b` in a tree sorted by `sort`: where their paths part, by [`Place`].
pub fn tree_order(sort: FileSort, a: &FileChange, b: &FileChange) -> Ordering {
    fn places(sort: FileSort, file: &FileChange) -> impl Iterator<Item = Place<'_>> {
        let mut parts = file.path.split('/').peekable();
        std::iter::from_fn(move || {
            let name = parts.next()?;
            let is_file = parts.peek().is_none();
            Some(match (is_file, sort) {
                (true, _) if file.conflicted => Place::Conflicted(name),
                (_, FileSort::DocsFirst) => Place::Named(Name(name)),
                (false, FileSort::Name | FileSort::Modified) => Place::Folder(name),
                (true, FileSort::Name) => Place::File(name),
                (true, FileSort::Modified) => Place::Written(Reverse(file.modified), name),
            })
        })
    }
    places(sort, a).cmp(places(sort, b))
}

/// `a` before `b` in `sort`'s order: by path, its parts by [`Name`] with `DocsFirst`, or the
/// last written first, with files of no known write time, as a commit's, last by path.
pub fn file_order(sort: FileSort, a: &FileChange, b: &FileChange) -> Ordering {
    let by_path = a.path.cmp(&b.path);
    // Conflicted first, either way: they're what needs doing.
    b.conflicted.cmp(&a.conflicted).then(match sort {
        FileSort::Name => by_path,
        FileSort::DocsFirst => (a.path.split('/').map(Name)).cmp(b.path.split('/').map(Name)),
        FileSort::Modified => b.modified.cmp(&a.modified).then(by_path),
    })
}

fn walk(
    node: &Node,
    parent: &str,
    depth: usize,
    collapsed: &HashSet<String>,
    rows: &mut Vec<TreeRow>,
) {
    for child in &node.children {
        let (name, mut dir) = match *child {
            Child::File(pos) => {
                rows.push(TreeRow::File { pos, depth });
                continue;
            }
            Child::Dir(index) => match node.dirs.get_index(index) {
                Some(dir) => dir,
                None => continue,
            },
        };
        let mut name = name.clone();
        while let [Child::Dir(only)] = dir.children.as_slice()
            && let Some((child, inner)) = dir.dirs.get_index(*only)
        {
            name = format!("{name}/{child}");
            dir = inner;
        }
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };
        let is_collapsed = collapsed.contains(&path);
        rows.push(TreeRow::Dir {
            path: path.clone(),
            name,
            depth,
            collapsed: is_collapsed,
            files: dir.count(),
        });
        if !is_collapsed {
            walk(dir, &path, depth + 1, collapsed, rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(path: &str, name: &str, depth: usize, files: usize) -> TreeRow {
        TreeRow::Dir {
            path: path.into(),
            name: name.into(),
            depth,
            collapsed: false,
            files,
        }
    }

    /// The rows' files for `paths` sorted by `sort` as the tree sorts them, by path.
    fn listed(sort: FileSort, files: &[FileChange]) -> Vec<String> {
        let mut sorted = files.to_vec();
        sorted.sort_by(|a, b| tree_order(sort, a, b));
        let paths: Vec<&str> = sorted.iter().map(|file| file.path.as_str()).collect();
        (tree_rows(&paths, &HashSet::new()).into_iter())
            .filter_map(|row| match row {
                TreeRow::File { pos, .. } => paths.get(pos).map(|path| path.to_string()),
                TreeRow::Dir { .. } => None,
            })
            .collect()
    }

    fn flat(sort: FileSort, files: &[FileChange]) -> Vec<String> {
        let mut sorted = files.to_vec();
        sorted.sort_by(|a, b| file_order(sort, a, b));
        sorted.into_iter().map(|file| file.path).collect()
    }

    #[test]
    fn by_name_folders_come_first_in_byte_order() {
        let files = [
            "README.md",
            "src/lib.rs",
            "Zebra.md",
            "apps/x.ts",
            "apple.md",
        ]
        .map(FileChange::bare);
        assert_eq!(
            listed(FileSort::Name, &files),
            [
                "apps/x.ts",
                "src/lib.rs",
                "README.md",
                "Zebra.md",
                "apple.md"
            ],
            "folders before files, each by name in byte order, as but.dev lists them"
        );
        assert_eq!(
            flat(FileSort::Name, &files),
            [
                "README.md",
                "Zebra.md",
                "apple.md",
                "apps/x.ts",
                "src/lib.rs"
            ],
            "the flat list goes by path"
        );
    }

    #[test]
    fn docs_first_lists_all_caps_names_first_then_the_rest_whatever_their_case() {
        let files = [
            "src/lib.rs",
            "Zebra.md",
            "README.md",
            "apps/x.ts",
            "CHANGELOG.md",
            "Cargo.toml",
            "apple.md",
            ".github/ci.yml",
        ]
        .map(FileChange::bare);
        let expected = [
            "CHANGELOG.md",
            "README.md",
            ".github/ci.yml",
            "apple.md",
            "apps/x.ts",
            "Cargo.toml",
            "src/lib.rs",
            "Zebra.md",
        ];
        assert_eq!(
            listed(FileSort::DocsFirst, &files),
            expected,
            "names projects keep for people first, then folders and files mixed alphabetically"
        );
        assert_eq!(
            flat(FileSort::DocsFirst, &files),
            expected,
            "the flat list goes by the same names"
        );
    }

    #[test]
    fn folders_list_where_their_files_first_come_and_single_child_chains_merge() {
        let paths = [
            "README.md",
            "src/backend/mod.rs",
            "src/backend/ops.rs",
            "src/ui/diff/rows.rs",
            "Cargo.toml",
        ];
        let rows = tree_rows(&paths, &HashSet::new());
        assert_eq!(
            rows,
            vec![
                TreeRow::File { pos: 0, depth: 0 },
                dir("src", "src", 0, 3),
                dir("src/backend", "backend", 1, 2),
                TreeRow::File { pos: 1, depth: 2 },
                TreeRow::File { pos: 2, depth: 2 },
                dir("src/ui/diff", "ui/diff", 1, 1),
                TreeRow::File { pos: 3, depth: 2 },
                TreeRow::File { pos: 4, depth: 0 },
            ],
            "`src` keeps two children so it stays; `ui` holds only `diff` so they merge; everything in the order given"
        );
    }

    #[test]
    fn by_last_modified_folders_come_first_then_the_newest_files() {
        let file = |path: &str, modified| FileChange {
            modified: Some(modified),
            ..FileChange::bare(path)
        };
        let files = [
            file("a.rs", 1),
            file("README.md", 0),
            file("z.rs", 3),
            file("src/x.rs", 2),
        ];
        assert_eq!(
            listed(FileSort::Modified, &files),
            ["src/x.rs", "z.rs", "a.rs", "README.md"],
            "a write time has no place among folder names, so folders keep going first"
        );
    }

    #[test]
    fn a_collapsed_directory_hides_what_is_under_it() {
        let paths = ["a/b/one.rs", "a/b/two.rs", "top.rs"];
        let rows = tree_rows(&paths, &HashSet::from(["a/b".to_owned()]));
        assert_eq!(
            rows,
            vec![
                TreeRow::Dir {
                    path: "a/b".into(),
                    name: "a/b".into(),
                    depth: 0,
                    collapsed: true,
                    files: 2,
                },
                TreeRow::File { pos: 2, depth: 0 },
            ],
            "the merged `a/b` row is the one that collapses, and still counts its files"
        );
    }

    #[test]
    fn conflicted_files_come_first_in_either_order() {
        let file = |path: &str, modified, conflicted| FileChange {
            modified: Some(modified),
            conflicted,
            ..FileChange::bare(path)
        };
        let (plain, conflicted) = (file("a.rs", 2, false), file("b.rs", 1, true));
        let tree = [
            file("a/x.rs", 2, false),
            file("A.md", 2, false),
            file("z.rs", 1, true),
        ];
        for sort in [FileSort::Name, FileSort::DocsFirst, FileSort::Modified] {
            assert_eq!(
                file_order(sort, &conflicted, &plain),
                Ordering::Less,
                "{sort:?}: a conflicted file goes before an earlier name or a newer write"
            );
            assert_eq!(
                listed(sort, &tree).first().map(String::as_str),
                Some("z.rs"),
                "{sort:?}: in a tree too, before its folder's folders and files"
            );
        }
    }
}
