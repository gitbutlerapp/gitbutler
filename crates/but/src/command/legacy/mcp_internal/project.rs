use std::path::Path;

use but_meta::VirtualBranchesTomlMetadata;
use gitbutler_project::Project;

pub fn project_from_path(path: &Path) -> anyhow::Result<Project> {
    Project::from_path(path)
}

pub enum RepositoryOpenMode {
    // We'll need this later for the commit command
    Merge,
}

fn configured_repo(
    mut repo: gix::Repository,
    mode: RepositoryOpenMode,
) -> anyhow::Result<gix::Repository> {
    match mode {
        RepositoryOpenMode::Merge => {
            let bytes = repo.compute_object_cache_size_for_tree_diffs(&***repo.index_or_empty()?);
            repo.object_cache_size_if_unset(bytes);
        }
    }
    Ok(repo)
}

pub fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

pub fn repo_and_maybe_project(
    current_dir: &Path,
    mode: RepositoryOpenMode,
) -> anyhow::Result<(gix::Repository, Option<Project>)> {
    let repo = configured_repo(gix::discover(current_dir)?, mode)?;
    let res = if let Some(work_dir) = repo.workdir() {
        let work_dir = gix::path::realpath(work_dir)?;
        (
            repo,
            gitbutler_project::Project::find_by_worktree_dir(&work_dir).ok(),
        )
    } else {
        (repo, None)
    };
    Ok(res)
}
