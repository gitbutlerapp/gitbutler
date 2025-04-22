use std::ffi::OsString;

/// Prepare `program` for invocation with a Git-compatible shell to help it pick up more of the usual environment.
///
/// On Windows, this specifically uses the Git-bundled shell, further increasing compatibility.
pub fn prepare_with_shell(program: impl Into<OsString>) -> gix::command::Prepare {
    let login_shell = std::env::var_os("SHELL");
    // if login shell is set and it is a POSIX shell, use it, otherwise use the default shell
    let shell_program = login_shell
        .filter(|s| {
            let s = s.to_string_lossy();
            s.ends_with("/sh") || s.ends_with("/bash") || s.ends_with("/zsh")
        })
        .unwrap_or_else(|| gix::path::env::shell().into());

    gix::command::prepare(program)
        // On Windows, this means a shell will always be used.
        .command_may_be_shell_script_disallow_manual_argument_splitting()
        // On Windows, this yields the Git-bundled `sh.exe`, which is what we want.
        // Only use a login shell here if it is a POSIX one.
        .with_shell_program(shell_program)
        // force using a shell, we want access to additional programs here
        .with_shell()
        // We know `program` is a path, so quote it.
        .with_quoted_command()
}
