use gitbutler_filemonitor::InternalEvent;
use gitbutler_project::ProjectId;

#[tokio::test]
async fn test_spawn_emits_watch_path_for_new_dir() {
    let (repo, _tmpdir) = but_testsupport::writable_scenario("watch-plan-ignores-node-modules");
    let worktree = repo.workdir().expect("non-bare");
    let project_id = ProjectId::generate();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    
    // Set watch mode to Plan to enable dynamic watches
    unsafe { std::env::set_var("GITBUTLER_WATCH_MODE", "plan"); }
    
    let _debouncer = gitbutler_filemonitor::spawn(project_id, worktree, tx).expect("spawn successful");

    // Create a new directory
    let new_dir = worktree.join("new_dir");
    std::fs::create_dir(&new_dir).expect("mkdir successful");
    
    // We need to wait for the debouncer to pick up the change and for the file_monitor to process it.
    // The debouncer has a TICK_RATE of 100-250ms.
    
    let mut watch_path_received = false;
    let timeout = std::time::Duration::from_secs(5);
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout {
        if let Ok(event) = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            match event {
                Some(InternalEvent::WatchPath(pid, path)) => {
                    if pid == project_id && path == new_dir {
                        watch_path_received = true;
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    
    assert!(watch_path_received, "Should have received WatchPath event for new_dir");
}
