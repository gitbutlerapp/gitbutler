use std::{path, str};

use super::{Repository, Result};
use crate::git;

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
