use but_graph::VirtualBranchesTomlMetadata;
use gitbutler_project::Project;
use std::path::Path;

pub fn project_from_path(path: &Path) -> anyhow::Result<Project> {
    Project::from_path(path)
}

pub fn project_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    let project = project_from_path(path)?;
    configured_repo(
        gix::open(project.worktree_path())?,
        RepositoryOpenMode::General,
    )
}
pub enum RepositoryOpenMode {
    // We'll need this later for the commit command
    Merge,
    General,
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
        RepositoryOpenMode::General => {
            repo.object_cache_size_if_unset(512 * 1024);
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
            gitbutler_project::list()?
                .into_iter()
                .find(|p| p.path == work_dir),
        )
    } else {
        (repo, None)
    };
    Ok(res)
}
