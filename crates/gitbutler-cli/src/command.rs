pub mod vbranch {
    use crate::command::debug_print;
    use futures::executor::block_on;
    use gitbutler_branch::{BranchCreateRequest, VirtualBranchesHandle};
    use gitbutler_branch_actions::VirtualBranchActions;
    use gitbutler_project::Project;

    pub fn list(project: Project) -> anyhow::Result<()> {
        let branches = VirtualBranchesHandle::new(project.gb_dir()).list_all_branches()?;
        for vbranch in branches {
            println!(
                "{active} {id} {name} {upstream}",
                active = if vbranch.applied { "✔️" } else { "⛌" },
                id = vbranch.id,
                name = vbranch.name,
                upstream = vbranch
                    .upstream
                    .map_or_else(Default::default, |b| b.to_string())
            );
        }
        Ok(())
    }

    pub fn create(project: Project, branch_name: String) -> anyhow::Result<()> {
        debug_print(block_on(VirtualBranchActions.create_virtual_branch(
            &project,
            &BranchCreateRequest {
                name: Some(branch_name),
                ..Default::default()
            },
        ))?)
    }
}

pub mod project {
    use crate::command::debug_print;
    use anyhow::{Context, Result};
    use futures::executor::block_on;
    use gitbutler_branch_actions::VirtualBranchActions;
    use gitbutler_project::Project;
    use gitbutler_reference::RemoteRefname;
    use std::path::PathBuf;

    pub fn list(ctrl: gitbutler_project::Controller) -> Result<()> {
        for project in ctrl.list()? {
            println!(
                "{id} {name} {path}",
                id = project.id,
                name = project.title,
                path = project.path.display()
            );
        }
        Ok(())
    }

    pub fn add(
        ctrl: gitbutler_project::Controller,
        path: PathBuf,
        refname: Option<RemoteRefname>,
    ) -> Result<()> {
        let path = gix::discover(path)?
            .work_dir()
            .context("Only non-bare repositories can be added")?
            .to_owned()
            .canonicalize()?;
        let project = ctrl.add(path)?;
        if let Some(refname) = refname {
            block_on(VirtualBranchActions.set_base_branch(&project, &refname))?;
        };
        debug_print(project)
    }

    pub fn switch_to_integration(project: Project, refname: RemoteRefname) -> Result<()> {
        debug_print(block_on(
            VirtualBranchActions.set_base_branch(&project, &refname),
        )?)
    }
}
pub mod snapshot {
    use anyhow::Result;
    use gitbutler_oplog::OplogExt;
    use gitbutler_project::Project;

    pub fn list(project: Project) -> Result<()> {
        let snapshots = project.list_snapshots(100, None)?;
        for snapshot in snapshots {
            let ts = chrono::DateTime::from_timestamp(snapshot.created_at.seconds(), 0);
            let details = snapshot.details;
            if let (Some(ts), Some(details)) = (ts, details) {
                println!("{} {} {}", ts, snapshot.commit_id, details.operation);
            }
        }
        Ok(())
    }

    pub fn restore(project: Project, snapshot_id: String) -> Result<()> {
        let _guard = project.try_exclusive_access()?;
        project.restore_snapshot(snapshot_id.parse()?)?;
        Ok(())
    }
}

pub mod prepare {
    use anyhow::{bail, Context};
    use gitbutler_project::Project;
    use std::path::PathBuf;

    pub fn project_from_path(path: PathBuf) -> anyhow::Result<Project> {
        let worktree_dir = gix::discover(path)?
            .work_dir()
            .context("Bare repositories aren't supported")?
            .to_owned();
        Ok(Project {
            path: worktree_dir,
            ..Default::default()
        })
    }

    pub fn project_controller(
        app_suffix: Option<String>,
        app_data_dir: Option<PathBuf>,
    ) -> anyhow::Result<gitbutler_project::Controller> {
        let path = if let Some(dir) = app_data_dir {
            std::fs::create_dir_all(&dir)
                .context("Failed to assure the designated data-dir exists")?;
            dir
        } else {
            dirs_next::data_dir()
                .map(|dir| {
                    dir.join(format!(
                        "com.gitbutler.app{}",
                        app_suffix
                            .map(|mut suffix| {
                                suffix.insert(0, '.');
                                suffix
                            })
                            .unwrap_or_default()
                    ))
                })
                .context("no data-directory available on this platform")?
        };
        if !path.is_dir() {
            bail!("Path '{}' must be a valid directory", path.display());
        }
        eprintln!("Using projects from '{}'", path.display());
        Ok(gitbutler_project::Controller::from_path(path))
    }
}

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    eprintln!("{:#?}", this);
    Ok(())
}
