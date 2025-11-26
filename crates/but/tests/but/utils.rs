use std::{env, io::Write, ops::DerefMut, path::Path};

use but_core::{
    RefMetadata,
    ref_metadata::{StackId, WorkspaceCommitRelation},
};
use but_meta::VirtualBranchesTomlMetadata;
use but_settings::{
    AppSettings,
    app_settings::{
        Claude, ExtraCsp, FeatureFlags, Fetch, GitHubOAuthAppSettings, Reviews, TelemetrySettings,
        UiSettings,
    },
};
use but_testsupport::gix_testtools::{Creation, tempfile};
use snapbox::{Assert, Redactions};

/// A sandbox that assumes read-write testing, so all data is editable and is cleaned up afterward.
pub struct Sandbox {
    /// The directory to hold at least one project to work with.
    projects_root: Option<tempfile::TempDir>,
    /// The space where the application can put its application-wide metadata.
    /// The more optional this is, the more testable the application.
    app_root: Option<tempfile::TempDir>,
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if std::env::var_os("GITBUTLER_TESTS_NO_CLEANUP").is_none() {
            return;
        }
        _ = self.app_root.take().unwrap().keep();
        _ = self.projects_root.take().unwrap().keep();
    }
}

//  TODO: prior init shouldn't be necessary
enum Mode {
    Init,
    Open,
}

/// Lifecycle
impl Sandbox {
    /// Create a new instance with empty everything.
    pub fn empty() -> anyhow::Result<Sandbox> {
        Ok(Sandbox {
            projects_root: Some(tempfile::TempDir::new()?),
            app_root: Some(tempfile::TempDir::new()?),
        })
    }

    /// Provide a scenario with `name` for writing, and `but init` already invoked to add the project,
    /// with a target added.
    ///
    /// Prefer to use [`Self::open_scenario_with_target_and_default_settings()`] instead for less side-effects
    /// TODO: we shouldn't have to add the project for interaction - it's only useful for listing.
    /// TODO: there should be no need for the target.
    pub fn init_scenario_with_target_and_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(name, Creation::CopyFromReadOnly, Mode::Init)
    }

    /// Provide a scenario with `name` for writing, with target added.
    ///
    /// TODO: we shouldn't have to add the project for interaction - it's only useful for listing.
    /// TODO: there should be no need for the target.
    pub fn open_scenario_with_target_and_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(name, Creation::CopyFromReadOnly, Mode::Open)
    }

    /// Like [`Self::init_scenario_with_target_and_default_settings`], Execute the script at `name` instead of
    /// copying it - necessary if Git places absolute paths.
    pub fn init_scenario_with_target_and_default_settings_slow(
        name: &str,
    ) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(name, Creation::ExecuteScript, Mode::Init)
    }

    fn open_or_init_scenario_with_target_inner(
        name: &str,
        script_creation: Creation,
        mode: Mode,
    ) -> anyhow::Result<Sandbox> {
        let project = but_testsupport::gix_testtools::scripted_fixture_writable_with_args(
            format!("scenario/{name}.sh"),
            None::<String>,
            script_creation,
        )
        .map_err(anyhow::Error::from_boxed)?;
        let sandbox = Sandbox {
            projects_root: Some(project),
            app_root: Some(tempfile::TempDir::new()?),
        };
        let repo = sandbox.open_repo()?;

        // This can fail on unborn repos, let it, see if we can handle unborn.
        if let Ok(commit_id) = repo.rev_parse_single("origin/main") {
            sandbox.file(
                ".git/gitbutler/virtual_branches.toml",
                r#"
[default_target]
branchName = "main"
remoteName = "origin"
remoteUrl = "https://github.com/gitbutlerapp/gitbutler"
sha = "<EXTRA_TARGET>"
pushRemoteName = "origin"

[branch_targets]

[branches]
        "#
                .replace("<EXTRA_TARGET>", &commit_id.to_string()),
            );
        }
        sandbox.set_default_settings()?;
        match mode {
            Mode::Init => {
                sandbox.but("init").assert().success();
            }
            Mode::Open => {}
        }
        Ok(sandbox)
    }
}

