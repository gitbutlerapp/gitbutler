use std::path::{Path, PathBuf};

use but_path::AppChannel;
use but_project_handle::{ProjectHandle, gitbutler_storage_path, storage_path_config_key};
use but_testsupport::{CommandExt as _, git, gix_testtools, open_repo};
use gix_testtools::tempfile::TempDir;

fn init_repo() -> anyhow::Result<(TempDir, gix::Repository)> {
    let tmp = TempDir::new()?;
    gix::init(tmp.path())?;
    let repo = open_repo(tmp.path())?;
    Ok((tmp, repo))
}

fn set_git_config(
    repo: &gix::Repository,
    key: &str,
    value: impl AsRef<std::ffi::OsStr>,
) -> anyhow::Result<gix::Repository> {
    git(repo).args(["config", "--local", key]).arg(value).run();
    // Have to reopen the repository for it to refresh its loaded Git configuration.
    open_repo(repo.path())
}

fn default_storage_dir_name() -> &'static str {
    match AppChannel::new() {
        AppChannel::Release => "gitbutler",
        AppChannel::Nightly => "gitbutler.nightly",
        AppChannel::Dev => "gitbutler.dev",
    }
}

#[test]
fn storage_path_from_relative_config() -> anyhow::Result<()> {
    let (_tmp, repo) = init_repo()?;
    let key = storage_path_config_key();

    let repo = set_git_config(&repo, key, "gitbutler-custom")?;

    assert_eq!(
        gitbutler_storage_path(&repo)?,
        repo.git_dir().join("gitbutler-custom")
    );
    Ok(())
}

#[test]
fn storage_path_from_relative_config_outside_git_dir_is_project_unique() -> anyhow::Result<()> {
    let (_tmp, repo) = init_repo()?;
    let key = storage_path_config_key();

    let repo = set_git_config(&repo, key, "../../gitbutler-shared")?;
    let configured_base = repo
        .git_dir()
        .parent()
        .expect("git dir has repo parent")
        .parent()
        .expect("repo parent has tempdir parent")
        .join("gitbutler-shared");
    let expected = configured_base.join(ProjectHandle::from_path(repo.git_dir())?.to_string());

    assert_eq!(gitbutler_storage_path(&repo)?, expected);
    Ok(())
}

#[test]
fn storage_path_from_absolute_config_inside_git_dir() -> anyhow::Result<()> {
    let (_tmp, repo) = init_repo()?;
    let key = storage_path_config_key();
    let custom_path = repo.git_dir().join("gitbutler-custom");

    let repo = set_git_config(&repo, key, &custom_path)?;

    assert_eq!(gitbutler_storage_path(&repo)?, custom_path);
    Ok(())
}

#[test]
fn storage_path_from_absolute_config_outside_git_dir_is_project_unique() -> anyhow::Result<()> {
    let (_tmp, repo) = init_repo()?;
    let key = storage_path_config_key();
    let configured_base = if cfg!(windows) {
        PathBuf::from(r"C:\gitbutler-storage")
    } else {
        PathBuf::from("/tmp/gitbutler-storage")
    };

    let repo = set_git_config(&repo, key, &configured_base)?;
    let expected = configured_base.join(ProjectHandle::from_path(repo.git_dir())?.to_string());

    assert_eq!(gitbutler_storage_path(&repo)?, expected);
    Ok(())
}

#[test]
fn storage_path_default_stays_in_git_dir() -> anyhow::Result<()> {
    let (_tmp, repo) = init_repo()?;
    let expected = repo.git_dir().join(default_storage_dir_name());

    assert_eq!(gitbutler_storage_path(&repo)?, expected);
    Ok(())
}

#[test]
fn docs_examples_are_viable_paths() -> anyhow::Result<()> {
    let (_tmp, repo) = init_repo()?;
    let key = storage_path_config_key();
    let examples = [
        Path::new("gitbutler-alt").to_path_buf(),
        Path::new("custom/gitbutler").to_path_buf(),
        Path::new("../gitbutler-projects").to_path_buf(),
        if cfg!(windows) {
            PathBuf::from(r"C:\gitbutler-projects")
        } else {
            PathBuf::from("/tmp/gitbutler-projects")
        },
    ];

    for example in examples {
        let repo = set_git_config(&repo, key, &example)?;
        let resolved = gitbutler_storage_path(&repo)?;
        assert!(resolved.is_absolute());
    }
    Ok(())
}
