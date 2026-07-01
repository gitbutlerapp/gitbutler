use std::{
    collections::VecDeque,
    ffi::{OsStr, OsString},
    path::Component,
};

use bstr::ByteVec as _;
use but_ctx::Context;
use indexmap::IndexMap;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem},
};

use crate::CliId;

#[derive(Debug, Default)]
pub struct FileBrowser {
    selection: Option<CliId>,
    tree: FileTree,
}

impl FileBrowser {
    pub fn needs_update(&self, is_visible: bool) -> bool {
        is_visible
    }

    pub fn update(&mut self, ctx: &mut Context, selection: Option<&CliId>) -> anyhow::Result<bool> {
        match (selection, self.selection.as_ref()) {
            (None, None) => Ok(false),
            (None, Some(_)) => {
                self.tree.0.clear();
                Ok(true)
            }
            (Some(new), None) => {
                self.recompute_paths(ctx, new)?;
                Ok(true)
            }
            (Some(new), Some(prev)) => {
                if new == prev {
                    Ok(false)
                } else {
                    self.recompute_paths(ctx, new)?;
                    Ok(true)
                }
            }
        }
    }

    fn recompute_paths(&mut self, ctx: &mut Context, selection: &CliId) -> anyhow::Result<()> {
        self.tree.0.clear();

        let paths = match selection {
            CliId::Uncommitted { .. } => but_api::diff::changes_in_worktree(ctx)?
                .worktree_changes
                .changes
                .into_iter()
                .map(|change| change.path_bytes.to_vec().into_path_buf_lossy())
                .collect::<Vec<_>>(),
            CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Commit { .. }
            | CliId::Stack { .. } => return Ok(()),
        };

        for path in paths {
            let mut path_components = path.components().filter_map(|comp| match comp {
                Component::Normal(os_str) => Some(os_str),
                Component::Prefix(..)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => None,
            });
            update_file_tree(&mut self.tree, &mut path_components);
        }

        self.tree.sort();

        Ok(())
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        if self.tree.0.is_empty() {
            frame.render_widget(ratatui::widgets::Clear, area);
            return;
        }

        let mut items = Vec::<ListItem<'_>>::new();

        let mut todo = VecDeque::from_iter(self.tree.0.iter().map(|tree| (tree, 0)));
        while let Some(((path, sub_tree), depth)) = todo.pop_front() {
            items.push(ListItem::from(Line::from_iter([
                Span::raw("  ".repeat(depth)),
                Span::raw(path.to_string_lossy()),
                Span::raw(if sub_tree.0.is_empty() { "" } else { "/" }),
            ])));

            for child in &sub_tree.0 {
                todo.push_front((child, depth + 1));
            }
        }

        let list = List::new(items);
        frame.render_widget(list, area);
    }
}

#[derive(Debug, Default)]
struct FileTree(IndexMap<OsString, FileTree>);

impl FileTree {
    fn sort(&mut self) {
        let mut todo = Vec::from([self]);
        while let Some(tree) = todo.pop() {
            tree.0.sort_keys();
            for sub_tree in tree.0.values_mut() {
                todo.push(sub_tree);
            }
        }
    }
}

fn update_file_tree(mut tree: &mut FileTree, iter: &mut dyn Iterator<Item = &OsStr>) {
    for component in iter {
        tree = tree.0.entry(component.to_owned()).or_default();
    }
}
