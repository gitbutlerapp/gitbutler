use gitbutler_project::Project;
use std::path::PathBuf;

pub fn project_from_path(path: PathBuf) -> anyhow::Result<Project> {
    Project::from_path(&path)
}

pub fn project_repo(path: PathBuf) -> anyhow::Result<gix::Repository> {
    let project = project_from_path(path)?;
    Ok(gix::open(project.worktree_path())?)
}

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

pub mod status {
    use crate::command::{debug_print, project_repo};
    use std::path::PathBuf;

    pub fn doit(current_dir: PathBuf, unified_diff: bool) -> anyhow::Result<()> {
        let repo = project_repo(current_dir)?;
        let worktree = but_core::diff::worktree_status(&repo)?;
        if unified_diff {
            debug_print((
                worktree
                    .changes
                    .into_iter()
                    .map(|tree_change| {
                        tree_change
                            .unified_diff(&repo)
                            .map(|diff| (tree_change, diff))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                worktree.ignored_changes,
            ))
        } else {
            debug_print(worktree)
        }
    }
}

pub mod stacks {
    use std::path::PathBuf;

    use but_workspace::stack_branches;
    use gitbutler_command_context::CommandContext;
    use gitbutler_settings::AppSettings;

    use crate::command::{debug_print, project_from_path};

    pub fn list(current_dir: PathBuf) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        debug_print(but_workspace::stacks(&project.gb_dir()))
    }

    pub fn branches(id: String, current_dir: PathBuf) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        let ctx = CommandContext::open(&project, AppSettings::default())?;
        debug_print(stack_branches(id, &ctx))
    }
}
