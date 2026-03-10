use std::collections::HashSet;

use anyhow::Result;

pub fn configured_submodule_paths(repo: &git2::Repository) -> Vec<String> {
    let mut paths = HashSet::new();

    collect_submodule_paths_from_repository(repo, &mut paths);
    collect_submodule_paths_from_gitmodules(repo, &mut paths);
    collect_submodule_paths_from_modules_dir(repo, &mut paths);

    let mut output = paths.into_iter().collect::<Vec<_>>();
    output.sort();
    output
}

fn collect_submodule_paths_from_repository(repo: &git2::Repository, paths: &mut HashSet<String>) {
    if let Ok(submodules) = repo.submodules() {
        for submodule in submodules {
            paths.insert(submodule.path().to_string_lossy().to_string());
        }
    }
}

fn collect_submodule_paths_from_gitmodules(repo: &git2::Repository, paths: &mut HashSet<String>) {
    let Some(workdir) = repo.workdir() else {
        return;
    };

    let gitmodules = workdir.join(".gitmodules");
    if !gitmodules.is_file() {
        return;
    }

    let Ok(config) = git2::Config::open(&gitmodules) else {
        return;
    };
    let Ok(mut entries) = config.entries(Some("submodule\\..*\\.path")) else {
        return;
    };

    while let Some(entry) = entries.next() {
        let Ok(entry) = entry else {
            continue;
        };
        let Some(path) = entry.value() else {
            continue;
        };
        if !path.is_empty() {
            paths.insert(path.to_owned());
        }
    }
}

fn collect_submodule_paths_from_modules_dir(repo: &git2::Repository, paths: &mut HashSet<String>) {
    let modules_root = repo.path().join("modules");
    if !modules_root.exists() {
        return;
    }

    let mut stack = vec![modules_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if path.join("HEAD").is_file()
                && let Ok(relative) = path.strip_prefix(&modules_root)
            {
                paths.insert(relative.to_string_lossy().to_string());
            }

            stack.push(path);
        }
    }
}

