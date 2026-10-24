//! Registered Gitoxide locks are removed when the CLI is signalled.
//!
//! Unix tests send SIGTERM, SIGINT, and SIGQUIT to a fresh child. Each child
//! calls the same startup function as `main`, then holds one real git-style
//! lock. Windows still compiles that function: Gitoxide registers its own
//! termination signals there and emulates the default terminate action, so
//! these tests do not assert POSIX signal numbers outside Unix.

use std::{
    fs,
    path::{Path, PathBuf},
};

const RESOURCE_NAME: &str = "resource";
const SENTINEL_NAME: &str = "sentinel.lock";
const METADATA_NAME: &str = "project-metadata";
const SENTINEL_CONTENTS: &[u8] = b"unregistered sentinel\n";
const METADATA_CONTENTS: &[u8] = b"unrelated project metadata\n";
const RESOURCE_CONTENTS: &[u8] = b"resource body\n";

struct Fixture {
    resource: PathBuf,
    registered_lock: PathBuf,
    sentinel: PathBuf,
    metadata: PathBuf,
}

fn lock_path_for(resource: &Path) -> PathBuf {
    let mut lock_path = resource.as_os_str().to_owned();
    lock_path.push(".lock");
    PathBuf::from(lock_path)
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    fs::write(path, contents).map_err(|err| format!("write {}: {err}", path.display()))
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))
}

fn prepare_fixture(dir: &Path) -> Result<Fixture, String> {
    let resource = dir.join(RESOURCE_NAME);
    write_file(&resource, RESOURCE_CONTENTS)?;
    let sentinel = dir.join(SENTINEL_NAME);
    write_file(&sentinel, SENTINEL_CONTENTS)?;
    let metadata = dir.join(METADATA_NAME);
    write_file(&metadata, METADATA_CONTENTS)?;
    Ok(Fixture {
        registered_lock: lock_path_for(&resource),
        resource,
        sentinel,
        metadata,
    })
}

fn acquire_registered_lock(resource: &Path) -> Result<gix::lock::File, String> {
    gix::lock::File::acquire_to_update_resource(
        resource,
        gix::lock::acquire::Fail::Immediately,
        None,
    )
    .map_err(|err| format!("acquire {}: {err}", resource.display()))
}

/// Sentinel, metadata, resource, and the directory itself are not registry entries.
fn assert_neighbors_intact(fixture: &Fixture) -> Result<(), String> {
    let Some(dir) = fixture.resource.parent() else {
        return Err("resource has no parent directory".to_owned());
    };
    if !dir.is_dir() {
        return Err(format!(
            "containing directory {} was removed",
            dir.display()
        ));
    }
    if read_file(&fixture.sentinel)? != SENTINEL_CONTENTS {
        return Err("unregistered sentinel lock contents changed".to_owned());
    }
    if read_file(&fixture.metadata)? != METADATA_CONTENTS {
        return Err("unrelated metadata contents changed".to_owned());
    }
    if read_file(&fixture.resource)? != RESOURCE_CONTENTS {
        return Err("locked resource contents changed".to_owned());
    }
    Ok(())
}

#[test]
fn dropping_the_guard_removes_the_registered_lock() {
    // Startup must not change ordinary guard-drop cleanup.
    super::install_registered_tempfile_cleanup();
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = prepare_fixture(dir.path()).expect("fixture files");
    let lock = acquire_registered_lock(&fixture.resource).expect("registered lock");
    assert_eq!(
        lock.lock_path(),
        fixture.registered_lock,
        "the held file is the git-style lock next to the resource"
    );
    assert!(
        fixture.registered_lock.is_file(),
        "the registered lock exists while the guard is held"
    );

    drop(lock);

    assert!(
        !fixture.registered_lock.exists(),
        "dropping the guard removes the registered lock"
    );
    assert_neighbors_intact(&fixture).unwrap_or_else(|err| panic!("{err}"));
}

