use super::Repository;
use crate::git;
use std::{path, str};

use super::Result;

pub fn show_file_at_head<P: AsRef<path::Path>>(
    repository: &Repository,
    file_path: P,
) -> Result<String> {
    let file_path = file_path.as_ref();
    let head_tree = repository.head()?.peel_to_commit()?.tree()?;
    match head_tree.get_path(file_path) {
        Ok(tree_entry) => {
            let blob = repository.find_blob(tree_entry.id())?;
            let content = str::from_utf8(blob.content())?;
            Ok(content.to_string())
        }
        // If a file is new, it's content at HEAD is the empty string
        Err(_) => Ok(String::new()),
    }
}

pub fn show_file_at_tree<P: AsRef<path::Path>>(
    repository: &Repository,
    file_path: P,
    tree: &git::Tree,
) -> Result<String> {
    let file_path = file_path.as_ref();
    match tree.get_path(file_path) {
        Ok(tree_entry) => {
            let blob = repository.find_blob(tree_entry.id())?;
            let content = str::from_utf8(blob.content())?;
            Ok(content.to_string())
        }
        // If a file was introduced in this commit, the content in the parent tree is the empty string
        Err(_) => Ok(String::new()),
    }
}
