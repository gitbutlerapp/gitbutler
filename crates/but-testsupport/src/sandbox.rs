use std::{io::Write, ops::DerefMut, path::Path};

use but_core::{
    RefMetadata,
    ref_metadata::{StackId, WorkspaceCommitRelation},
};
use but_meta::VirtualBranchesTomlMetadata;
use gix_testtools::{Creation, tempfile};
use snapbox::{Assert, Redactions};

use crate::{
    git_status, graph_workspace_determinisitcally, invoke_bash_at_dir, isolate_snapbox_cmd,
    visualize_commit_graph_all_from_dir,
};

/// A sandbox for a GitButler application that assumes read-write testing, so all data is editable and is cleaned up afterward.
pub struct Sandbox {
    /// The directory to hold the repository to work with, either bare or non-bare.
    project_root: Option<tempfile::TempDir>,
    /// The space where the application can put its application-wide metadata.
    /// The more optional this is, the more testable the application.
    #[cfg(feature = "sandbox-but-api")]
    app_root: Option<tempfile::TempDir>,
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if std::env::var_os("GITBUTLER_TESTS_NO_CLEANUP").is_none() {
            return;
        }
        #[cfg(feature = "sandbox-but-api")]
        {
            _ = self.app_root.take().unwrap().keep();
        }
        _ = self.project_root.take().unwrap().keep();
    }
}

/// TODO: remove this once there is no old code anymore that needs target information
enum InitMetadata {
    Allow,
    Disallow,
}

/// Lifecycle
impl Sandbox {
    /// Create a new instance with empty everything.
    pub fn empty() -> anyhow::Result<Sandbox> {
        Ok(Sandbox {
            project_root: Some(tempfile::TempDir::new()?),
            #[cfg(feature = "sandbox-but-api")]
            app_root: Some(tempfile::TempDir::new()?),
        })
    }

