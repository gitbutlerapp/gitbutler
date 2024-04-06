use gitbutler_core::lock::Dir;

use gitbutler_testsupport::temp_dir;

#[tokio::test]
async fn lock_same_instance() {
    let dir_path = temp_dir();
    std::fs::write(dir_path.path().join("file.txt"), "").unwrap();
    let dir = Dir::new(dir_path.path()).unwrap();

    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    // spawn a task that will signal right after aquireing the lock
    let _ = tokio::spawn({
        let dir = dir.clone();
        async move {
            dir.batch(|root| {
                tx.send(()).unwrap();
                assert_eq!(
                    std::fs::read_to_string(root.join("file.txt")).unwrap(),
                    String::new()
                );
                std::fs::write(root.join("file.txt"), "1")
            })
        }
    })
    .await
    .unwrap();

    // then we wait until the lock is aquired
    rx.recv().unwrap();

    // and immidiately try to lock again
    dir.batch(|root| {
        assert_eq!(std::fs::read_to_string(root.join("file.txt")).unwrap(), "1");
        std::fs::write(root.join("file.txt"), "2")
    })
    .unwrap()
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(dir_path.path().join("file.txt")).unwrap(),
        "2"
    );
}

#[tokio::test]
async fn lock_different_instances() {
    let dir_path = temp_dir();
    std::fs::write(dir_path.path().join("file.txt"), "").unwrap();

    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    // spawn a task that will signal right after aquireing the lock
    let _ = tokio::spawn({
        let dir_path = dir_path.path().to_owned();
        async move {
            // one dir instance is created on a separate thread
            let dir = Dir::new(&dir_path).unwrap();
            dir.batch(|root| {
                tx.send(()).unwrap();
                assert_eq!(
                    std::fs::read_to_string(root.join("file.txt")).unwrap(),
                    String::new()
                );
                std::fs::write(root.join("file.txt"), "1")
            })
        }
    })
    .await
    .unwrap();

    // another dir instance is created on the main thread
    let dir = Dir::new(&dir_path).unwrap();

    // then we wait until the lock is aquired
    rx.recv().unwrap();

    // and immidiately try to lock again
    dir.batch(|root| {
        assert_eq!(std::fs::read_to_string(root.join("file.txt")).unwrap(), "1");
        std::fs::write(root.join("file.txt"), "2")
    })
    .unwrap()
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(dir_path.path().join("file.txt")).unwrap(),
        "2"
    );
}
