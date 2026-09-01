use std::{ffi::OsString, path::PathBuf, process::Stdio};

use bstr::BStr;
use tracing::instrument;

/// Describes how the process environment was initialized at application startup.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ApplicationEnvironment {
    /// The application was launched from a terminal, so its environment was preserved.
    Inherited,
    /// The interactive login-shell environment was imported.
    Imported,
    /// A login-shell environment could not be obtained, so the existing environment was preserved.
    Unavailable,
}

/// Prepare `program` for invocation with a Git-compatible shell to help it pick up more of the usual environment on Windows.
///
/// On Windows, this specifically uses the Git-bundled shell, further increasing compatibility.
pub fn prepare_with_shell_on_windows(program: impl Into<OsString>) -> gix::command::Prepare {
    if cfg!(windows) {
        gix::command::prepare(program)
            // On Windows, this means a shell will always be used.
            .command_may_be_shell_script_disallow_manual_argument_splitting()
            // force using a shell, we want access to additional programs here
            .with_shell()
            // We know `program` is a path, so quote it.
            .with_quoted_command()
    } else {
        gix::command::prepare(program)
    }
}

/// Launch the login shell and try to extract their environment variables, or `None` if the shell couldn't be determined,
/// or if it couldn't be launched, or if the environment extraction failed.
#[instrument()]
pub fn extract_interactive_login_shell_environment() -> Option<Vec<(OsString, OsString)>> {
    // NOTE that `SHELL` isn't usually set on Windows, so this will not usually run there.
    let shell_path: PathBuf = std::env::var_os("SHELL")?.into();
    let output = std::process::Command::from(
        // This automatically prevents a Window from popping up on Windows.
        gix::command::prepare(shell_path)
            .args(["-i", "-l", "-c", "env"])
            .stderr(Stdio::null()),
    )
    .output()
    .ok()?;
    if !output.status.success() {
        return None;
    }

    let vars = parse_key_value_pairs(output.stdout.as_slice());
    (!vars.is_empty()).then_some(vars)
}

/// Initialize the process environment for an application before it launches child processes.
///
/// Applications launched from a terminal retain their inherited environment. Other applications
/// import the environment produced by the user's interactive login shell when it is available.
///
/// This mutates the process environment, so applications must invoke it during startup, before
/// starting work that reads environment variables or launches child processes.
pub fn initialize_application_environment() -> ApplicationEnvironment {
    initialize_application_environment_with(
        std::env::var_os("TERM"),
        extract_interactive_login_shell_environment,
        |key, value| {
            // SAFETY: This startup-only API is called before application work can concurrently
            // access the process environment.
            unsafe { std::env::set_var(key, value) }
        },
    )
}

fn initialize_application_environment_with(
    terminal: Option<OsString>,
    extract: impl FnOnce() -> Option<Vec<(OsString, OsString)>>,
    mut apply: impl FnMut(OsString, OsString),
) -> ApplicationEnvironment {
    if terminal.is_some() {
        return ApplicationEnvironment::Inherited;
    }

    let Some(variables) = extract() else {
        return ApplicationEnvironment::Unavailable;
    };
    for (key, value) in variables {
        apply(key, value);
    }
    ApplicationEnvironment::Imported
}

/// Parse `a=b\n` input and convert these into OsStrings for later consumption
fn parse_key_value_pairs<'a>(input: impl Into<&'a BStr>) -> Vec<(OsString, OsString)> {
    use bstr::ByteSlice;
    let mut out = Vec::new();
    for line in input.into().lines() {
        let mut tokens = line.splitn(2, |b| b == &b'=');
        let (key, value) = (tokens.next(), tokens.next());
        match (key, value) {
            (Some(key), Some(value)) => {
                out.push((
                    gix::path::from_byte_slice(key).into(),
                    gix::path::from_byte_slice(value).into(),
                ));
            }
            _ => continue,
        }
    }
    out
}

#[cfg(test)]
mod extract_login_shell_command {
    use std::{cell::RefCell, ffi::OsString};

    use super::{
        ApplicationEnvironment, initialize_application_environment_with, parse_key_value_pairs,
    };

    #[test]
    fn parse_key_value_pairs_various_inputs() {
        let one_line_missing_newline = "a=b";
        assert_eq!(
            parse_key_value_pairs(one_line_missing_newline),
            osvec(Some(("a", "b")))
        );

        let value_with_equal_sign = "a=b=c";
        assert_eq!(
            parse_key_value_pairs(value_with_equal_sign),
            osvec(Some(("a", "b=c")))
        );

        let multi_line = "a=b\nkey=value\n";
        assert_eq!(
            parse_key_value_pairs(multi_line),
            osvec([("a", "b"), ("key", "value")])
        );

        let multi_line_missing_trailing_newline = "a=b\nkey=value";
        assert_eq!(
            parse_key_value_pairs(multi_line_missing_trailing_newline),
            osvec([("a", "b"), ("key", "value")])
        );
    }

    #[test]
    fn terminal_environment_is_preserved() {
        let extracted = RefCell::new(false);
        let applied = RefCell::new(Vec::new());

        let outcome = initialize_application_environment_with(
            Some("xterm".into()),
            || {
                *extracted.borrow_mut() = true;
                Some(osvec([("PATH", "/from/shell")]))
            },
            |key, value| applied.borrow_mut().push((key, value)),
        );

        assert_eq!(outcome, ApplicationEnvironment::Inherited);
        assert!(
            !extracted.into_inner(),
            "the login shell must not be launched"
        );
        assert!(
            applied.into_inner().is_empty(),
            "the inherited environment must not be changed"
        );
    }

    #[test]
    fn login_shell_environment_is_applied_as_a_complete_set() {
        let applied = RefCell::new(Vec::new());

        let outcome = initialize_application_environment_with(
            None,
            || Some(osvec([("PATH", "/from/shell"), ("TOKEN", "a=b")])),
            |key, value| applied.borrow_mut().push((key, value)),
        );

        assert_eq!(outcome, ApplicationEnvironment::Imported);
        assert_eq!(
            applied.into_inner(),
            osvec([("PATH", "/from/shell"), ("TOKEN", "a=b")])
        );
    }

    #[test]
    fn unavailable_login_shell_leaves_environment_unchanged() {
        let applied = RefCell::new(Vec::new());

        let outcome = initialize_application_environment_with(
            None,
            || None,
            |key, value| applied.borrow_mut().push((key, value)),
        );

        assert_eq!(outcome, ApplicationEnvironment::Unavailable);
        assert!(
            applied.into_inner().is_empty(),
            "failed extraction must not partially change the environment"
        );
    }

    fn osvec(
        pairs: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Vec<(OsString, OsString)> {
        pairs
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect()
    }
}
