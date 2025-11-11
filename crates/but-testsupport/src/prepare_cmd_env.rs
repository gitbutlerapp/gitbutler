use std::{borrow::Cow, ffi::OsStr};

/// Change the `cmd` environment to be very isolated, particularly when Git is involved.
pub fn isolate_env_std_cmd(cmd: &mut std::process::Command) -> &mut std::process::Command {
    for op in updates() {
        match op {
            EnvOp::Remove(var) => {
                cmd.env_remove(var);
            }
            EnvOp::Add { name, value } => {
                cmd.env(name, value);
            }
        }
    }
    cmd
}

/// Change the `cmd` environment to be very isolated, particularly when Git is involved.
#[cfg(feature = "snapbox")]
pub fn isolate_snapbox_cmd(mut cmd: snapbox::cmd::Command) -> snapbox::cmd::Command {
    for op in updates() {
        cmd = match op {
            EnvOp::Remove(var) => cmd.env_remove(var),
            EnvOp::Add { name, value } => cmd.env(name, value),
        };
    }
    cmd
}

enum EnvOp {
    Remove(&'static str),
    Add {
        name: &'static str,
        value: Cow<'static, OsStr>,
    },
}

fn updates() -> Vec<EnvOp> {
    // Copied from gix-testtools/lib.rs, in an attempt to isolate everything as good as possible,
    #[cfg(windows)]
    const NULL_DEVICE: &str = "NUL";
    #[cfg(not(windows))]
    const NULL_DEVICE: &str = "/dev/null";

    // particularly mutation.
    let mut msys_for_git_bash_on_windows = std::env::var_os("MSYS").unwrap_or_default();
    msys_for_git_bash_on_windows.push(" winsymlinks:nativestrict");
    [
        EnvOp::Remove("GIT_DIR"),
        EnvOp::Remove("GIT_INDEX_FILE"),
        EnvOp::Remove("GIT_OBJECT_DIRECTORY"),
        EnvOp::Remove("GIT_ALTERNATE_OBJECT_DIRECTORIES"),
        EnvOp::Remove("GIT_WORK_TREE"),
        EnvOp::Remove("GIT_COMMON_DIR"),
        EnvOp::Remove("GIT_ASKPASS"),
        EnvOp::Remove("SSH_ASKPASS"),
    ]
    .into_iter()
    .chain(
        [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", NULL_DEVICE),
            ("GIT_TERMINAL_PROMPT", "false"),
            ("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000"),
            ("GIT_AUTHOR_EMAIL", "author@example.com"),
            ("GIT_AUTHOR_NAME", "author"),
            ("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000"),
            ("GIT_COMMITTER_EMAIL", "committer@example.com"),
            ("GIT_COMMITTER_NAME", "committer"),
            ("GIT_CONFIG_COUNT", "4"),
            ("GIT_CONFIG_KEY_0", "commit.gpgsign"),
            ("GIT_CONFIG_VALUE_0", "false"),
            ("GIT_CONFIG_KEY_1", "tag.gpgsign"),
            ("GIT_CONFIG_VALUE_1", "false"),
            ("GIT_CONFIG_KEY_2", "init.defaultBranch"),
            ("GIT_CONFIG_VALUE_2", "main"),
            ("GIT_CONFIG_KEY_3", "protocol.file.allow"),
            ("GIT_CONFIG_VALUE_3", "always"),
            ("CLICOLOR_FORCE", "1"),
            ("RUST_BACKTRACE", "0"),
        ]
        .into_iter()
        .map(|(name, value)| EnvOp::Add {
            name,
            value: Cow::Borrowed(OsStr::new(value)),
        }),
    )
    .chain(Some(EnvOp::Add {
        name: "MSYS",
        value: Cow::Owned(msys_for_git_bash_on_windows),
    }))
    .collect()
}
