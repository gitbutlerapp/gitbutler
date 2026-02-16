use anyhow::Result;
use git2::Repository;

pub fn reset_index(repo: &Repository, tree_id: git2::Oid) -> Result<()> {
    let mut index = repo.index()?;
    let tree = repo.find_tree(tree_id)?;
    index.read_tree(&tree)?;
    Ok(index.write()?)
}
