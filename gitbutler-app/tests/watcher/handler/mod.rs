use crate::init_opts_bare;

fn test_remote_repository() -> anyhow::Result<git2::Repository> {
    let path = tempfile::tempdir()?.path().to_str().unwrap().to_string();
    let repo_a = git2::Repository::init_opts(path, &init_opts_bare())?;

    Ok(repo_a)
}

mod calculate_delta_handler;
mod fetch_gitbutler_data;
mod git_file_change;
mod push_project_to_gitbutler;
