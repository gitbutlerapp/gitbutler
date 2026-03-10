use std::collections::HashSet;

use anyhow::Result;

pub fn has_submodules_configured(repo: &git2::Repository) -> bool {
    if repo
        .workdir()
        .is_some_and(|workdir| workdir.join(".gitmodules").exists())
    {
        return true;
    }

    let Ok(config) = repo.config() else {
        return false;
    };

    // `repo.path()` points to the repository git-dir (for non-bare repos: `.git`),
    // so this checks `.git/modules` rather than a `modules` folder in the worktree root.
    let modules_dir_has_entries = repo
        .path()
        .join("modules")
        .read_dir()
        .map(|mut it| it.next().is_some())
        .unwrap_or(false);

    let Ok(mut entries) = config.entries(Some("submodule\\..*\\.url")) else {
        return modules_dir_has_entries;
    };

    entries.next().transpose().ok().flatten().is_some() || modules_dir_has_entries
}

pub fn configured_submodule_paths(repo: &git2::Repository) -> Vec<String> {
    let mut paths = HashSet::new();

    if let Ok(submodules) = repo.submodules() {
        for submodule in submodules {
            paths.insert(submodule.path().to_string_lossy().to_string());
        }
    }

    let modules_root = repo.path().join("modules");
    if modules_root.exists() {
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

    let mut output = paths.into_iter().collect::<Vec<_>>();
    output.sort();
    output
}

pub fn is_submodule_related_path(path: &str, submodule_paths: &[String]) -> bool {
    submodule_paths
        .iter()
        .any(|sm| path == sm || path.strip_prefix(sm).is_some_and(|rest| rest.starts_with('/')))
}

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

    use tempfile::TempDir;

    use super::{
        has_submodules_configured, remove_untracked_excluding_submodule_paths,
    };

    fn init_repo() -> (TempDir, git2::Repository) {
        let tempdir = TempDir::new().expect("tempdir should be created");
        let repo = git2::Repository::init(tempdir.path()).expect("repo should be initialized");
        (tempdir, repo)
    }

    #[test]
    fn detects_submodules_from_gitmodules_file() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        fs::write(workdir.join(".gitmodules"), "[submodule \"example\"]\n")
            .expect(".gitmodules should be created");

        assert!(has_submodules_configured(&repo));
    }

    #[test]
    fn detects_submodules_from_config_entries() {
        let (_tempdir, repo) = init_repo();
        {
            let mut config = repo.config().expect("repo config should open");
            config
                .set_str("submodule.example.url", "https://example.com/example.git")
                .expect("submodule config should be written");
        }

        assert!(has_submodules_configured(&repo));
    }

    #[test]
    fn detects_submodules_from_git_modules_directory_entries() {
        let (_tempdir, repo) = init_repo();
        fs::create_dir_all(repo.path().join("modules").join("example"))
            .expect("modules dir should be created");

        assert!(has_submodules_configured(&repo));
    }

    #[test]
    fn uses_modules_directory_fallback_when_config_entries_cannot_be_read() {
        let (_tempdir, repo) = init_repo();
        fs::create_dir_all(repo.path().join("modules").join("example"))
            .expect("modules dir should be created");
        fs::write(repo.path().join("config"), "[core\n")
            .expect("config file should be made invalid");

        assert!(has_submodules_configured(&repo));
    }

    #[test]
    fn returns_false_when_no_submodule_signals_exist() {
        let (_tempdir, repo) = init_repo();

        assert!(!has_submodules_configured(&repo));
    }

    #[test]
    fn cleanup_removes_non_submodule_untracked_files() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        let untracked = workdir.join("tmp-untracked.txt");
        fs::write(&untracked, "tmp").expect("untracked file should be created");

        remove_untracked_excluding_submodule_paths(&repo)
            .expect("cleanup should succeed");

        assert!(!untracked.exists());
    }

    #[test]
    fn cleanup_preserves_submodule_untracked_files() {
        let (_tempdir, repo) = init_repo();
        let workdir = repo.workdir().expect("non-bare repository");
        let modules_repo_path = repo.path().join("modules").join("submodules").join("test-module");
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

        remove_untracked_excluding_submodule_paths(&repo)
            .expect("cleanup should succeed");

        assert!(submodule_file.exists());
    }
}
