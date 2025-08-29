use bstr::BStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;

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
pub fn extract_interactive_login_shell_environment() -> Option<Vec<(OsString, OsString)>> {
    let shell_path: PathBuf = if cfg!(windows) {
        gix::path::env::shell().into()
    } else {
        std::env::var_os("SHELL")?.into()
    };
    let stdout = std::process::Command::new(shell_path)
        .args(["-i", "-l", "-c", "env"])
        .stderr(Stdio::null())
        .output()
        .ok()?
        .stdout;

    let vars = parse_key_value_pairs(stdout.as_slice());
    (!vars.is_empty()).then_some(vars)
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
    use super::parse_key_value_pairs;
    use std::ffi::OsString;

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

    fn osvec(
        pairs: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Vec<(OsString, OsString)> {
        pairs
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect()
    }
}