/// Utilities
impl Sandbox {
    /// Create an assert with custom redactions. Adapt as needed.
    pub fn assert_with_oplog_redactions(&self) -> Assert {
        let mut redactions = Redactions::new();
        redactions
            .insert(
                "[SHORTHASH]",
                regex::Regex::new(r#"[a-f0-9]{7}\b"#).unwrap(),
            )
            .unwrap();
        redactions
            .insert(
                "[MICROHASH]",
                regex::Regex::new(r#"\b[a-f0-9]{5}\b"#).unwrap(),
            )
            .unwrap();
        Assert::new()
            .action_env("SNAPSHOTS")
            .redact_with(redactions)
    }

    /// Create an assert with custom redactions. Adapt as needed.
    pub fn assert_with_uuid_and_timestamp_redactions(&self) -> Assert {
        let mut redactions = Redactions::new();
        redactions
            .insert(
                "[UUID]",
                regex::Regex::new(r#"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}"#).unwrap(),
            )
            .unwrap();
        redactions
            .insert("[TIMESTAMP]", regex::Regex::new(r#"[1-9]\d{12}"#).unwrap())
            .unwrap();
        // Match RFC3339 timestamps like "2025-11-18T22:27:14+00:00" or "2025-10-31T13:01:58.072+00:00"
        redactions
            .insert(
                "[RFC_TIMESTAMP]",
                regex::Regex::new(
                    r#"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?\+\d{2}:\d{2}"#,
                )
                .unwrap(),
            )
            .unwrap();
        Assert::new()
            .action_env("SNAPSHOTS")
            .redact_with(redactions)
    }

    /// Print the paths to our directories, and keep them.
    pub fn debug(mut self) -> ! {
        eprintln!(
            "projects_root: {:?}",
            self.projects_root.take().unwrap().keep()
        );
        eprintln!("app_root: {:?}", self.app_root.take().unwrap().keep());
        todo!("Check the directories manually")
    }

    /// Open a repository on the projects-directory.
    pub fn open_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open_opts(
            self.projects_root(),
            gix::open::Options::isolated(),
        )?)
    }

    /// Create a metadata instance on the project.
    pub fn meta(&self) -> anyhow::Result<impl but_core::RefMetadata> {
        VirtualBranchesTomlMetadata::from_path(
            self.projects_root()
                .join(".git/gitbutler/virtual_branches.toml"),
        )
    }

    /// return the graph at `HEAD`, along with the `(graph, repo, meta)` repository and metadata used to create it.
    pub fn graph_at_head(
        &self,
    ) -> anyhow::Result<(
        but_graph::Graph,
        gix::Repository,
        impl but_core::RefMetadata,
    )> {
        let repo = self.open_repo()?;
        let meta = self.meta()?;
        let graph = but_graph::Graph::from_head(&repo, &meta, Default::default())?;
        Ok((graph, repo, meta))
    }

    /// Show a git log for all refs.
    pub fn git_log(&self) -> anyhow::Result<String> {
        Ok(but_testsupport::visualize_commit_graph_all_from_dir(
            self.projects_root(),
        )?)
    }

    /// Show the `git status` as string.
    pub fn git_status(&self) -> anyhow::Result<String> {
        let repo = self.open_repo()?;
        Ok(but_testsupport::git_status(&repo)?)
    }

    /// Write `data` to `path` in our projects root, creating a new file.
    pub fn file(&self, path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> &Self {
        let path = self.projects_root().join(path);
        std::fs::create_dir_all(path.parent().unwrap())
            .expect("parent directory has nothing in its way");
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .expect("File does not exists and can be opened")
            .write_all(data.as_ref())
            .expect("writes should work");
        self
    }

    /// Append `data` to `path` in our projects root.
    pub fn append_file(&self, path: impl AsRef<Path>, data: &str) -> &Self {
        std::fs::OpenOptions::new()
            .append(true)
            .create(false)
            .open(self.projects_root().join(path))
            .expect("File exists and can be opened")
            .write_all(data.as_ref())
            .expect("appending should always work");
        self
    }
}

/// Invocations
impl Sandbox {
    /// Create a command suitable for testing the output of the invocation with `args`.
    /// Note that more args can be added later as well, whie `None` is a valid argument as well.
    pub fn but<'a>(&self, args: impl Into<Option<&'a str>>) -> snapbox::cmd::Command {
        let args = args.into();
        let mut cmd = snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("but"));
        if let Some(args) = args {
            cmd = cmd
                .args(shell_words::split(args).expect("statically known args must split correctly"))
        }
        self.with_updated_env(cmd)
            .env("GITBUTLER_CHANGE_ID", "42")
            .env("BUT_OUTPUT_FORMAT", "human")
            .env("NOPAGER", "1")
    }

    /// Invoke an isolated `git` with the given `args`, which will be split automatically.
    pub fn invoke_git(&self, args: &str) -> &Self {
        let cmd = snapbox::cmd::Command::new(gix::path::env::exe_invocation());
        self.with_updated_env(cmd)
            .args(shell_words::split(args).expect("statically known args must split correctly"))
            .assert()
            .success();
        self
    }

    /// Invoke the given `script` in `bash` in this sandbox
    pub fn invoke_bash(&self, script: impl AsRef<str>) -> &Self {
        but_testsupport::invoke_bash_at_dir(script.as_ref(), self.projects_root());
        self
    }
}

impl Sandbox {
    fn set_default_settings(&self) -> anyhow::Result<()> {
        let settings = AppSettings {
            context_lines: 3,
            onboarding_complete: true,
            telemetry: TelemetrySettings {
                app_metrics_enabled: false,
                app_error_reporting_enabled: false,
                app_non_anon_metrics_enabled: false,
                app_distinct_id: None,
            },
            github_oauth_app: GitHubOAuthAppSettings {
                oauth_client_id: "but journey tests won't use github".to_string(),
            },
            feature_flags: FeatureFlags {
                ws3: true,
                cv3: true,
                apply3: true,
                undo: true,
                rules: true,
                single_branch: true,
            },
            extra_csp: ExtraCsp { hosts: vec![] },
            fetch: Fetch {
                auto_fetch_interval_minutes: 0,
            },
            claude: Claude {
                executable: "".to_string(),
                notify_on_completion: false,
                notify_on_permission_request: false,
                dangerously_allow_all_permissions: false,
                auto_commit_after_completion: false,
                use_configured_model: false,
            },
            reviews: Reviews {
                auto_fill_pr_description_from_commit: false,
            },
            ui: UiSettings {
                use_native_title_bar: false,
            },
        };
        settings.save(&self.app_root().join("gitbutler/settings.json"))
    }

    fn projects_root(&self) -> &Path {
        self.projects_root.as_ref().unwrap().path()
    }

    fn app_root(&self) -> &Path {
        self.app_root.as_ref().unwrap().path()
    }

    fn with_updated_env(&self, cmd: snapbox::cmd::Command) -> snapbox::cmd::Command {
        but_testsupport::isolate_snapbox_cmd(cmd)
            .env("E2E_TEST_APP_DATA_DIR", self.app_root())
            .current_dir(self.projects_root())
    }
}

pub fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}

// TODO: in most cases, this shouldn't be necessary as single-branch mode is a thing.
//       Review each usage, try without.
pub fn setup_metadata(env: &Sandbox, branch_names: &[&str]) -> anyhow::Result<()> {
    let mut meta = env.meta()?;
    let mut ws = meta.workspace(r("refs/heads/gitbutler/workspace"))?;
    let ws_data: &mut but_core::ref_metadata::Workspace = ws.deref_mut();
    for (stable_id, branch_name) in (0_u128..).zip(branch_names.iter()) {
        ws_data.add_or_insert_new_stack_if_not_present(
            r(&format!("refs/heads/{branch_name}")),
            None,
            WorkspaceCommitRelation::Merged,
            |_| StackId::from_number_for_testing(stable_id),
        );
    }
    meta.set_workspace(&ws)?;

    Ok(())
}
