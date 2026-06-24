use std::{io::Write, ops::DerefMut, path::Path};

use but_core::{
    RefMetadata, RepositoryExt, WORKSPACE_REF_NAME,
    ref_metadata::{ProjectMeta, StackId, WorkspaceCommitRelation},
};
use but_meta::VirtualBranchesTomlMetadata;
#[cfg(feature = "sandbox-but-api")]
use but_settings::AppSettings;
use gix::bstr::ByteVec;
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
    /// The settings that are used for the application, if they are set.
    #[cfg(feature = "sandbox-but-api")]
    app_settings: Option<AppSettings>,
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
    /// Create a new instance with empty everything, except for basic application settings that prevent the app to break out.
    ///
    /// Change these if you want to test something specific on top of that.
    pub fn empty() -> Sandbox {
        #[cfg_attr(not(feature = "sandbox-but-api"), allow(unused_mut))]
        let mut sandbox = Sandbox {
            project_root: Some(tempfile::TempDir::new().unwrap()),
            #[cfg(feature = "sandbox-but-api")]
            app_root: Some(tempfile::TempDir::new().unwrap()),
            #[cfg(feature = "sandbox-but-api")]
            app_settings: None,
        };
        #[cfg(feature = "sandbox-but-api")]
        sandbox.set_default_settings();
        sandbox
    }

    /// A utility to init a scenario if the legacy feature is set, or open a repo otherwise.
    pub fn open_or_init_scenario_with_target_and_default_settings(name: &str) -> Sandbox {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Allow,
        )
    }

    /// Open a repository without any additional setup and default application settings.
    pub fn open_with_default_settings(name: &str) -> Sandbox {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Disallow,
        )
    }

    /// Provide a scenario with `name` for writing, and `but setup` already invoked to add the project,
    /// with a target added.
    ///
    /// Prefer to use [`Self::open_scenario_with_target_and_default_settings()`] instead for less side-effects
    /// TODO: we shouldn't have to add the project for interaction - it's only useful for listing.
    /// TODO: there should be no need for the target.
    pub fn init_scenario_with_target_and_default_settings(name: &str) -> Sandbox {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Allow,
        )
    }

    /// Provide a scenario with `name` for writing, with target added.
    pub fn open_scenario_with_target_and_default_settings(name: &str) -> Sandbox {
        Self::open_or_init_scenario_with_target_inner(
            name,
            Creation::CopyFromReadOnly,
            InitMetadata::Allow,
        )
    }

    /// Like [`Self::init_scenario_with_target_and_default_settings`], Execute the script at `name` instead of
    /// copying it - necessary if Git places absolute paths.
    pub fn init_scenario_with_target_and_default_settings_slow(name: &str) -> Sandbox {
        Self::open_or_init_scenario_with_target_inner(name, Creation::Execute, InitMetadata::Allow)
    }

    fn open_or_init_scenario_with_target_inner(
        name: &str,
        script_creation: Creation,
        meta_mode: InitMetadata,
    ) -> Sandbox {
        let repo_dir = gix_testtools::scripted_fixture_writable_with_args(
            format!("scenario/{name}.sh"),
            None::<String>,
            script_creation,
        )
        .map_err(anyhow::Error::from_boxed)
        .unwrap();
        #[cfg_attr(not(feature = "sandbox-but-api"), allow(unused_mut))]
        let mut sandbox = Sandbox {
            project_root: Some(repo_dir),
            #[cfg(feature = "sandbox-but-api")]
            app_root: Some(tempfile::TempDir::new().unwrap()),
            #[cfg(feature = "sandbox-but-api")]
            app_settings: None,
        };
        let repo = sandbox.open_repo();

        // This can fail on unborn repos, let it, see if we can handle unborn.
        if matches!(meta_mode, InitMetadata::Allow)
            && let Ok(commit_id) = repo.rev_parse_single("origin/main")
        {
            sandbox.file(
                repo.gitbutler_storage_path()
                    .unwrap()
                    .join("virtual_branches.toml"),
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
        sandbox.set_default_settings();
        sandbox
    }
}

/// Utilities
impl Sandbox {
    /// Create an assert with custom redactions. Adapt as needed.
    pub fn assert_with_oplog_redactions(&self) -> Assert {
        let mut redactions = Redactions::new();
        redactions
            .insert("[HASH]", regex::Regex::new(r#"\b[a-f0-9]{12}\b"#).unwrap())
            .unwrap();
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
    pub fn open_repo(&self) -> gix::Repository {
        gix::open_opts(self.projects_root(), gix::open::Options::isolated()).unwrap()
    }

    /// Create a metadata instance on the project.
    pub fn meta(&self) -> impl but_core::RefMetadata {
        VirtualBranchesTomlMetadata::from_path(
            self.open_repo()
                .gitbutler_storage_path()
                .unwrap()
                .join("virtual_branches.toml"),
        )
        .unwrap()
    }

    /// Read project-scoped metadata, falling back to legacy workspace metadata.
    pub fn project_meta(&self) -> ProjectMeta {
        ProjectMeta::resolve(&self.open_repo(), &self.meta()).unwrap()
    }

    /// Return a fully isolated context configured to interact with this repository.
    ///
    /// ### Not for plumbing
    ///
    /// This feature is only meant for higher-level Client or API tests. Plumbing crates must not use the [`but_ctx::Context`].
    #[cfg(feature = "sandbox-but-api")]
    pub fn context(&self) -> but_ctx::Context {
        but_ctx::Context::from_repo(self.open_repo())
            .map(but_ctx::Context::with_memory_app_cache)
            .unwrap()
    }

    /// Return the graph at `HEAD`, along with the `(graph, repo, meta)` repository and metadata used to create it.
    pub fn graph_at_head(
        &self,
    ) -> (
        but_graph::Graph,
        gix::Repository,
        impl but_core::RefMetadata,
    ) {
        let repo = self.open_repo();
        let meta = self.meta();
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            self.project_meta(),
            but_graph::init::Options::default(),
        )
        .unwrap();
        (graph, repo, meta)
    }

    /// Return a worktree visualisation, freshly read from [Self::graph_at_head()].
    pub fn workspace_debug_at_head(&self) -> String {
        let (graph, _repo, _meta) = self.graph_at_head();
        graph_workspace_determinisitcally(&graph.into_workspace().unwrap()).to_string()
    }

    /// Open the graph at `HEAD` as SVG for debugging.
    #[cfg(unix)]
    pub fn open_graph_at_head_as_svg(&self) {
        let (graph, _repo, _meta) = self.graph_at_head();
        graph.open_as_svg();
    }

    /// Show a git log for all refs.
    pub fn git_log(&self) -> String {
        visualize_commit_graph_all_from_dir(self.projects_root()).unwrap()
    }

    /// Show the `git status` as string.
    pub fn git_status(&self) -> String {
        let repo = self.open_repo();
        git_status(&repo).unwrap()
    }

    /// Return app settings if these were initialized.
    #[cfg(feature = "sandbox-but-api")]
    pub fn try_app_settings(&self) -> Option<&AppSettings> {
        self.app_settings.as_ref()
    }

    /// Return app settings or panic if these weren't initialized.
    #[cfg(feature = "sandbox-but-api")]
    pub fn app_settings(&self) -> &AppSettings {
        self.app_settings
            .as_ref()
            .expect("BUG: must not call this in an empty or partially initialised sandbox")
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

    /// Read a file at `path` from our projects root.
    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<String, gix::bstr::FromUtf8Error> {
        std::fs::read(self.projects_root().join(path))
            .expect("File exists and can be read")
            .into_string()
    }

    /// Prepend `data` to `path` in our projects root.
    pub fn prepend_file(&self, path: impl AsRef<Path>, data: &str) -> &Self {
        let path = self.projects_root().join(path);
        let mut existing = std::fs::read(&path).expect("File exists and can be read");
        let mut contents = Vec::with_capacity(data.len() + existing.len());
        contents.extend_from_slice(data.as_bytes());
        contents.append(&mut existing);
        std::fs::write(path, contents).expect("prepending should always work");
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

    /// Remove a file in our projects root.
    pub fn remove_file(&self, path: impl AsRef<Path>) -> &Self {
        std::fs::remove_file(self.projects_root().join(path)).expect("failed to remove file");
        self
    }

    /// Rename a file in our projects root.
    pub fn rename_file(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> &Self {
        std::fs::rename(
            self.projects_root().join(from),
            self.projects_root().join(to),
        )
        .expect("failed to rename file");
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
    /// Return its trimmed `stdout` for consumption.
    ///
    /// # Use `gix::Repository` if you can
    ///
    /// This method should only be used if there is no convenient way to do this via [`Self::open_repo()`].
    pub fn invoke_git(&self, args: &str) -> String {
        let cmd = snapbox::cmd::Command::new(gix::path::env::exe_invocation());
        let output = isolate_snapbox_cmd(cmd)
            .current_dir(self.projects_root())
            .args(shell_words::split(args).expect("statically known args must split correctly"))
            .output()
            .expect("git should execute");
        assert!(
            output.status.success(),
            "git {args} failed with {status:?}",
            status = output.status
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Invoke an isolated `git` with the given `args`, assert it fails, and return the trimmed stdout.
    ///
    /// Use this when the command is expected to fail for a specific `reason`.
    pub fn invoke_git_fails(&self, args: &str, reason: &str) -> String {
        let cmd = snapbox::cmd::Command::new(gix::path::env::exe_invocation());
        let output = isolate_snapbox_cmd(cmd)
            .current_dir(self.projects_root())
            .args(shell_words::split(args).expect("statically known args must split correctly"))
            .output()
            .expect("git should execute");
        assert!(
            !output.status.success(),
            "git {args} should fail because: {reason}; got status {status:?}",
            status = output.status
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Invoke the given `script` in `bash` in this sandbox
    pub fn invoke_bash(&self, script: impl AsRef<str>) -> &Self {
        invoke_bash_at_dir(script.as_ref(), self.projects_root());
        self
    }

    // TODO: in most cases, this shouldn't be necessary as single-branch mode is a thing.
    //       Review each usage, try without.
    /// Create stack metadata for `branch_names` and return its StackIds, one per item in the input slice, in order.
    pub fn setup_metadata(&self, branch_names: &[&str]) -> Vec<StackId> {
        let mut meta = self.meta();
        let mut ws = meta.workspace(r(WORKSPACE_REF_NAME)).unwrap();
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
        let project_meta = ws.project_meta();
        meta.set_workspace(&ws).unwrap();
        project_meta
            .persist_to_local_config(&self.open_repo())
            .unwrap();

        out
    }

    /// Set target sha to a given refspec
    ///
    /// Returns the target sha we ended up setting.
    pub fn set_target_sha(&self, spec: &str) -> gix::ObjectId {
        let mut meta = self.meta();
        let mut ws = meta.workspace(r(WORKSPACE_REF_NAME)).unwrap();
        let repo = self.open_repo();
        let target_sha = repo.rev_parse_single(spec).unwrap();
        let mut project_meta = ws.project_meta();
        project_meta.target_commit_id = Some(target_sha.detach());
        ws.set_project_meta(project_meta);
        let project_meta = ws.project_meta();
        meta.set_workspace(&ws).unwrap();
        project_meta.persist_to_local_config(&repo).unwrap();

        target_sha.detach()
    }
}

impl Sandbox {
    #[cfg(feature = "sandbox-but-api")]
    fn set_default_settings(&mut self) {
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
                app_distinct_id: None,
                migrated_from_legacy: true,
            },
            github_oauth_app: GitHubOAuthAppSettings {
                oauth_client_id: "but journey tests won't use github".to_string(),
            },
            feature_flags: FeatureFlags {
                cv3: true,
                unapply_v3_pgm: false,
                undo: true,
                rules: true,
                single_branch: true,
                irc: false,
                watch_mode: "auto".into(),
                write_commit_evolution: true,
                tui_file_browser: false,
            },
            extra_csp: ExtraCsp {
                hosts: vec![],
                img_src: vec![],
            },
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
                no_shadow: false,
                cli_is_managed_by_package_manager: false,
                #[expect(deprecated)]
                check_for_updates_interval_in_seconds: 0,
            },
            app_updates_check_interval_sec: 0,
            irc: but_settings::app_settings::IrcSettings {
                server: but_settings::app_settings::IrcServerSettings {
                    host: "irc.example.com".to_string(),
                    port: 6697,
                },
                auto_share: false,
                project_channel: None,
                connection: but_settings::app_settings::IrcConnectionSettings {
                    enabled: false,
                    nickname: None,
                    server_password: None,
                    sasl_password: None,
                    realname: None,
                },
            },
        };
        settings
            .save(&self.app_data_dir().join("gitbutler/settings.json"), None)
            .unwrap();
        self.app_settings = Some(settings);
    }
}

pub fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}
