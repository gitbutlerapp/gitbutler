//! Build and restart, for a client `cargo run` started from a checkout of this repository on
//! macOS or Linux: build it again, from this checkout or another worktree of it, and run the
//! new binary in this process's place, so `cargo run` and its terminal keep it.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use parking_lot::Mutex;

/// The binary `cargo build` makes, as `Cargo.toml` names it.
const BIN: &str = "gitbutler-gpui";

/// The askpass helper that fetch and push run, which they look for beside [`BIN`].
const ASKPASS_BIN: &str = "gitbutler-git-askpass";

/// How many columns cargo's progress bar takes, its crates' names included: as wide as the
/// modal showing it.
const PROGRESS_WIDTH: u16 = 100;

/// How this binary was built, to build it again.
#[derive(Clone)]
pub struct Rebuild {
    /// The `cargo` that ran it, of the toolchain that built it.
    cargo: PathBuf,
    /// The checkout it was built from: the worktree's root.
    checkout: PathBuf,
    /// The crate's folder within a checkout, `crates/but-gpui`.
    crate_dir: PathBuf,
    /// The build's profile, by the folder its binary is in: `debug` for `dev`.
    profile: Option<String>,
}

/// A worktree of the repository to build from.
pub struct Source {
    pub dir: PathBuf,
    /// The branch it has checked out, without `refs/heads/`, unless its `HEAD` is detached.
    pub branch: Option<String>,
}

impl Rebuild {
    /// How to build this binary again, when `cargo run` started it from the checkout it was
    /// built from; `None` otherwise, as for an installed app or one the desktop app started.
    /// Windows locks a running binary, so a build couldn't replace it, and has no exec.
    pub fn detect() -> Option<Self> {
        let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
        let cargo = PathBuf::from(std::env::var_os("CARGO")?);
        if !cfg!(unix) || manifest_dir != Path::new(env!("CARGO_MANIFEST_DIR")) {
            return None;
        }
        let checkout = manifest_dir.ancestors().nth(2)?.to_owned();
        let crate_dir = manifest_dir.strip_prefix(&checkout).ok()?.to_owned();
        let exe = std::env::current_exe().ok()?;
        let profile = match exe.parent()?.file_name()?.to_str()? {
            "debug" => None,
            profile => Some(profile.to_owned()),
        };
        Some(Self {
            cargo,
            checkout,
            crate_dir,
            profile,
        })
    }

    /// The checkout this binary was built from.
    pub fn checkout(&self) -> &Path {
        &self.checkout
    }

    /// The repository's worktrees that have the client to build, the main one first.
    pub fn sources(&self) -> Result<Vec<Source>> {
        let repo = gix::open(&self.checkout)?;
        let main = repo.main_repo()?;
        let linked =
            (repo.worktrees()?.into_iter()).filter_map(|worktree| worktree.into_repo().ok());
        Ok(std::iter::once(main)
            .chain(linked)
            .filter_map(|repo| {
                let dir = repo.workdir()?.to_owned();
                let branch = repo.head_name().ok().flatten();
                // Deleted without Git, or on a branch from before the client.
                dir.join(&self.crate_dir)
                    .join("Cargo.toml")
                    .is_file()
                    .then(|| Source {
                        dir,
                        branch: branch.map(|name| name.shorten().to_string()),
                    })
            })
            .collect())
    }

    /// What `cargo build` is given, after `cargo`: this binary with the profile it was built
    /// with, its features, and the shaders compiled at runtime rather than with Xcode's tools.
    /// The askpass helper too, in the same build so it lands beside the binary, as cargo
    /// doesn't build a dependency's binaries and each worktree has a target folder of its own.
    pub fn args(&self) -> Vec<String> {
        let mut features = vec!["gpui_platform/runtime_shaders"];
        if cfg!(feature = "dev-script") {
            features.push("dev-script");
        }
        let mut args = [
            "build",
            "-p",
            "but-gpui",
            "-p",
            "gitbutler-git",
            "--bin",
            BIN,
            "--bin",
            ASKPASS_BIN,
            "--features",
        ]
        .map(str::to_owned)
        .to_vec();
        args.push(features.join(","));
        match self.profile.as_deref() {
            None => {}
            Some("release") => args.push("--release".into()),
            Some(profile) => args.extend(["--profile".into(), profile.to_owned()]),
        }
        args
    }