pub fn is_submodule_related_path(path: &str, submodule_paths: &[String]) -> bool {
    submodule_paths.iter().any(|sm| {
        path == sm
            || path
                .strip_prefix(sm)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// libgit2's checkout cleanup can't express "remove unrelated untracked files while preserving
/// populated submodule worktrees":
/// - an unrestricted `remove_untracked(true)` deletes the populated submodule worktree
/// - a path-limited `remove_untracked(true)` preserves the submodule, but also leaves unrelated
///   untracked files behind
///
/// When concrete submodule paths exist we therefore perform the checkout without libgit2 cleanup
/// and prune only the non-submodule untracked paths afterwards.
pub fn remove_untracked_excluding_submodule_paths(repo: &git2::Repository) -> Result<()> {
    let Some(workdir) = repo.workdir() else {
        return Ok(());
    };

    let submodule_paths = configured_submodule_paths(repo);
    let mut status_opts = git2::StatusOptions::new();
    status_opts
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .include_unmodified(false)
        .exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut status_opts))?;
    for status_entry in statuses.iter() {
        if !status_entry.status().contains(git2::Status::WT_NEW) {
            continue;
        }
        let Some(path) = status_entry.path() else {
            continue;
        };
        if is_submodule_related_path(path, &submodule_paths) {
            continue;
        }

        let absolute = workdir.join(path);
        if absolute.is_dir() {
            let _ = std::fs::remove_dir_all(&absolute);
        } else {
            let _ = std::fs::remove_file(&absolute);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use git2::build::CheckoutBuilder;
    use tempfile::TempDir;

    use super::{configured_submodule_paths, remove_untracked_excluding_submodule_paths};

    fn init_repo() -> (TempDir, git2::Repository) {
        let tempdir = TempDir::new().expect("tempdir should be created");
        let repo = git2::Repository::init(tempdir.path()).expect("repo should be initialized");
        (tempdir, repo)
    }

    #[test]
    fn configured_paths_include_gitmodules_entries() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        fs::write(
            workdir.join(".gitmodules"),
            "[submodule \"example\"]\n\tpath = example\n\turl = https://example.com/example.git\n",
        )
        .expect(".gitmodules should be created");

        assert_eq!(configured_submodule_paths(&repo), vec!["example"]);
    }

    #[test]
    fn configured_paths_include_modules_directory_entries() {
        let (_tempdir, repo) = init_repo();
        fs::create_dir_all(
            repo.path()
                .join("modules")
                .join("submodules")
                .join("test-module"),
        )
        .expect("modules dir should be created");
        fs::write(
            repo.path()
                .join("modules")
                .join("submodules")
                .join("test-module")
                .join("HEAD"),
            "ref: refs/heads/main\n",
        )
        .expect("head marker should be created");

        assert_eq!(
            configured_submodule_paths(&repo),
            vec!["submodules/test-module"]
        );
    }

    #[test]
    fn configured_paths_use_modules_directory_when_gitmodules_is_invalid() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        fs::write(workdir.join(".gitmodules"), "[submodule\n")
            .expect("gitmodules file should be made invalid");
        fs::create_dir_all(
            repo.path()
                .join("modules")
                .join("submodules")
                .join("test-module"),
        )
        .expect("modules dir should be created");
        fs::write(
            repo.path()
                .join("modules")
                .join("submodules")
                .join("test-module")
                .join("HEAD"),
            "ref: refs/heads/main\n",
        )
        .expect("head marker should be created");

        assert_eq!(
            configured_submodule_paths(&repo),
            vec!["submodules/test-module"]
        );
    }

    #[test]
    fn configured_paths_return_empty_when_no_submodule_signals_exist() {
        let (_tempdir, repo) = init_repo();

        assert!(configured_submodule_paths(&repo).is_empty());
    }

    fn commit_tracked_file(repo: &git2::Repository) {
        let workdir = repo.workdir().expect("non-bare repository");
        fs::write(workdir.join("tracked.txt"), "tracked\n")
            .expect("tracked file should be created");
        let mut index = repo.index().expect("index should open");
        index
            .add_path(Path::new("tracked.txt"))
            .expect("tracked file should be added");
        let tree_id = index.write_tree().expect("tree should be written");
        let tree = repo.find_tree(tree_id).expect("tree should exist");
        let signature =
            git2::Signature::now("test", "test@example.com").expect("signature should be created");
        repo.commit(Some("HEAD"), &signature, &signature, "init", &tree, &[])
            .expect("commit should succeed");
    }

    #[test]
    fn cleanup_removes_non_submodule_untracked_files() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        let untracked = workdir.join("tmp-untracked.txt");
        fs::write(&untracked, "tmp").expect("untracked file should be created");

        remove_untracked_excluding_submodule_paths(&repo).expect("cleanup should succeed");

        assert!(!untracked.exists());
    }

    #[test]
    fn cleanup_preserves_submodule_untracked_files() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        let modules_repo_path = repo
            .path()
            .join("modules")
            .join("submodules")
            .join("test-module");
        fs::create_dir_all(&modules_repo_path).expect("modules path should be created");
        fs::write(modules_repo_path.join("HEAD"), "ref: refs/heads/main\n")
            .expect("head marker should be created");

        let submodule_file = workdir
            .join("submodules")
            .join("test-module")
            .join("probe.txt");
        fs::create_dir_all(submodule_file.parent().expect("submodule parent"))
            .expect("submodule worktree path should be created");
        fs::write(&submodule_file, "probe").expect("submodule probe should be created");

        remove_untracked_excluding_submodule_paths(&repo).expect("cleanup should succeed");

        assert!(submodule_file.exists());
    }

    #[test]
    fn libgit2_remove_untracked_deletes_populated_submodule_worktrees() {
        let (_tempdir, repo) = init_repo();
        commit_tracked_file(&repo);
        let workdir = repo.workdir().expect("non-bare repository");
        let modules_repo_path = repo
            .path()
            .join("modules")
            .join("submodules")
            .join("test-module");
        fs::create_dir_all(&modules_repo_path).expect("modules path should be created");
        fs::write(modules_repo_path.join("HEAD"), "ref: refs/heads/main\n")
            .expect("head marker should be created");
        let submodule_file = workdir
            .join("submodules")
            .join("test-module")
            .join("probe.txt");
        fs::create_dir_all(submodule_file.parent().expect("submodule parent"))
            .expect("submodule worktree path should be created");
        fs::write(&submodule_file, "probe").expect("submodule probe should be created");

        let mut checkout = CheckoutBuilder::new();
        checkout.force().remove_untracked(true);
        repo.checkout_head(Some(&mut checkout))
            .expect("checkout should succeed");

        assert!(
            !submodule_file.exists(),
            "libgit2 remove_untracked(true) removes the populated submodule worktree"
        );
    }

    #[test]
    fn libgit2_path_limited_remove_untracked_leaves_unrelated_untracked_files_behind() {
        let (_tempdir, repo) = init_repo();
        commit_tracked_file(&repo);
        let workdir = repo.workdir().expect("non-bare repository");
        let ordinary = workdir.join("ordinary.txt");
        fs::write(&ordinary, "ordinary").expect("ordinary file should be created");
        let modules_repo_path = repo
            .path()
            .join("modules")
            .join("submodules")
            .join("test-module");
        fs::create_dir_all(&modules_repo_path).expect("modules path should be created");
        fs::write(modules_repo_path.join("HEAD"), "ref: refs/heads/main\n")
            .expect("head marker should be created");
        let submodule_file = workdir
            .join("submodules")
            .join("test-module")
            .join("probe.txt");
        fs::create_dir_all(submodule_file.parent().expect("submodule parent"))
            .expect("submodule worktree path should be created");
        fs::write(&submodule_file, "probe").expect("submodule probe should be created");

        let mut checkout = CheckoutBuilder::new();
        checkout.force().remove_untracked(true).path("tracked.txt");
        repo.checkout_head(Some(&mut checkout))
            .expect("checkout should succeed");

        assert!(
            ordinary.exists(),
            "path-limited remove_untracked(true) leaves unrelated untracked files behind"
        );
        assert!(
            submodule_file.exists(),
            "path-limited remove_untracked(true) preserves the submodule worktree"
        );
    }
}
