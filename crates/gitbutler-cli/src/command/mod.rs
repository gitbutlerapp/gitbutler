pub mod prepare;
pub mod project;
pub mod vbranch;

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

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}
