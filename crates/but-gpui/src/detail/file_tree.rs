//! Changed files as a tree of their directories. A directory holding nothing but one other
//! directory merges with it into one row (`src/backend`), as in Zed's project panel.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};

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
    dirs: BTreeMap<String, Node>,
    files: Vec<(String, usize)>,
}

impl Node {
    fn count(&self) -> usize {
        self.files.len() + self.dirs.values().map(Node::count).sum::<usize>()
    }
}

/// The rows for `paths`, directories before files at each level, the directories by name
/// and the files in the order `paths` has them. Nothing under a directory in `collapsed` is
/// listed.
pub fn tree_rows(paths: &[&str], collapsed: &HashSet<String>) -> Vec<TreeRow> {
    let mut root = Node::default();
    for (pos, path) in paths.iter().enumerate() {
        let mut parts: Vec<&str> = path.split('/').collect();
        let name = parts.pop().unwrap_or(path);
        let node = parts.into_iter().fold(&mut root, |node, part| {
            node.dirs.entry(part.to_owned()).or_default()
        });
        node.files.push((name.to_owned(), pos));
    }
    let mut rows = Vec::new();
    walk(&root, "", 0, collapsed, &mut rows);
    rows
}

/// How the tree lists two paths: where they part, a directory before a file, then each
/// directory by name, as [`tree_rows`] lists them. Two files of one directory are level, as
/// the tree keeps them in the order they came.
pub fn folder_order(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.split('/').peekable(), b.split('/').peekable());
    loop {
        let (Some(x), Some(y)) = (a.next(), b.next()) else {
            return Ordering::Equal;
        };
        let (x_dir, y_dir) = (a.peek().is_some(), b.peek().is_some());
        match (x_dir, y_dir) {
            (false, false) => return Ordering::Equal,
            (true, true) if x == y => continue,
            _ => return y_dir.cmp(&x_dir).then_with(|| x.cmp(y)),
        }
    }
}

/// `a` before `b` in `sort`'s order: by path, or the last written first, with files of no
/// known write time, as a commit's, last by path.
pub fn file_order(sort: FileSort, a: &FileChange, b: &FileChange) -> Ordering {
    let by_path = a.path.cmp(&b.path);
    // Conflicted first, either way: they're what needs doing.
    b.conflicted.cmp(&a.conflicted).then(match sort {
        FileSort::Name => by_path,
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
    for (name, mut dir) in &node.dirs {
        let mut name = name.clone();
        while dir.files.is_empty()
            && dir.dirs.len() == 1
            && let Some((child, inner)) = dir.dirs.iter().next()
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
    rows.extend((node.files.iter()).map(|&(_, pos)| TreeRow::File { pos, depth }));
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

    #[test]
    fn directories_come_first_and_single_child_chains_merge() {
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
                dir("src", "src", 0, 3),
                dir("src/backend", "backend", 1, 2),
                TreeRow::File { pos: 1, depth: 2 },
                TreeRow::File { pos: 2, depth: 2 },
                dir("src/ui/diff", "ui/diff", 1, 1),
                TreeRow::File { pos: 3, depth: 2 },
                TreeRow::File { pos: 0, depth: 0 },
                TreeRow::File { pos: 4, depth: 0 },
            ],
            "`src` keeps two children so it stays; `ui` holds only `diff` so they merge; files follow directories in the order given"
        );
    }

    #[test]
    fn sorting_by_folder_order_lists_files_as_the_tree_does() {
        // In some order picked, as the newest first, which the tree keeps in each folder.
        let picked = [
            "src/checkout.rs",
            "README.md",
            "src/backend/mod.rs",
            "a-root.txt",
            "src/zeta/z.rs",
            "src/backend/ops.rs",
            "Cargo.toml",
        ];
        let listed: Vec<&str> = (tree_rows(&picked, &HashSet::new()).into_iter())
            .filter_map(|row| match row {
                TreeRow::File { pos, .. } => Some(picked[pos]),
                TreeRow::Dir { .. } => None,
            })
            .collect();
        let mut stepped = picked.to_vec();
        stepped.sort_by(|a, b| folder_order(a, b));
        assert_eq!(
            stepped, listed,
            "a diff in folder order steps through its files as the list shows them"
        );
        assert_eq!(
            listed,
            [
                "src/backend/mod.rs",
                "src/backend/ops.rs",
                "src/zeta/z.rs",
                "src/checkout.rs",
                "README.md",
                "a-root.txt",
                "Cargo.toml"
            ],
            "folders by name before files, the files of each in the order picked"
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
        for sort in [FileSort::Name, FileSort::Modified] {
            assert_eq!(
                file_order(sort, &conflicted, &plain),
                Ordering::Less,
                "{sort:?}: a conflicted file goes before an earlier name or a newer write"
            );
        }
    }
}
