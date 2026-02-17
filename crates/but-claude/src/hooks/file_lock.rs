use but_ctx::Context;

pub(crate) fn obtain_or_insert(ctx: &mut Context, session_id: String, file_path: String) -> anyhow::Result<()> {
    let mut db = ctx.db.get_mut()?;
    let mut locks_mut = db.file_write_locks_mut();
    let max_wait_time = std::time::Duration::from_secs(30);
    let start = std::time::Instant::now();

    loop {
        let locks = locks_mut.to_ref().list()?;

        if let Some(lock) = locks.into_iter().find(|l| l.path == file_path) {
            if lock.owner == session_id {
                return Ok(());
            } else {
                if start.elapsed() > max_wait_time {
                    return Err(anyhow::anyhow!(
                        "Failed to obtain lock for {file_path} after waiting for {max_wait_time:?}"
                    ));
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        } else {
            let lock = but_db::FileWriteLock {
                path: file_path.clone(),
                created_at: chrono::Local::now().naive_local(),
                owner: session_id.clone(),
            };
            locks_mut
                .insert(lock)
                .map_err(|e| anyhow::anyhow!("Failed to insert lock: {e}"))?;
            return Ok(());
        }
    }
}

/// If file_path is provided, it will clear the lock for that file.
/// Otherwise, it will clear all locks for the session_id.
pub fn clear(ctx: &mut Context, session_id: String, file_path: Option<String>) -> anyhow::Result<()> {
    let mut db = ctx.db.get_mut()?;
    let mut trans = db.transaction()?;

    let locks = trans.file_write_locks().list()?;
    let locks_to_remove: Vec<_> = if let Some(ref path) = file_path {
        locks
            .into_iter()
            .filter(|l| l.path == *path && l.owner == session_id)
            .collect()
    } else {
        locks.into_iter().filter(|l| l.owner == session_id).collect()
    };

    for lock in &locks_to_remove {
        trans
            .file_write_locks_mut()
            .delete(&lock.path)
            .map_err(|e| anyhow::anyhow!("Failed to remove lock for path {}: {}", lock.path, e))?;
    }
    trans.commit()?;
    Ok(())
}
