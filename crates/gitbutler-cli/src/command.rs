pub mod vbranch {
    use anyhow::{bail, Result};
    use gitbutler_branch::{
        Branch, BranchCreateRequest, BranchUpdateRequest, VirtualBranchesHandle,
    };
    use gitbutler_branch_actions::VirtualBranchActions;
    use gitbutler_project::Project;

    use crate::command::debug_print;

    pub fn list(project: Project) -> Result<()> {
        let branches = VirtualBranchesHandle::new(project.gb_dir()).list_all_branches()?;
        for vbranch in branches {
            println!(
                "{active} {id} {name} {upstream} {default}",
                active = if vbranch.applied { "✔️" } else { "⛌" },
                id = vbranch.id,
                name = vbranch.name,
                upstream = vbranch
                    .upstream
                    .map_or_else(Default::default, |b| b.to_string()),
                default = if vbranch.in_workspace { "🌟" } else { "" }
            );
        }
        Ok(())
    }

    pub fn unapply(project: Project, branch_name: String) -> Result<()> {
        let branch = branch_by_name(&project, &branch_name)?;
        debug_print(VirtualBranchActions.convert_to_real_branch(&project, branch.id)?)
    }

    pub fn create(project: Project, branch_name: String, set_default: bool) -> Result<()> {
        let new = VirtualBranchActions.create_virtual_branch(
            &project,
            &BranchCreateRequest {
                name: Some(branch_name),
                ..Default::default()
            },
        )?;
        if set_default {
            let new = VirtualBranchesHandle::new(project.gb_dir()).get_branch(new)?;
            set_default_branch(&project, &new)?;
        }
        debug_print(new)
    }

    pub fn set_default(project: Project, branch_name: String) -> Result<()> {
        let branch = branch_by_name(&project, &branch_name)?;
        set_default_branch(&project, &branch)
    }

    fn set_default_branch(project: &Project, branch: &Branch) -> Result<()> {
        VirtualBranchActions.update_virtual_branch(
            project,
            BranchUpdateRequest {
                id: branch.id,
                name: None,
                notes: None,
                ownership: None,
                order: None,
                upstream: None,
                selected_for_changes: Some(true),
                allow_rebasing: None,
            },
        )
    }

    pub fn commit(project: Project, branch_name: String, message: String) -> Result<()> {
        let branch = branch_by_name(&project, &branch_name)?;
        let (info, skipped) = VirtualBranchActions.list_virtual_branches(&project)?;

        if !skipped.is_empty() {
            eprintln!(
                "{} files could not be processed (binary or large size)",
                skipped.len()
            )
        }

        let populated_branch = info
            .iter()
            .find(|b| b.id == branch.id)
            .expect("A populated branch exists for a branch we can list");
        if populated_branch.ownership.claims.is_empty() {
            bail!(
                "Branch '{branch_name}' has no change to commit{hint}",
                hint = {
                    let candidate_names = info
                        .iter()
                        .filter_map(|b| (!b.ownership.claims.is_empty()).then_some(b.name.as_str()))
                        .collect::<Vec<_>>();
                    let mut candidates = candidate_names.join(", ");
                    if !candidate_names.is_empty() {
                        candidates = format!(
                            ". {candidates} {have} changes.",
                            have = if candidate_names.len() == 1 {
                                "has"
                            } else {
                                "have"
                            }
                        )
                    };
                    candidates
                }
            )
        }

        let run_hooks = false;
        debug_print(VirtualBranchActions.create_commit(
            &project,
            branch.id,
            &message,
            Some(&populated_branch.ownership),
            run_hooks,
        )?)
    }

    pub fn branch_by_name(project: &Project, name: &str) -> Result<Branch> {
        let mut found: Vec<_> = VirtualBranchesHandle::new(project.gb_dir())
            .list_all_branches()?
            .into_iter()
            .filter(|b| b.name == name)
            .collect();
        if found.is_empty() {
            bail!("No virtual branch named '{name}'");
        } else if found.len() > 1 {
            bail!("Found more than one virtual branch named '{name}'");
        }
        Ok(found.pop().expect("present"))
    }
}

pub mod project {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use gitbutler_branch_actions::VirtualBranchActions;
    use gitbutler_project::Project;
    use gitbutler_reference::RemoteRefname;

    use crate::command::debug_print;

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
            VirtualBranchActions.set_base_branch(&project, &refname)?;
        };
        debug_print(project)
    }

    pub fn switch_to_integration(project: Project, refname: RemoteRefname) -> Result<()> {
        debug_print(VirtualBranchActions.set_base_branch(&project, &refname)?)
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
    use std::path::PathBuf;

    use anyhow::{bail, Context};
    use gitbutler_project::Project;

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