    /// Start building the client in checkout `dir`, as `args` says, there and with its config.
    pub fn start(&self, dir: &Path) -> Result<Build> {
        let crate_dir = dir.join(&self.crate_dir);
        let mut cargo = Command::new(&self.cargo);
        cargo
            .args(self.args())
            .arg("--manifest-path")
            .arg(crate_dir.join("Cargo.toml"))
            // Its own output in JSON, for where it put the binary, and rustc's as it'd show,
            // coloured as in a terminal.
            .args([
                "--message-format",
                "json-render-diagnostics",
                "--color",
                "always",
            ])
            // Its progress bar too, which it draws only in a terminal unless told to.
            .args(["--config", "term.progress.when='always'", "--config"])
            .arg(format!("term.progress.width={PROGRESS_WIDTH}"))
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // A group of its own, with the compilers and linkers it starts, for Cancel to stop.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cargo, 0);
        let child = cargo.spawn().context("couldn't start cargo")?;
        Ok(Build {
            child: Arc::new(Mutex::new(child)),
            crate_dir,
        })
    }
}

/// A `cargo build` under way.
#[derive(Clone)]
pub struct Build {
    child: Arc<Mutex<Child>>,
    crate_dir: PathBuf,
}

/// What cargo printed: a line, or its progress bar, which it draws again in place.
pub enum Printed {
    Line(String),
    Progress(String),
}

/// A binary a build made, to restart on.
pub struct Built {
    exe: PathBuf,
    crate_dir: PathBuf,
}

impl Build {
    /// Wait for the build, giving `printed` what cargo prints as it prints it. Errs when it
    /// fails or is cancelled.
    pub fn finish(&self, printed: impl FnMut(Printed)) -> Result<Built> {
        let (stdout, stderr) = {
            let mut child = self.child.lock();
            (child.stdout.take(), child.stderr.take())
        };
        let (Some(stdout), Some(stderr)) = (stdout, stderr) else {
            bail!("the build's output was already read");
        };
        // Read beside the lines shown, or cargo would wait on a full pipe.
        let exe = std::thread::spawn(move || built_binary(stdout));
        read_printed(BufReader::new(stderr), printed)?;
        let exe = exe.join().ok().flatten();
        let status = self.child.lock().wait()?;
        if !status.success() {
            bail!("cargo build failed ({status})");
        }
        let exe = exe.context("cargo didn't say where it put the binary")?;
        Ok(Built {
            exe,
            crate_dir: self.crate_dir.clone(),
        })
    }

    /// Stop the build, and the compilers and linkers it started. Unless it's ending anyway,
    /// as when [`Build::finish`] waits for it.
    pub fn cancel(&self) {
        let Some(mut child) = self.child.try_lock() else {
            return;
        };
        // Once cargo has exited, what it started has too, and its group may be another's.
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        #[cfg(unix)]
        let stopped = match libc::pid_t::try_from(child.id()) {
            // SAFETY: `killpg` only sends a signal, and `start` made cargo the leader of
            // group `group`, which it still leads as it hasn't been waited for.
            Ok(group) => match unsafe { libc::killpg(group, libc::SIGKILL) } {
                0 => Ok(()),
                _ => Err(std::io::Error::last_os_error()),
            },
            Err(_) => child.kill(),
        };
        #[cfg(not(unix))]
        let stopped = child.kill();
        if let Err(err) = stopped {
            tracing::debug!("couldn't stop the build: {err:#}");
        }
    }
}

/// Pass what `stderr` has to `printed` as it comes: a line at each `\n`, and the progress bar
/// at each `\r`, which takes the cursor back to draw it again. Waiting for a `\n` would hold
/// the bar back until the next line, as while the client itself compiles.
fn read_printed(mut stderr: impl BufRead, mut printed: impl FnMut(Printed)) -> Result<()> {
    let mut piece = Vec::new();
    loop {
        let available = stderr.fill_buf()?;
        if available.is_empty() {
            break;
        }
        let end = available.iter().position(|b| matches!(b, b'\n' | b'\r'));
        let taken = end.map_or(available.len(), |at| at + 1);
        piece.extend(available.iter().take(taken));
        let ended = end.and_then(|at| available.get(at).copied());
        stderr.consume(taken);
        let Some(ended) = ended else {
            continue;
        };
        let text = String::from_utf8_lossy(&piece).trim_end().to_owned();
        printed(match ended {
            b'\n' => Printed::Line(text),
            _ => Printed::Progress(text),
        });
        piece.clear();
    }
    if !piece.is_empty() {
        printed(Printed::Line(
            String::from_utf8_lossy(&piece).trim_end().to_owned(),
        ));
    }
    Ok(())
}

/// The binary named in cargo's JSON messages on `stdout`.
fn built_binary(stdout: impl Read) -> Option<PathBuf> {
    #[derive(serde::Deserialize)]
    struct Message {
        reason: String,
        target: Option<Target>,
        executable: Option<PathBuf>,
    }
    #[derive(serde::Deserialize)]
    struct Target {
        name: String,
    }
    let mut exe = None;
    for line in BufReader::new(stdout).lines() {
        let Ok(message) = serde_json::from_str::<Message>(&line.ok()?) else {
            continue;
        };
        if message.reason == "compiler-artifact"
            && message.target.is_some_and(|target| target.name == BIN)
        {
            exe = message.executable.or(exe);
        }
    }
    exe
}