#[cfg(unix)]
mod termination {
    use std::{
        io::Read,
        os::unix::process::ExitStatusExt,
        process::{Child, Command, ExitStatus, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use super::{
        Fixture, acquire_registered_lock, assert_neighbors_intact, prepare_fixture, read_file,
        write_file,
    };

    const FIXTURE_DIR: &str = "BUT_SIGNAL_LOCK_FIXTURE_DIR";
    const CHILD_TEST: &str = "signal_tests::termination::registered_lock_child";
    const READY_NAME: &str = "ready";
    const REGISTERED_LOCK_CONTENTS: &[u8] = b"registered lock\n";
    /// POSIX values reported by `ExitStatus::signal` after `kill -s NAME`.
    const SIGINT: i32 = 2;
    const SIGQUIT: i32 = 3;
    const SIGTERM: i32 = 15;
    const READY_TIMEOUT: Duration = Duration::from_secs(60);
    const SIGNAL_TIMEOUT: Duration = Duration::from_secs(10);
    const CHILD_SIGNAL_WAIT: Duration = Duration::from_secs(120);
    const POLL: Duration = Duration::from_millis(20);

    struct ChildSession {
        child: Child,
        stdout: Option<thread::JoinHandle<Vec<u8>>>,
        stderr: Option<thread::JoinHandle<Vec<u8>>>,
        reaped: bool,
    }

    impl ChildSession {
        fn id(&self) -> u32 {
            self.child.id()
        }

        fn try_wait(&mut self) -> Result<Option<ExitStatus>, String> {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.reaped = true;
                    Ok(Some(status))
                }
                Ok(None) => Ok(None),
                Err(err) => Err(format!("wait for child: {err}")),
            }
        }

        fn wait_until(
            &mut self,
            timeout: Duration,
            mut ready: impl FnMut() -> bool,
        ) -> Result<Option<ExitStatus>, String> {
            let deadline = Instant::now() + timeout;
            loop {
                if let Some(status) = self.try_wait()? {
                    return Ok(Some(status));
                }
                if ready() {
                    return Ok(None);
                }
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                thread::sleep(POLL);
            }
        }

        fn diagnostics(mut self) -> String {
            if !self.reaped {
                let _ = self.child.kill();
                let _ = self.child.wait();
                self.reaped = true;
            }
            let stdout = join_output(self.stdout.take());
            let stderr = join_output(self.stderr.take());
            format!("--- child stdout ---\n{stdout}\n--- child stderr ---\n{stderr}")
        }
    }

    impl Drop for ChildSession {
        fn drop(&mut self) {
            if !self.reaped {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }

    fn spawn_reader<R>(mut pipe: R) -> thread::JoinHandle<Vec<u8>>
    where
        R: Read + Send + 'static,
    {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    }

    fn join_output(handle: Option<thread::JoinHandle<Vec<u8>>>) -> String {
        let Some(handle) = handle else {
            return String::new();
        };
        let bytes = handle.join().unwrap_or_default();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn spawn_child(dir: &std::path::Path) -> Result<ChildSession, String> {
        let exe = std::env::current_exe().map_err(|err| format!("current_exe: {err}"))?;
        // `ulimit` is a shell builtin. `exec` keeps this pid as the test process
        // and stops SIGQUIT's default action from writing a core file.
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg("ulimit -c 0; exec \"$0\" \"$@\"")
            .arg(exe)
            .arg(CHILD_TEST)
            .arg("--exact")
            .arg("--ignored")
            .arg("--test-threads=1")
            .env(FIXTURE_DIR, dir)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("spawn lock child: {err}"))?;
        let stdout = child.stdout.take().map(spawn_reader);
        let stderr = child.stderr.take().map(spawn_reader);
        Ok(ChildSession {
            child,
            stdout,
            stderr,
            reaped: false,
        })
    }

    fn signal_child(pid: u32, signal_name: &str) -> Result<(), String> {
        if pid == 0 || pid == std::process::id() {
            return Err(format!("refusing to signal pid {pid}"));
        }
        let output = Command::new("kill")
            .args(["-s", signal_name, &pid.to_string()])
            .output()
            .map_err(|err| format!("run kill: {err}"))?;
        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "kill -s {signal_name} {pid} failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }

    fn assert_signal_status(
        status: ExitStatus,
        signal_name: &str,
        signal_number: i32,
    ) -> Result<(), String> {
        if status.success() {
            return Err("child exited successfully instead of dying from the signal".to_owned());
        }
        if status.code().is_some() {
            return Err(format!(
                "child exited with a status code ({status}) instead of signal {signal_name}"
            ));
        }
        if status.signal() != Some(signal_number) {
            return Err(format!(
                "child died from signal {:?}, expected {signal_name} ({signal_number})",
                status.signal()
            ));
        }
        Ok(())
    }

    fn run_case(signal_name: &str, signal_number: i32) -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|err| format!("tempdir: {err}"))?;
        let mut session = spawn_child(dir.path())?;
        let result = exercise_signal(&mut session, dir.path(), signal_name, signal_number);
        if let Err(err) = result {
            return Err(format!("{err}\n{}", session.diagnostics()));
        }
        Ok(())
    }

