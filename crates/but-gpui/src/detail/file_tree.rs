//! Changed files as a tree of their directories. A directory holding nothing but one other
//! directory merges with it into one row (`src/backend`), as in Zed's project panel.

use std::collections::{BTreeMap, HashSet};

#[derive(Debug, PartialEq)]
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

/// The rows for `paths`, directories before files at each level, each sorted by name.
/// Nothing under a directory in `collapsed` is listed.
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

fn walk(
    node: &Node,
    parent: &str,
    depth: usize,
    collapsed: &HashSet<String>,
    rows: &mut Vec<TreeRow>,
) {
    for (name, mut dir) in &node.dirs {
        let mut name = name.clone();
        while dir.files.is_empty() && dir.dirs.len() == 1 {
            let (child, inner) = dir.dirs.iter().next().expect("one directory");
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
    let mut files = node.files.clone();
    files.sort();
    rows.extend(
        files
            .into_iter()
            .map(|(_, pos)| TreeRow::File { pos, depth }),
    );
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
                TreeRow::File { pos: 4, depth: 0 },
                TreeRow::File { pos: 0, depth: 0 },
            ],
            "`src` keeps two children so it stays; `ui` holds only `diff` so they merge; files sort by name after directories"
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
}
