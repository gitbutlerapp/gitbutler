use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

fn executable(dir: &std::path::Path) -> std::path::PathBuf {
    let source = dir.join("but 'quoted' \"double\" $dollar `ticks`\n spaces");
    fs::write(&source, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
    source
}

#[test]
fn installs_into_missing_directory_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let source = executable(dir.path());
    let destination = dir.path().join("bin/but");
    install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).unwrap();
    install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).unwrap();
    assert_eq!(
        fs::read_link(destination).unwrap(),
        source,
        "link preserves source path exactly"
    );
}

#[test]
fn refuses_existing_files_and_unrelated_links() {
    let dir = tempfile::tempdir().unwrap();
    let source = executable(dir.path());
    let destination = dir.path().join("but");
    fs::write(&destination, "keep me").unwrap();
    assert!(
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).is_err(),
        "existing files must survive"
    );
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        "keep me",
        "file was not overwritten"
    );
    fs::remove_file(&destination).unwrap();
    for target in [source.clone(), dir.path().join("missing")] {
        let unrelated = dir.path().join("unrelated");
        std::os::unix::fs::symlink(&target, &unrelated).unwrap();
        std::os::unix::fs::symlink(&unrelated, &destination).unwrap();
        assert!(
            install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).is_err(),
            "unrelated links must survive, even when dangling"
        );
        assert_eq!(
            fs::read_link(&destination).unwrap(),
            unrelated,
            "link was not replaced"
        );
        fs::remove_file(&destination).unwrap();
        fs::remove_file(&unrelated).unwrap();
    }
}

#[test]
fn replaces_live_and_broken_symlinks_only_when_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let source = executable(dir.path());
    let destination = dir.path().join("but");
    for target in [dir.path().to_path_buf(), dir.path().join("missing")] {
        std::os::unix::fs::symlink(&target, &destination).unwrap();
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Replace).unwrap();
        assert_eq!(
            fs::read_link(&destination).unwrap(),
            source,
            "replacement points to this app"
        );
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Replace).unwrap();
        fs::remove_file(&destination).unwrap();
    }
}

#[test]
fn replacement_preserves_files_directories_and_links_when_source_is_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let source = executable(dir.path());
    let destination = dir.path().join("but");
    fs::write(&destination, "keep me").unwrap();
    assert!(
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Replace).is_err(),
        "replacement only allows symlinks"
    );
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        "keep me",
        "file is preserved"
    );
    fs::remove_file(&destination).unwrap();
    fs::create_dir(&destination).unwrap();
    assert!(
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Replace).is_err(),
        "directories must survive"
    );
    assert!(destination.is_dir(), "directory is preserved");
    fs::remove_dir(&destination).unwrap();
    std::os::unix::fs::symlink(&source, &destination).unwrap();
    assert!(
        install_cli_link(
            &dir.path().join("missing"),
            &destination,
            ExistingSymlinkPolicy::Replace
        )
        .is_err(),
        "invalid source must not remove existing link"
    );
    assert_eq!(
        fs::read_link(&destination).unwrap(),
        source,
        "existing link is preserved"
    );
}

#[test]
fn distinguishes_cancellation_from_script_failure() {
    use std::os::unix::process::ExitStatusExt;
    let output = |code, stdout: &[u8], stderr: &[u8]| std::process::Output {
        status: std::process::ExitStatus::from_raw(code << 8),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    };
    check_cli_install_output(output(0, b"gitbutler-cli-installed\n", b"")).unwrap();
    let cancelled =
        check_cli_install_output(output(0, b"gitbutler-cli-install-cancelled\n", b"")).unwrap_err();
    assert!(
        matches!(
            cancelled.downcast_ref::<ErrorContext>().unwrap().code,
            Code::CliInstallCancelled
        ),
        "only explicit cancellation gets cancellation code"
    );
    let failed = check_cli_install_output(output(1, b"", b"permission denied")).unwrap_err();
    assert!(
        failed.downcast_ref::<ErrorContext>().is_none(),
        "exit code 1 is not cancellation"
    );
    assert!(
        failed.to_string().contains("permission denied"),
        "failure preserves diagnostics"
    );
    assert!(
        check_cli_install_output(output(0, b"unexpected\n", b"")).is_err(),
        "unexpected responses are not success"
    );
}

#[test]
fn validates_source_even_when_link_matches() {
    let dir = tempfile::tempdir().unwrap();
    let source = executable(dir.path());
    let destination = dir.path().join("but");
    install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).unwrap();
    fs::remove_file(&source).unwrap();
    assert!(
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).is_err(),
        "matching broken link is not success"
    );
    fs::create_dir(&source).unwrap();
    assert!(
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).is_err(),
        "directories are not executables"
    );
    fs::remove_dir(&source).unwrap();
    fs::write(&source, "not executable").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        install_cli_link(&source, &destination, ExistingSymlinkPolicy::Refuse).is_err(),
        "source needs executable permissions"
    );
    assert!(
        install_cli_link(
            std::path::Path::new("relative"),
            &destination,
            ExistingSymlinkPolicy::Refuse
        )
        .is_err(),
        "source must be absolute"
    );
}
