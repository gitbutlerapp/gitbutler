use std::ffi::OsString;

/// Prepare `program` for invocation with a shell on Windows, and directly everywhere else.
pub fn prepare_with_shell_on_windows(program: impl Into<OsString>) -> gix::command::Prepare {
    let prepare = gix::command::prepare(program);
    if cfg!(windows) {
        prepare
            .command_may_be_shell_script_disallow_manual_argument_splitting()
            // On Windows, this yields the Git-bundled `sh.exe`, which is what we want.
            .with_shell_program(gix::path::env::shell())
            // force using a shell, we want access to additional programs here
            .with_shell()
            .with_quoted_command()
    } else {
        prepare
    }
}