    fn exercise_signal(
        session: &mut ChildSession,
        dir: &std::path::Path,
        signal_name: &str,
        signal_number: i32,
    ) -> Result<(), String> {
        let fixture = Fixture {
            resource: dir.join(super::RESOURCE_NAME),
            registered_lock: super::lock_path_for(&dir.join(super::RESOURCE_NAME)),
            sentinel: dir.join(super::SENTINEL_NAME),
            metadata: dir.join(super::METADATA_NAME),
        };
        let ready = dir.join(READY_NAME);
        let early = session.wait_until(READY_TIMEOUT, || ready.is_file())?;
        if let Some(status) = early {
            return Err(format!(
                "child exited before the registered lock was ready: {status}"
            ));
        }
        if !ready.is_file() {
            return Err("timed out waiting for the child to acquire the lock".to_owned());
        }

        let pid = session.id();
        if !fixture.registered_lock.is_file() {
            return Err(format!(
                "registered lock {} was missing after the handshake",
                fixture.registered_lock.display()
            ));
        }
        if read_file(&fixture.registered_lock)? != REGISTERED_LOCK_CONTENTS {
            return Err("registered lock did not contain the bytes the child wrote".to_owned());
        }
        assert_neighbors_intact(&fixture)?;

        signal_child(pid, signal_name)?;
        let status = session
            .wait_until(SIGNAL_TIMEOUT, || false)?
            .ok_or_else(|| format!("{signal_name} left the child running"))?;
        assert_signal_status(status, signal_name, signal_number)?;

        if fixture.registered_lock.exists() {
            return Err(format!(
                "registered lock {} survived {signal_name}",
                fixture.registered_lock.display()
            ));
        }
        assert_neighbors_intact(&fixture)?;
        if !ready.is_file() {
            return Err("readiness file was removed with the registered lock".to_owned());
        }
        Ok(())
    }

    fn hold_registered_lock(dir: &std::path::Path) -> Result<gix::lock::File, String> {
        // Same call `main` makes. `Mode::default()` is the wrong mode in this
        // binary: under `cfg(test)` Gitoxide's default does not terminate.
        super::super::install_registered_tempfile_cleanup();
        let fixture = prepare_fixture(dir)?;
        let mut lock = acquire_registered_lock(&fixture.resource)?;
        if lock.lock_path() != fixture.registered_lock {
            return Err(format!(
                "lock path {} did not match {}",
                lock.lock_path().display(),
                fixture.registered_lock.display()
            ));
        }
        std::io::Write::write_all(&mut lock, REGISTERED_LOCK_CONTENTS)
            .map_err(|err| format!("write lock: {err}"))?;
        std::io::Write::flush(&mut lock).map_err(|err| format!("flush lock: {err}"))?;
        write_file(&dir.join(READY_NAME), b"ready")?;
        Ok(lock)
    }

    fn wait_for_termination(lock: gix::lock::File) -> ! {
        let deadline = Instant::now() + CHILD_SIGNAL_WAIT;
        while Instant::now() < deadline {
            thread::park_timeout(Duration::from_millis(200));
        }
        drop(lock);
        eprintln!("registered lock child timed out waiting for a termination signal");
        std::process::exit(3);
    }

    #[test]
    #[ignore = "subprocess fixture spawned by the signal cleanup tests"]
    fn registered_lock_child() {
        let Ok(dir) = std::env::var(FIXTURE_DIR) else {
            return;
        };
        match hold_registered_lock(std::path::Path::new(&dir)) {
            Ok(lock) => wait_for_termination(lock),
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(2);
            }
        }
    }

    #[test]
    fn sigterm_removes_registered_lock() {
        if let Err(err) = run_case("TERM", SIGTERM) {
            panic!("{err}");
        }
    }

    #[test]
    fn sigint_removes_registered_lock() {
        if let Err(err) = run_case("INT", SIGINT) {
            panic!("{err}");
        }
    }

    #[test]
    fn sigquit_removes_registered_lock() {
        if let Err(err) = run_case("QUIT", SIGQUIT) {
            panic!("{err}");
        }
    }
}
