//! The dev app's executable, which `make-app.sh` compiles into the bundle. It runs the client
//! through `gitbutler-gpui` beside it, the symlink to the build in `target`, in this process.
//! Run by the link's path, the client takes the bundle as its own, which it wouldn't if
//! macOS started the link itself: it starts the binary the link points at, outside the bundle.
//!
//! The client gets the environment `make-app.sh` saved in `Resources/environment` on top of
//! this one, for Build and restart's cargo. Not with `LSEnvironment`: macOS keeps the `PATH`
//! of whatever opens the app over that one's, and the Dock's has no toolchain.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("couldn't find the dev app: {err}");
            return ExitCode::FAILURE;
        }
    };
    let client = exe.with_file_name("gitbutler-gpui");
    let mut command = Command::new(&client);
    command.args(std::env::args_os().skip(1));
    if let Some(contents) = exe.parent().and_then(Path::parent) {
        let saved = contents.join("Resources/environment");
        match std::fs::read(&saved) {
            Ok(saved) => {
                command.envs(variables(&saved));
            }
            Err(err) => eprintln!("couldn't read {}: {err}", saved.display()),
        }
    }
    let err = command.exec();
    eprintln!("couldn't run {}: {err}", client.display());
    ExitCode::FAILURE
}

/// The variables in `saved`, as `env -0` prints them: `NAME=value`, each ended by a NUL.
fn variables(saved: &[u8]) -> impl Iterator<Item = (&OsStr, &OsStr)> {
    saved.split(|byte| *byte == 0).filter_map(|entry| {
        let at = entry.iter().position(|byte| *byte == b'=')?;
        let (name, value) = entry.split_at(at);
        Some((OsStr::from_bytes(name), OsStr::from_bytes(&value[1..])))
    })
}
