use anyhow::{Context, Result};

use crate::fs;

pub trait Reader {
    fn read_to_string(&self, file_path: &str) -> Result<String>;
    fn list_files(&self, dir_path: &str) -> Result<Vec<String>>;
}

pub struct WdReader<'reader> {
    git_repository: &'reader git2::Repository,
}

impl WdReader<'_> {
    pub fn read_to_string(&self, path: &str) -> Result<String> {
        let contents =
            std::fs::read_to_string(self.git_repository.path().parent().unwrap().join(path))
                .with_context(|| format!("{}: not found", path))?;
        Ok(contents)
    }
}

impl Reader for WdReader<'_> {
    fn read_to_string(&self, path: &str) -> Result<String> {
        self.read_to_string(path)
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        let files: Vec<String> =
            fs::list_files(self.git_repository.path().parent().unwrap().join(dir_path))?
                .iter()
                .map(|f| f.to_str().unwrap().to_string())
                .filter(|f| !f.starts_with(".git"))
                .collect();
        Ok(files)
    }
}

pub fn get_working_directory_reader(git_repository: &git2::Repository) -> WdReader {
    WdReader { git_repository }
}

pub struct CommitReader<'reader> {
    repository: &'reader git2::Repository,
    commit_oid: git2::Oid,
    tree: git2::Tree<'reader>,
}

impl CommitReader<'_> {
    pub fn get_commit_oid(&self) -> git2::Oid {
        self.commit_oid
    }
}

impl Reader for CommitReader<'_> {
    fn read_to_string(&self, path: &str) -> Result<String> {
        let entry = self
            .tree
            .get_path(std::path::Path::new(path))
            .with_context(|| format!("{}: tree entry not found", path))?;
        let blob = self
            .repository
            .find_blob(entry.id())
            .with_context(|| format!("{}: blob not found", entry.id()))?;
        let contents = String::from_utf8_lossy(blob.content()).to_string();
        Ok(contents)
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        let mut files: Vec<String> = Vec::new();
        let repo_root = self.repository.path().parent().unwrap();
        self.tree
            .walk(git2::TreeWalkMode::PreOrder, |root, entry| {
                if entry.name().is_none() {
                    return git2::TreeWalkResult::Ok;
                }

                let abs_dir_path = repo_root.join(dir_path);
                let abs_entry_path = repo_root.join(root).join(entry.name().unwrap());
                if !abs_entry_path.starts_with(&abs_dir_path) {
                    return git2::TreeWalkResult::Ok;
                }
                if abs_dir_path.eq(&abs_entry_path) {
                    return git2::TreeWalkResult::Ok;
                }
                if entry.kind() == Some(git2::ObjectType::Tree) {
                    return git2::TreeWalkResult::Ok;
                }

                let relpath = abs_entry_path.strip_prefix(abs_dir_path).unwrap();

                files.push(relpath.to_str().unwrap().to_string());

                git2::TreeWalkResult::Ok
            })
            .with_context(|| format!("{}: tree walk failed", dir_path))?;

        Ok(files)
    }
}

pub fn get_commit_reader<'reader>(
    repository: &'reader git2::Repository,
    commit_oid: git2::Oid,
) -> Result<CommitReader<'reader>> {
    let commit = repository
        .find_commit(commit_oid)
        .with_context(|| format!("{}: commit not found", commit_oid))?;
    let tree = commit
        .tree()
        .with_context(|| format!("{}: tree not found", commit_oid))?;
    Ok(CommitReader {
        repository,
        tree,
        commit_oid,
    })
}