    /// A utility to init a scenario if the legacy feature is set, or open a repo otherwise.
    pub fn open_or_init_scenario_with_target_and_default_settings(
        name: &str,
    ) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Allow,
        )
    }

    /// Open a repository without any additional setup and default application settings.
    pub fn open_with_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Disallow,
        )
    }

    /// Provide a scenario with `name` for writing, and `but init` already invoked to add the project,
    /// with a target added.
    ///
    /// Prefer to use [`Self::open_scenario_with_target_and_default_settings()`] instead for less side-effects
    /// TODO: we shouldn't have to add the project for interaction - it's only useful for listing.
    /// TODO: there should be no need for the target.
    pub fn init_scenario_with_target_and_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Allow,
        )
    }

    /// Provide a scenario with `name` for writing, with target added.
    pub fn open_scenario_with_target_and_default_settings(name: &str) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Allow,
        )
    }

    /// Like [`Self::init_scenario_with_target_and_default_settings`], Execute the script at `name` instead of
    /// copying it - necessary if Git places absolute paths.
    pub fn init_scenario_with_target_and_default_settings_slow(
        name: &str,
    ) -> anyhow::Result<Sandbox> {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::ExecuteScript,
            InitMetadata::Allow,
        )
    }

    fn open_or_init_scenario_with_target_inner(
        name: &str,
        script_creation: Creation,
        meta_mode: InitMetadata,
    ) -> anyhow::Result<Sandbox> {
        let repo_dir = gix_testtools::scripted_fixture_writable_with_args(
            format!("scenario/{name}.sh"),
            None::<String>,
            script_creation,
        )
        .map_err(anyhow::Error::from_boxed)?;
        let sandbox = Sandbox {
            project_root: Some(repo_dir),
            #[cfg(feature = "sandbox-but-api")]
            app_root: Some(tempfile::TempDir::new()?),
        };
        let repo = sandbox.open_repo()?;

        // This can fail on unborn repos, let it, see if we can handle unborn.
        if matches!(meta_mode, InitMetadata::Allow)
            && let Ok(commit_id) = repo.rev_parse_single("origin/main")
        {
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
        #[cfg(feature = "sandbox-but-api")]
        sandbox.set_default_settings()?;
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
                regex::Regex::new(r#"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}"#)
                    .unwrap(),
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
            self.project_root.take().unwrap().keep()
        );
        #[cfg(feature = "sandbox-but-api")]
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

    /// Return a context configured to interact with this repository.
    ///
    /// Note that in legacy mode, it will provide a minimal `LegacyProject` as well, but with all settings defaulted.
    ///
    /// ### Not for plumbing
    ///
    /// This feature is only meant for higher-level Client or API tests. Plumbing crates must not use the [`but_ctx::Context`].
    #[cfg(feature = "sandbox-but-api")]
    pub fn context(&self) -> anyhow::Result<but_ctx::Context> {
        self.open_repo()?.try_into()
    }

    /// Return the graph at `HEAD`, along with the `(graph, repo, meta)` repository and metadata used to create it.
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

    /// Return a worktree visualisation, freshly read from [Self::graph_at_head()].
    pub fn workspace_debug_at_head(&self) -> anyhow::Result<String> {
        let (graph, _repo, _meta) = self.graph_at_head()?;
        Ok(graph_workspace_determinisitcally(&graph.to_workspace()?).to_string())
    }

    /// Open the graph at `HEAD` as SVG for debugging.
    #[cfg(unix)]
    pub fn open_graph_at_head_as_svg(&self) -> anyhow::Result<()> {
        let (graph, _repo, _meta) = self.graph_at_head()?;
        graph.open_as_svg();
        Ok(())
    }

    /// Show a git log for all refs.
    pub fn git_log(&self) -> anyhow::Result<String> {
        Ok(visualize_commit_graph_all_from_dir(self.projects_root())?)
    }

    /// Show the `git status` as string.
    pub fn git_status(&self) -> anyhow::Result<String> {
        let repo = self.open_repo()?;
        Ok(git_status(&repo)?)
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

    /// The root directory of the project itself, either the `workdir` or `gitdir` if the underlying repository is bare.
    pub fn projects_root(&self) -> &Path {
        self.project_root.as_ref().unwrap().path()
    }

    /// A place for the application to store
    #[cfg(feature = "sandbox-but-api")]
    pub fn app_data_dir(&self) -> &Path {
        self.app_root.as_ref().unwrap().path()
    }
}

/// Invocations
impl Sandbox {
    /// Invoke an isolated `git` with the given `args`, which will be split automatically.
    pub fn invoke_git(&self, args: &str) -> &Self {
        let cmd = snapbox::cmd::Command::new(gix::path::env::exe_invocation());
        isolate_snapbox_cmd(cmd)
            .current_dir(self.projects_root())
            .args(shell_words::split(args).expect("statically known args must split correctly"))
            .assert()
            .success();
        self
    }

    /// Invoke the given `script` in `bash` in this sandbox
    pub fn invoke_bash(&self, script: impl AsRef<str>) -> &Self {
        invoke_bash_at_dir(script.as_ref(), self.projects_root());
        self
    }

    // TODO: in most cases, this shouldn't be necessary as single-branch mode is a thing.
    //       Review each usage, try without.
    /// Create stack metadata for `branch_names` and return its StackIds, one per item in the input slice, in order.
    pub fn setup_metadata(&self, branch_names: &[&str]) -> anyhow::Result<Vec<StackId>> {
        let mut meta = self.meta()?;
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
        let out = ws_data.stacks.iter().map(|s| s.id).collect();
        meta.set_workspace(&ws)?;

        Ok(out)
    }
}

impl Sandbox {
    #[cfg(feature = "sandbox-but-api")]
    fn set_default_settings(&self) -> anyhow::Result<()> {
        use but_settings::{
            AppSettings,
            app_settings::{
                Claude, ExtraCsp, FeatureFlags, Fetch, GitHubOAuthAppSettings, Reviews,
                TelemetrySettings, UiSettings,
            },
        };
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
        settings.save(&self.app_data_dir().join("gitbutler/settings.json"))
    }
}

pub fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}
