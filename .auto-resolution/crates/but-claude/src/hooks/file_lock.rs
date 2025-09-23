use gitbutler_command_context::CommandContext;

pub(crate) fn obtain(
    ctx: &mut CommandContext,
    session_id: String,
    file_path: String,
) -> anyhow::Result<()> {
    let mut db = ctx.db()?.file_write_locks();
    let max_wait_time = std::time::Duration::from_secs(60 * 10);
    let start = std::time::Instant::now();

    loop {
        let locks = db.list()?;

        if let Some(lock) = locks.into_iter().find(|l| l.path == file_path) {
            if lock.owner == session_id {
                // We already own the lock, so we can proceed
                return Ok(());
            } else {
                // Another session owns the lock, wait and retry, but not indefinitely
                if start.elapsed() > max_wait_time {
                    return Err(anyhow::anyhow!(
                        "Failed to obtain lock for {} after waiting for {:?}",
                        file_path,
                        max_wait_time
                    ));
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        } else {
            // Create a lock entry
            let lock = but_db::FileWriteLock {
                path: file_path.clone(),
                created_at: chrono::Local::now().naive_local(),
                owner: session_id.clone(),
            };
            db.insert(lock)
                .map_err(|e| anyhow::anyhow!("Failed to insert lock: {}", e))?;
            return Ok(());
        }
    }
}

/// If file_path is provided, it will clear the lock for that file.
/// Otherwise, it will clear all locks for the session_id.
pub fn clear(
    ctx: &mut CommandContext,
    session_id: String,
    file_path: Option<String>,
) -> anyhow::Result<()> {
    let mut db = ctx.db()?.file_write_locks();

    let locks = db.list()?;
    let locks_to_remove: Vec<_> = if let Some(path) = file_path {
        locks
            .into_iter()
            .filter(|l| l.path == path && l.owner == session_id)
            .collect()
    } else {
        locks
            .into_iter()
            .filter(|l| l.owner == session_id)
            .collect()
    };

    for lock in locks_to_remove {
        db.delete(&lock.path)
            .map_err(|e| anyhow::anyhow!("Failed to remove lock for path {}: {}", lock.path, e))?;
    }
    Ok(())
}