impl Built {
    /// Run the new binary in this process's place, with this one's arguments and environment,
    /// so `cargo run` and its terminal keep it. The manifest's folder is the one it was built
    /// from, as `cargo run` would have set it, so it offers Build and restart too. Returns only
    /// when that fails.
    pub fn restart(&self) -> anyhow::Error {
        let mut new = Command::new(&self.exe);
        new.args(std::env::args_os().skip(1))
            .env("CARGO_MANIFEST_DIR", &self.crate_dir)
            .env("CARGO_MANIFEST_PATH", self.crate_dir.join("Cargo.toml"));
        #[cfg(unix)]
        let err: anyhow::Error = std::os::unix::process::CommandExt::exec(&mut new).into();
        #[cfg(not(unix))]
        let err = anyhow::anyhow!("restarting in place needs exec");
        err.context(format!("couldn't start {}", self.exe.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Printed, built_binary, read_printed};

    /// Whether process `pid` is still running, as `kill -0` says.
    #[cfg(unix)]
    fn alive(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    #[test]
    #[cfg(unix)]
    fn cancel_stops_what_cargo_started_too() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::time::{Duration, Instant};

        use super::Rebuild;

        let dir = tempfile::tempdir().unwrap();
        // A cargo that starts a compiler, as rustc, and says which, then waits for it.
        let cargo = dir.path().join("cargo");
        std::fs::write(&cargo, "#!/bin/sh\nsleep 60 &\necho $! > compiler\nwait\n").unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let rebuild = Rebuild {
            cargo,
            checkout: dir.path().to_owned(),
            crate_dir: "crate".into(),
            profile: None,
        };
        let build = rebuild.start(dir.path()).unwrap();
        let started = Instant::now();
        let compiler = loop {
            match std::fs::read_to_string(dir.path().join("compiler")) {
                Ok(pid) if pid.ends_with('\n') => break pid.trim().to_owned(),
                _ => assert!(started.elapsed() < Duration::from_secs(10), "cargo started"),
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(alive(&compiler), "the compiler runs while the build does");

        build.cancel();
        build.child.lock().wait().unwrap();
        let cancelled = Instant::now();
        while alive(&compiler) && cancelled.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !alive(&compiler),
            "cancelling stops the compiler cargo started, not only cargo"
        );
    }

    #[test]
    fn the_build_makes_the_askpass_helper_too() {
        use super::Rebuild;

        let rebuild = Rebuild {
            cargo: "cargo".into(),
            checkout: "/checkout".into(),
            crate_dir: "crates/but-gpui".into(),
            profile: None,
        };
        assert_eq!(
            rebuild.args()[..9],
            [
                "build",
                "-p",
                "but-gpui",
                "-p",
                "gitbutler-git",
                "--bin",
                "gitbutler-gpui",
                "--bin",
                "gitbutler-git-askpass",
            ],
            "fetch and push run the helper from beside the client, so the build puts it there too"
        );
    }

    #[test]
    fn the_progress_bar_comes_at_each_carriage_return() {
        let stderr = "  Compiling a\n    Building [=> ] 1/2: b\r\x1b[K  Compiling b\nlast";
        let mut printed = Vec::new();
        read_printed(stderr.as_bytes(), |p| {
            printed.push(match p {
                Printed::Line(line) => format!("line {line}"),
                Printed::Progress(bar) => format!("bar {bar}"),
            })
        })
        .unwrap();
        assert_eq!(
            printed,
            [
                "line   Compiling a",
                "bar     Building [=> ] 1/2: b",
                "line \x1b[K  Compiling b",
                "line last",
            ],
            "a line ends at \\n, a drawing of the bar at \\r, and what's left at the end is a line"
        );
    }

    #[test]
    fn the_binary_is_where_cargo_says_it_put_the_client() {
        let messages = [
            r#"{"reason":"compiler-artifact","target":{"name":"but_gpui"},"executable":null}"#,
            "not json",
            r#"{"reason":"compiler-artifact","target":{"name":"gitbutler-gpui"},"executable":"/t/debug/gitbutler-gpui"}"#,
            r#"{"reason":"compiler-artifact","target":{"name":"other"},"executable":"/t/debug/other"}"#,
            r#"{"reason":"build-finished","success":true}"#,
        ]
        .join("\n");
        assert_eq!(
            built_binary(messages.as_bytes()),
            Some("/t/debug/gitbutler-gpui".into()),
            "only the client's artifact names the binary, and other lines are skipped"
        );
    }
}
