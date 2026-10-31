use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{CliResult, bad_input};

/// Where to read a message from: a file path, or stdin when given as `-`.
#[derive(Debug, Clone)]
pub struct MessageFileArg(PathBuf);

impl From<OsString> for MessageFileArg {
    fn from(path: OsString) -> Self {
        Self(path.into())
    }
}

impl MessageFileArg {
    /// Read the whole message as UTF-8, unmodified.
    ///
    /// `-` reads stdin to EOF, after which nothing in the run can prompt. Other relative paths
    /// resolve against `current_dir`, the global `-C`.
    ///
    /// Errors with bad input for `--file` that names the path as typed and the OS error,
    /// including for content that is not UTF-8.
    pub fn read(&self, current_dir: &Path) -> CliResult<String> {
        let Self(path) = self;
        let content = if path.as_os_str() == "-" {
            std::io::read_to_string(std::io::stdin())
        } else {
            std::fs::read_to_string(current_dir.join(path))
        };
        content.map_err(|err| {
            bad_input(format!(
                "Could not read message from '{}': {err}",
                path.display()
            ))
            .arg_name("--file")
            .into()
        })
    }
}
