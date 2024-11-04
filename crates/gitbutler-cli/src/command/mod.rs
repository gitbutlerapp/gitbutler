pub mod prepare;
pub mod project;
pub mod vbranch;
pub mod snapshot {
    use crate::command::debug_print;
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

    pub fn diff(project: Project, snapshot_id: String) -> Result<()> {
        debug_print(project.snapshot_diff(snapshot_id.parse()?))
    }
}

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

pub mod ownership {
    use gitbutler_diff::Hunk;
    use gitbutler_project::Project;
    use gitbutler_stack::{BranchOwnershipClaims, OwnershipClaim};
    use std::path::PathBuf;

    pub fn unapply(
        project: Project,
        file_path: PathBuf,
        from_line: u32,
        to_line: u32,
    ) -> anyhow::Result<()> {
        let claims = BranchOwnershipClaims {
            claims: vec![OwnershipClaim {
                file_path,
                hunks: vec![Hunk {
                    hash: None,
                    start: from_line,
                    end: to_line,
                }],
            }],
        };
        gitbutler_branch_actions::unapply_ownership(&project, &claims)
    }
}

pub mod workspace {
    use crate::args::UpdateMode;
    use gitbutler_branch_actions::upstream_integration;
    use gitbutler_project::Project;

    pub fn update(project: Project, mode: UpdateMode) -> anyhow::Result<()> {
        let approach = match mode {
            UpdateMode::Rebase => upstream_integration::ResolutionApproach::Rebase,
            UpdateMode::Merge => upstream_integration::ResolutionApproach::Merge,
            UpdateMode::Unapply => upstream_integration::ResolutionApproach::Unapply,
            UpdateMode::Delete => upstream_integration::ResolutionApproach::Delete,
        };
        let resolutions: Vec<_> = gitbutler_branch_actions::list_virtual_branches(&project)?
            .0
            .into_iter()
            .map(|b| upstream_integration::Resolution {
                branch_id: b.id,
                branch_tree: b.tree,
                approach,
            })
            .collect();
        gitbutler_branch_actions::integrate_upstream(&project, &resolutions, None)
    }
}
