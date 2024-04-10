use tempfile::TempDir;

use gitbutler_testsupport::init_opts_bare;

fn test_remote_repository() -> anyhow::Result<(git2::Repository, TempDir)> {
    let tmp = tempfile::tempdir()?;
    let repo_a = git2::Repository::init_opts(&tmp, &init_opts_bare())?;
    Ok((repo_a, tmp))
}

mod calculate_delta;
mod fetch_gitbutler_data;
mod git_file_change;
mod push_project_to_gitbutler;
