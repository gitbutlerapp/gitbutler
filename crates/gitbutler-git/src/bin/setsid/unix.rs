use nix::{
    libc::{c_int, wait, EXIT_FAILURE, WEXITSTATUS, WIFEXITED, WIFSIGNALED, WTERMSIG},
    unistd::{fork, setsid, ForkResult},
};
use std::{os::unix::process::CommandExt, process};

pub fn main() {
    let has_pipe_var = std::env::var("GITBUTLER_ASKPASS_PIPE")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if !has_pipe_var {
        panic!("This binary is only meant to be run by GitButler; please do not use it yourself as it's entirely unstable.");
    }

    let args = std::env::args().skip(1).collect::<Vec<_>>();

    match unsafe { fork() }.unwrap() {
        ForkResult::Parent { child, .. } => {
            let mut status: c_int = 0;

            let waited_pid = unsafe { wait(&mut status as *mut _) };
            if waited_pid != child.as_raw() {
                panic!(
                    "wait(): unexpected child process; got {}, expected {}",
                    waited_pid, child
                );
            }

            if WIFEXITED(status) {
                let exit_status = WEXITSTATUS(status);
                process::exit(exit_status);
            } else if WIFSIGNALED(status) {
                let signal = WTERMSIG(status);
                process::exit(128 + signal);
            } else {
                process::exit(EXIT_FAILURE);
            }
        }
        ForkResult::Child => {
            setsid().expect("setsid():");

            let err = process::Command::new(&args[0]).args(&args[1..]).exec();

            panic!("exec(): {}", err);
        }
    }
}
