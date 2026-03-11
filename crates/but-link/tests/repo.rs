#[expect(dead_code)]
#[path = "../src/cli.rs"]
mod cli;
#[expect(dead_code)]
#[path = "../src/repo.rs"]
mod repo_impl;

use repo_impl::resolve_repo_relative_path;
use tempfile::TempDir;

#[test]
fn resolve_repo_relative_path_maps_subdir_inputs_to_repo_relative() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let repo_root = tempdir.path().join("repo");
    let nested = repo_root.join("src").join("nested");
    std::fs::create_dir_all(&nested)?;
    std::fs::create_dir(repo_root.join(".git"))?;

    let resolved = resolve_repo_relative_path("lib.rs", &nested, &repo_root)?;

    assert_eq!(resolved, "src/nested/lib.rs");
    Ok(())
}

#[test]
fn resolve_repo_relative_path_rejects_outside_repo() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let repo_root = tempdir.path().join("repo");
    let nested = repo_root.join("src");
    std::fs::create_dir_all(&nested)?;
    std::fs::create_dir(repo_root.join(".git"))?;

    let err = resolve_repo_relative_path("../../elsewhere.rs", &nested, &repo_root)
        .expect_err("outside-repo paths must be rejected");

    assert!(err.to_string().contains("path must stay within repository"));
    Ok(())
}
