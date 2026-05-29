//! Run `but-{name}` executables discovered on [`std::env::var_os`]("PATH"), akin to Git's `git-send-email`-style helpers.

use std::{ffi::OsString, path::Path};

#[cfg(unix)]
use std::{
    ffi::OsStr,
    io::ErrorKind,
    os::unix::ffi::OsStrExt,
    process::{Command, Stdio},
};

#[cfg(unix)]
use anyhow::Context;

#[cfg(unix)]
use crate::bad_input;

use crate::{CliError, CliResult};

pub(crate) fn dispatch(current_dir: &Path, extra: &[OsString]) -> CliResult<()> {
    let subcommand_name = match extra {
        [head, ..] => head,
        _ => {
            return Err(
                anyhow::anyhow!("BUG: external subcommand parsed without any arguments").into(),
            );
        }
    };

    #[cfg(windows)]
    {
        // External commands not yet supported on Windows
        return Err(CliError::ExternalCommandNotFound(
            subcommand_name.to_owned(),
        ));
    }

    #[cfg(unix)]
    {
        if !is_allowed_command_name(subcommand_name) {
            return Err(bad_input("Subcommand contains illegal characters")
                .arg_value(subcommand_name.to_string_lossy())
                .hint("Are you trying to write an extension command 'but-<command>'? Make sure that '<command>' only contains characters in the set [a-zA-Z_-]")
                .into());
        }

        let mut prefixed = OsString::from("but-");
        prefixed.push(subcommand_name);

        let status = match Command::new(&prefixed)
            .args(&extra[1..])
            .current_dir(current_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
        {
            Ok(status) => status,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Err(CliError::ExternalCommandNotFound(
                    subcommand_name.to_owned(),
                ));
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("could not invoke `{prefixed:?}` from PATH"))
                    .map_err(CliError::from);
            }
        };

        if status.success() {
            Ok(())
        } else {
            std::process::exit(status.code().unwrap_or(1));
        }
    }
}

/// A command name must consist of characters [a-zA-Z_-].
///
/// This is overly restrictive, but I just want to prevent people from doing anything funny here. We
/// can loosen this as we need to in the future.
#[cfg(unix)]
fn is_allowed_command_name(command_name: &OsStr) -> bool {
    command_name
        .as_bytes()
        .iter()
        .copied()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_lowercase() || c == b'-' || c == b'_')
}
