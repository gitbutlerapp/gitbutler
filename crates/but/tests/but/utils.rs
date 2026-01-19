use std::{
    env,
    ops::{Deref, DerefMut},
};

use but_testsupport::isolate_snapbox_cmd;

/// A wrapper around [but_testsupport::Sandbox] to add `but` invocation support, which can only come from the crate that
/// defines the binary.
pub struct Sandbox {
    inner: but_testsupport::Sandbox,
}

impl Deref for Sandbox {
    type Target = but_testsupport::Sandbox;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Sandbox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Lifecycle
impl Sandbox {
    /// Create a new instance with empty everything.
    pub fn empty() -> anyhow::Result<Sandbox> {
        Ok(Sandbox {
            inner: but_testsupport::Sandbox::empty()?,
        })
    }

    /// A utility to init a scenario if the legacy feature is set, or open a repo otherwise.
    pub fn open_or_init_scenario_with_target_and_default_settings(
        name: &str,
    ) -> anyhow::Result<Sandbox> {
        let inner =
            but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(name)?;
        let this = Sandbox { inner };
        this.run_but_init_if_needed();
        Ok(this)
    }

    fn run_but_init_if_needed(&self) {
        // New code does everything lazily and can open any repository without extra step, so no need for `but setup`.
        if cfg!(feature = "legacy") {
            // Needs setup, as it's not single-branch compatible, must have legacy project added so legacy commands can find it.
            // This isn't needed at all when modernisation is complete.
            self.but("setup").assert().success();
        }
    }

    /// Open a repository without any additional setup and default application settings.
    pub fn open_with_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Ok(Sandbox {
            inner: but_testsupport::Sandbox::open_with_default_settings(name)?,
        })
    }

    /// Provide a scenario with `name` for writing, and `but setup` already invoked to add the project,
    /// with a target added.
    ///
    /// Prefer to use [`Self::open_scenario_with_target_and_default_settings()`] instead for less side-effects
    /// TODO: we shouldn't have to add the project for interaction - it's only useful for listing.
    /// TODO: there should be no need for the target.
    pub fn init_scenario_with_target_and_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        let inner = but_testsupport::Sandbox::init_scenario_with_target_and_default_settings(name)?;
        let this = Sandbox { inner };
        this.run_but_init_if_needed();
        Ok(this)
    }

    /// Provide a scenario with `name` for writing, with target added.
    pub fn open_scenario_with_target_and_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Ok(Sandbox {
            inner: but_testsupport::Sandbox::open_scenario_with_target_and_default_settings(name)?,
        })
    }

    /// Like [`Self::init_scenario_with_target_and_default_settings`], Execute the script at `name` instead of
    /// copying it - necessary if Git places absolute paths.
    pub fn init_scenario_with_target_and_default_settings_slow(
        name: &str,
    ) -> anyhow::Result<Sandbox> {
        let inner =
            but_testsupport::Sandbox::init_scenario_with_target_and_default_settings_slow(name)?;
        let this = Sandbox { inner };
        this.run_but_init_if_needed();
        Ok(this)
    }
}

/// Utilities
impl Sandbox {
    /// Print the paths to our directories, and keep them.
    pub fn debug(self) -> ! {
        self.inner.debug();
    }
}

/// Invocations
impl Sandbox {
    /// Create a command suitable for testing the output of the invocation with `args`. If `args` is empty,
    /// no arguments are provided.
    /// Note that more arguments can be added to the returned [snapbox::cmd::Command] later as well.
    pub fn but(&self, args: impl AsRef<str>) -> snapbox::cmd::Command {
        let args = args.as_ref();
        let mut cmd = snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("but"));
        if !args.is_empty() {
            cmd = cmd
                .args(shell_words::split(args).expect("statically known args must split correctly"))
        }
        self.with_updated_env(cmd)
            .env("GITBUTLER_CHANGE_ID", "42")
            .env("BUT_OUTPUT_FORMAT", "human")
            .env("NOPAGER", "1")
    }

    fn with_updated_env(&self, cmd: snapbox::cmd::Command) -> snapbox::cmd::Command {
        isolate_snapbox_cmd(cmd)
            .env("E2E_TEST_APP_DATA_DIR", self.app_data_dir())
            .current_dir(self.projects_root())
    }
}

pub trait CommandExt {
    /// Force terminal color codes to be emitted. This should be used when
    /// asserting with [snapbox::cmd::OutputAssert::stdout_eq] against an SVG
    /// file.
    fn with_color_for_svg(self) -> Self;
    /// Change the environment to allow passing `--json`.
    fn allow_json(self) -> Self;
}

impl CommandExt for snapbox::cmd::Command {
    fn with_color_for_svg(self) -> snapbox::cmd::Command {
        self.env("CLICOLOR_FORCE", "1")
    }

    fn allow_json(self) -> Self {
        self.env_remove("BUT_OUTPUT_FORMAT")
    }
}
