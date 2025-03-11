use std::ffi::OsString;

/// Prepare `program` for invocation with a Git-compatible shell to help it pick up more of the usual environment.
///
/// On Windows, this specifically uses the Git-bundled shell, further increasing compatibility.
pub fn prepare_with_shell(program: impl Into<OsString>) -> gix::command::Prepare {
    gix::command::prepare(program)
        // On Windows, this means a shell will always be used.
        .command_may_be_shell_script_disallow_manual_argument_splitting()
        // On Windows, this yields the Git-bundled `sh.exe`, which is what we want.
        .with_shell_program(
            std::env::var_os("SHELL").unwrap_or_else(|| gix::path::env::shell().into()),
        )
        // force using a shell, we want access to additional programs here
        .with_shell()
        // We know `program` is a path, so quote it.
        .with_quoted_command()
}
