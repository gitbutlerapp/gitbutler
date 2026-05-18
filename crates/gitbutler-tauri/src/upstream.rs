use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::Context as _;
use but_api::{json, legacy::virtual_branches};
use but_ctx::{ProjectHandleOrLegacyProjectId, ThreadSafeContext};
use gitbutler_branch_actions::upstream_integration::{
    BaseBranchResolution, IntegrationOutcome, Resolution,
};
use tauri::{AppHandle, Emitter, EventTarget, Manager, Window};
use tracing::instrument;

const GIT_LFS_PROGRESS_ENV: &str = "GIT_LFS_PROGRESS";

fn workspace_update_progress_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[derive(Debug, Clone, PartialEq)]
struct LfsProgressLine {
    direction: String,
    current_file: u64,
    total_files: u64,
    downloaded_bytes: u64,
    total_bytes: u64,
    path: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct WorkspaceUpdateProgress {
    direction: String,
    current_file: u64,
    total_files: u64,
    file_downloaded_bytes: u64,
    file_total_bytes: u64,
    progress_percent: f64,
    bytes_per_second: Option<f64>,
    path: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct FileProgress {
    total_bytes: u64,
    downloaded_bytes: u64,
}

#[derive(Default)]
struct LfsProgressTracker {
    files: BTreeMap<u64, FileProgress>,
    last_current_file: Option<u64>,
    last_sample: Option<(Instant, u64)>,
    last_speed: Option<f64>,
}

struct WorkspaceUpdateProgressScope {
    progress_path: PathBuf,
    previous_progress_path: Option<OsString>,
    stop: Arc<AtomicBool>,
    monitor: Option<JoinHandle<()>>,
}

impl WorkspaceUpdateProgressScope {
    fn new(
        window: Window,
        project_id: &ProjectHandleOrLegacyProjectId,
    ) -> anyhow::Result<WorkspaceUpdateProgressScope> {
        let progress_path = std::env::temp_dir().join(format!(
            "gitbutler-lfs-progress-{}-{}.log",
            project_id,
            uuid::Uuid::new_v4()
        ));
        File::create(&progress_path).with_context(|| {
            format!(
                "failed to create Git LFS progress file at '{}'",
                progress_path.display()
            )
        })?;

        let previous_progress_path = std::env::var_os(GIT_LFS_PROGRESS_ENV);
        // SAFETY: we serialize these temporary environment mutations behind a process-wide async
        // mutex and restore the previous value before releasing it again.
        unsafe {
            std::env::set_var(GIT_LFS_PROGRESS_ENV, &progress_path);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let monitor = Some(spawn_progress_monitor(
            window.app_handle().clone(),
            window.label().to_owned(),
            format!("project://{project_id}/workspace_update_progress"),
            progress_path.clone(),
            stop.clone(),
        ));

        Ok(WorkspaceUpdateProgressScope {
            progress_path,
            previous_progress_path,
            stop,
            monitor,
        })
    }

    fn finish(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(monitor) = self.monitor.take() {
            let _ = monitor.join();
        }
        restore_lfs_progress_env(self.previous_progress_path.as_ref());
        let _ = std::fs::remove_file(&self.progress_path);
    }
}

fn restore_lfs_progress_env(previous_value: Option<&OsString>) {
    match previous_value {
        Some(value) => {
            // SAFETY: callers only restore the variable while still holding the same global
            // workspace-update lock that serialized the corresponding mutation.
            unsafe {
                std::env::set_var(GIT_LFS_PROGRESS_ENV, value);
            }
        }
        None => {
            // SAFETY: callers only clear the variable while still holding the same global
            // workspace-update lock that serialized the corresponding mutation.
            unsafe {
                std::env::remove_var(GIT_LFS_PROGRESS_ENV);
            }
        }
    }
}

fn spawn_progress_monitor(
    app_handle: AppHandle,
    window_label: String,
    event_name: String,
    progress_path: PathBuf,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut tracker = LfsProgressTracker::default();
        let mut offset = 0_u64;

        loop {
            let drained = drain_progress_updates(
                &app_handle,
                &window_label,
                &event_name,
                &progress_path,
                &mut offset,
                &mut tracker,
            );

            if stop.load(Ordering::Relaxed) && !drained {
                break;
            }

            thread::sleep(Duration::from_millis(150));
        }
    })
}

fn drain_progress_updates(
    app_handle: &AppHandle,
    window_label: &str,
    event_name: &str,
    progress_path: &PathBuf,
    offset: &mut u64,
    tracker: &mut LfsProgressTracker,
) -> bool {
    let Ok(mut file) = OpenOptions::new().read(true).open(progress_path) else {
        return false;
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return false;
    }

    let mut reader = BufReader::new(file);
    let mut drained_any = false;
    loop {
        let mut line = String::new();
        let Ok(bytes_read) = reader.read_line(&mut line) else {
            break;
        };
        if bytes_read == 0 {
            break;
        }
        *offset += bytes_read as u64;
        drained_any = true;

        if let Some(progress) = tracker.observe(line.trim_end(), Instant::now()) {
            let _ = app_handle.emit_to(EventTarget::window(window_label), event_name, progress);
        }
    }

    drained_any
}

fn parse_lfs_progress_line(line: &str) -> Option<LfsProgressLine> {
    let mut parts = line.splitn(4, ' ');
    let direction = parts.next()?.trim().to_owned();
    let file_counts = parts.next()?.trim();
    let byte_counts = parts.next()?.trim();
    let path = parts.next()?.trim().to_owned();

    let (current_file, total_files) = parse_fraction(file_counts)?;
    let (downloaded_bytes, total_bytes) = parse_fraction(byte_counts)?;

    Some(LfsProgressLine {
        direction,
        current_file,
        total_files,
        downloaded_bytes: downloaded_bytes.min(total_bytes),
        total_bytes,
        path,
    })
}

fn parse_fraction(value: &str) -> Option<(u64, u64)> {
    let (left, right) = value.split_once('/')?;
    Some((left.parse().ok()?, right.parse().ok()?))
}

impl LfsProgressTracker {
    fn observe(&mut self, line: &str, now: Instant) -> Option<WorkspaceUpdateProgress> {
        let parsed = parse_lfs_progress_line(line)?;

        if let Some(previous_file) = self.last_current_file
            && previous_file < parsed.current_file
            && let Some(previous) = self.files.get_mut(&previous_file)
        {
            previous.downloaded_bytes = previous.total_bytes;
        }

        let entry = self.files.entry(parsed.current_file).or_default();
        entry.total_bytes = parsed.total_bytes;
        entry.downloaded_bytes = entry.downloaded_bytes.max(parsed.downloaded_bytes);

        let total_downloaded: u64 = self
            .files
            .values()
            .map(|progress| progress.downloaded_bytes.min(progress.total_bytes))
            .sum();

        let bytes_per_second = match self.last_sample {
            Some((previous_at, previous_total_downloaded))
                if total_downloaded > previous_total_downloaded =>
            {
                let elapsed = now.saturating_duration_since(previous_at).as_secs_f64();
                if elapsed > 0.0 {
                    let speed = (total_downloaded - previous_total_downloaded) as f64 / elapsed;
                    self.last_sample = Some((now, total_downloaded));
                    self.last_speed = Some(speed);
                    Some(speed)
                } else {
                    self.last_speed
                }
            }
            Some(_) => self.last_speed,
            None => {
                self.last_sample = Some((now, total_downloaded));
                None
            }
        };

        self.last_current_file = Some(parsed.current_file);

        let current_file_fraction = if parsed.total_bytes == 0 {
            0.0
        } else {
            parsed.downloaded_bytes as f64 / parsed.total_bytes as f64
        };
        let progress_percent = if parsed.total_files == 0 {
            0.0
        } else {
            ((((parsed.current_file.saturating_sub(1)) as f64) + current_file_fraction)
                / parsed.total_files as f64)
                * 100.0
        }
        .clamp(0.0, 100.0);

        Some(WorkspaceUpdateProgress {
            direction: parsed.direction,
            current_file: parsed.current_file,
            total_files: parsed.total_files,
            file_downloaded_bytes: parsed.downloaded_bytes,
            file_total_bytes: parsed.total_bytes,
            progress_percent,
            bytes_per_second,
            path: parsed.path,
        })
    }
}

#[tauri::command(async)]
#[instrument(skip(window), err(Debug))]
#[allow(non_snake_case)]
pub async fn integrate_upstream(
    window: Window,
    projectId: ProjectHandleOrLegacyProjectId,
    resolutions: Vec<Resolution>,
    baseBranchResolution: Option<BaseBranchResolution>,
) -> Result<IntegrationOutcome, json::Error> {
    let _progress_lock = workspace_update_progress_lock().lock().await;
    let ctx = ThreadSafeContext::try_from(projectId.clone())?;
    let progress_scope = WorkspaceUpdateProgressScope::new(window, &projectId)?;
    let result = virtual_branches::integrate_upstream(ctx, resolutions, baseBranchResolution).await;
    progress_scope.finish();
    result.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{LfsProgressLine, LfsProgressTracker, parse_lfs_progress_line};

    #[test]
    fn parses_git_lfs_progress_lines_with_paths_that_contain_spaces() {
        let parsed =
            parse_lfs_progress_line("download 3/12 2097152/4194304 Assets/Bundles/My World.bundle")
                .expect("line should parse");

        assert_eq!(
            parsed,
            LfsProgressLine {
                direction: "download".into(),
                current_file: 3,
                total_files: 12,
                downloaded_bytes: 2 * 1024 * 1024,
                total_bytes: 4 * 1024 * 1024,
                path: "Assets/Bundles/My World.bundle".into(),
            },
            "the parser must preserve the whole path because Unity assets often contain spaces"
        );
    }

    #[test]
    fn tracks_percent_and_speed_across_multiple_progress_updates() {
        let start = Instant::now();
        let mut tracker = LfsProgressTracker::default();

        let first = tracker
            .observe("download 1/4 256/1024 Assets/Bundles/a.bundle", start)
            .expect("first sample should emit progress");
        assert_eq!(
            first.progress_percent, 6.25,
            "progress should include the partial completion of the current file"
        );
        assert_eq!(
            first.bytes_per_second, None,
            "the first sample cannot produce a speed because there is no previous sample"
        );

        let second = tracker
            .observe(
                "download 1/4 768/1024 Assets/Bundles/a.bundle",
                start + Duration::from_secs(2),
            )
            .expect("second sample should emit progress");
        assert_eq!(
            second.progress_percent, 18.75,
            "progress should increase as more of the current file downloads"
        );
        assert_eq!(
            second.bytes_per_second,
            Some(256.0),
            "speed should be derived from the total downloaded bytes delta over time"
        );

        let third = tracker
            .observe(
                "download 2/4 512/2048 Assets/Bundles/b.bundle",
                start + Duration::from_secs(4),
            )
            .expect("moving to the next file should still emit progress");
        assert_eq!(
            third.progress_percent, 31.25,
            "progress should carry completed files forward when the next file starts"
        );
        assert_eq!(
            third.bytes_per_second,
            Some(384.0),
            "speed should account for the previous file having completed before the next one began"
        );
    }
}
