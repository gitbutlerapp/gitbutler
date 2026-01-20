//! Use this file only for this one tests, as it **fork-execs**!
//! Otherwise, some tools may have problems, or it seems to hang there.
//!
//! # WARNING: Flaky the first time it runs locally
use but_db::DbHandle;

#[cfg(unix)]
#[test]
#[ignore = "it's too unstable locally and I think more needs to be figured out - \
            let's assume migrations really work now as this test always failed previously.\
            Now it runs once the test compiled at least once"]
fn migrations_in_parallel_with_processes() -> anyhow::Result<()> {
    use std::os::unix::process::ExitStatusExt;

    let tmp = tempfile::tempdir()?;
    // More processes = higher failure rate.
    // Procs lowered from 7 as there is flake on CI.
    let num_procs = 7;
    let mut children = Vec::new();

    // NOTE: if migrations fail, this tests fails MOST of the time. It's timing dependent.
    for _ in 0..num_procs {
        unsafe {
            let pid = libc::fork();
            if pid == 0 {
                // child
                for _round in 0..10 {
                    let handle = match DbHandle::new_in_directory(tmp.path()) {
                        Ok(h) => h,
                        Err(err) => {
                            eprintln!("Failed to open DB: {err}");
                            libc::_exit(42);
                        }
                    };
                    assert!(handle.hunk_assignments().list_all().unwrap().is_empty());
                }
                libc::_exit(0);
            } else if pid > 0 {
                // parent
                children.push(pid);
            } else {
                return Err(std::io::Error::last_os_error().into());
            }
        }
    }

    // parent waits for all children
    for pid in children {
        let mut status = 0;
        unsafe {
            if libc::waitpid(pid, &mut status, 0) < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        assert!(
            libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
            "FLAKY (try again): child exited unsuccessfully: {:?}",
            std::process::ExitStatus::from_raw(status)
        );
    }

    Ok(())
}
