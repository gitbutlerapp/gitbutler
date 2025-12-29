use std::{
    path::{Path, PathBuf},
    time,
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::default_true::DefaultTrue;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AuthKey {
    GitCredentialsHelper,
    Local {
        private_key_path: PathBuf,
    },
    // There used to be more auth option variants that we are deprecating and replacing with this
    #[serde(other)]
    #[default]
    SystemExecutable,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiProject {
    pub name: String,
    pub description: Option<String>,
    pub repository_id: String,
    /// The "gitbuler data, i.e. oplog" URL
    pub git_url: String,
    /// The "project" git URL
    pub code_git_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Determines if the project Operations log will be synched with the GitButHub
    pub sync: bool,
    /// Determines if the project code will be synched with the GitButHub
    #[serde(default)]
    pub sync_code: bool,
    // if this project is using gitbutler reviews
    #[serde(default)]
    pub reviews: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FetchResult {
    Fetched {
        timestamp: time::SystemTime,
    },
    Error {
        timestamp: time::SystemTime,
        error: String,
    },
}

impl FetchResult {
    pub fn timestamp(&self) -> time::SystemTime {
        match self {
            FetchResult::Fetched { timestamp } | FetchResult::Error { timestamp, .. } => *timestamp,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub struct CodePushState {
    #[serde(with = "but_serde::oid")]
    pub id: git2::Oid,
    pub timestamp: time::SystemTime,
}

pub type ProjectId = but_core::Id<'P'>;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Project {
    // TODO: We shouldn't need these IDs and most definitely shouldn't persist them.
    //       A project is a `git_dir`, and from there all other project data can be derived.
    pub id: ProjectId,
    pub title: String,
    pub description: Option<String>,
    /// The worktree directory of the project's repository.
    // TODO: Make it optional for bare repo support!
    // TODO: Do not actually store it, but obtain it on the fly by using a repository!
    #[serde(rename = "path")]
    // TODO(1.0): enable the line below to clear the value from storage - we only want the git dir,
    //       but need to remain compatible. The frontend shouldn't care, so we may need a specific type for that
    //       which already exists, but… needs cleanup.
    //       However, this field SHOULD STAY to present better errors when the path isn't there anymore.
    //       But it must still be optional.
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_dir: PathBuf,
    /// The storage location of the Git repository itself.
    /// This is the only value we need to access everything related to the Git repository.
    ///
    // TODO(1.0): remove the `default` which is just needed while there is project files without it.
    #[serde(default)]
    pub(crate) git_dir: PathBuf,
    #[serde(default)]
    pub preferred_key: AuthKey,
    /// if ok_with_force_push is true, we'll not try to avoid force pushing
    /// for example, when updating base branch
    #[serde(default)]
    pub ok_with_force_push: DefaultTrue,
    /// Force push protection uses safer force push flags instead of doing straight force pushes
    #[serde(default)]
    pub force_push_protection: bool,
    pub api: Option<ApiProject>,
    #[serde(default)]
    pub gitbutler_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_code_push_state: Option<CodePushState>,
    #[serde(default)]
    pub project_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub omit_certificate_check: Option<bool>,
    // The number of changed lines that will trigger a snapshot
    pub snapshot_lines_threshold: Option<usize>,
    #[serde(default)]
    pub forge_override: Option<String>,
    #[serde(
        default,
        deserialize_with = "but_forge::deserialize_preferred_forge_user_opt"
    )]
    pub preferred_forge_user: Option<but_forge::ForgeUser>,
}

impl Project {
    /// Return a new instance with `id` and all other fields defaulted.
    pub fn default_with_id(id: ProjectId) -> Self {
        Project {
            id,
            title: "".to_string(),
            description: None,
            worktree_dir: Default::default(),
            git_dir: Default::default(),
            preferred_key: Default::default(),
            ok_with_force_push: Default::default(),
            force_push_protection: false,
            api: None,
            gitbutler_data_last_fetch: None,
            gitbutler_code_push_state: None,
            project_data_last_fetch: None,
            omit_certificate_check: None,
            snapshot_lines_threshold: None,
            forge_override: None,
            preferred_forge_user: None,
        }
    }

    /// A utility to support old code for basic path needs, but without actually needing full
    /// or meaningful metadata.
    pub fn with_paths_for_testing(
        mut self,
        git_dir: PathBuf,
        worktree_dir: Option<PathBuf>,
    ) -> Self {
        self.git_dir = git_dir;
        if let Some(worktree_dir) = worktree_dir {
            self.worktree_dir = worktree_dir;
        }
        self
    }
}

/// Testing
// TODO: remove once `gitbutler-testsupport` isn't needed anymore, and `gitbutler-repo`
impl Project {
    /// A special constructor needed as `worktree_dir` isn't accessible anymore.
    pub fn new_for_gitbutler_testsupport(title: String, worktree_dir: PathBuf) -> Self {
        Project {
            title,
            worktree_dir,
            ..Project::default_with_id(ProjectId::generate())
        }
        .migrated()
        .unwrap()
    }

    /// A special constructor needed as `worktree_dir` isn't accessible anymore.
    pub fn new_for_gitbutler_repo(worktree_dir: PathBuf, preferred_key: AuthKey) -> Self {
        Project {
            worktree_dir,
            preferred_key,
            ..Project::default_with_id(ProjectId::generate())
        }
        .migrated()
        .unwrap()
    }

    /// Call this after each invocation of `list()` with manual filtering to get fields filled in.
    pub fn migrated(mut self) -> anyhow::Result<Self> {
        self.migrate()?;
        Ok(self)
    }
}

impl Project {
    /// Return `true` if the project was migrated, and thus is changed, or `false` otherwise.
    pub fn migrate(&mut self) -> anyhow::Result<bool> {
        if !self.git_dir.as_os_str().is_empty() {
            return Ok(false);
        }
        let repo = gix::open_opts(&self.worktree_dir, gix::open::Options::isolated()).inspect_err(
            |err| {
                tracing::error!(
                    "failed to open worktree at {} for migration: {err}",
                    self.worktree_dir.display()
                )
            },
        )?;
        self.git_dir = repo.git_dir().to_owned();
        // NOTE: we set the worktree so the frontend is happier until this usage can be reviewed,
        // probably for supporting bare repositories.
        self.worktree_dir = repo
            .workdir()
            .context("BUG: we currently only support non-bare repos, yet this one didn't have a worktree dir")?
            .to_owned();
        Ok(true)
    }

    pub(crate) fn worktree_dir_but_should_use_git_dir(&self) -> &Path {
        &self.worktree_dir
    }
}

/// Instantiation
impl Project {
    /// Search upwards from `path` to discover a Git worktree.
    pub fn from_path(path: &Path) -> anyhow::Result<Project> {
        let worktree_dir = gix::discover(path)?
            .workdir()
            .context("Bare repositories aren't supported")?
            .to_owned();
        Project {
            worktree_dir,
            ..Project::default_with_id(ProjectId::generate())
        }
        .migrated()
    }
    /// Finds an existing project by its path. Errors out if not found.
    pub fn find_by_worktree_dir(worktree_dir: &Path) -> anyhow::Result<Project> {
        Self::find_by_worktree_dir_opt(worktree_dir)?
            .context("No project found with the given path")
    }

    /// Finds an existing project by its path or return `None` if there was none. Errors out if not found.
    // TODO: find by git-dir instead!
    pub fn find_by_worktree_dir_opt(worktree_dir: &Path) -> anyhow::Result<Option<Project>> {
        let mut projects = crate::dangerously_list_projects_without_migration()?;
        // Sort projects by longest pathname to shortest.
        // We need to do this because users might have one gitbutler project
        // nested inside another via a gitignored folder.
        // We want to match on the longest project path.
        projects.sort_by(|a, b| {
            a.worktree_dir
                .as_os_str()
                .len()
                .cmp(&b.worktree_dir.as_os_str().len())
                // longest first
                .reverse()
        });
        let resolved_path = if worktree_dir.is_relative() {
            worktree_dir
                .canonicalize()
                .context("Failed to canonicalize path")?
        } else {
            worktree_dir.to_path_buf()
        };

        let Some(project) = projects.into_iter().find(|p| {
            // Canonicalize project path for comparison
            match p.worktree_dir.canonicalize() {
                Ok(proj_canon) => resolved_path.starts_with(proj_canon),
                Err(_) => false,
            }
        }) else {
            return Ok(None);
        };
        project.migrated().map(Some)
    }
}

/// Repository helpers.
impl Project {
    /// Open an isolated repository, one that didn't read options beyond `.git/config` and
    /// knows no environment variables.
    ///
    /// Use it for fastest-possible access, when incomplete configuration is acceptable.
    pub fn open_isolated_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open_opts(
            self.git_dir(),
            gix::open::Options::isolated(),
        )?)
    }

    /// Open a standard Git repository at the project directory, just like a real user would.
    ///
    /// This repository is good for standard tasks, like checking refs and traversing the commit graph,
    /// and for reading objects as well.
    ///
    /// Diffing and merging is better done with [`Self::open_repo_for_merging()`].
    pub fn open_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open(self.git_dir())?)
    }

    /// Calls [`but_core::open_repo_for_merging()`]
    pub fn open_repo_for_merging(&self) -> anyhow::Result<gix::Repository> {
        but_core::open_repo_for_merging(self.git_dir())
    }

    /// Open a git2 repository.
    /// Deprecated, but still in use.
    pub fn open_git2(&self) -> anyhow::Result<git2::Repository> {
        Ok(git2::Repository::open(self.git_dir())?)
    }
}

impl Project {
    /// Determines if the project Operations log will be synched with the GitButHub
    pub fn oplog_sync_enabled(&self) -> bool {
        let has_url = self.api.as_ref().map(|api| api.git_url.clone()).is_some();
        self.api.as_ref().map(|api| api.sync).unwrap_or_default() && has_url
    }
    /// Determines if the project code will be synched with the GitButHub
    pub fn code_sync_enabled(&self) -> bool {
        let has_code_url = self
            .api
            .as_ref()
            .and_then(|api| api.code_git_url.clone())
            .is_some();
        self.api
            .as_ref()
            .map(|api| api.sync_code)
            .unwrap_or_default()
            && has_code_url
    }

    pub fn has_code_url(&self) -> bool {
        self.api
            .as_ref()
            .map(|api| api.code_git_url.is_some())
            .unwrap_or_default()
    }

    /// Returns the path to the directory containing the `GitButler` state for this project.
    ///
    /// Normally this is `.git/gitbutler` in the project's repository.
    pub fn gb_dir(&self) -> PathBuf {
        self.git_dir().join("gitbutler")
    }

    pub fn snapshot_lines_threshold(&self) -> usize {
        self.snapshot_lines_threshold.unwrap_or(20)
    }

    // TODO(ST): Actually remove this - people should use the `gix::Repository` for worktree handling (which makes it optional, too)
    pub fn worktree_dir(&self) -> anyhow::Result<&Path> {
        // TODO: open a repo and get the workdir.
        // For now we don't have to open a repo as we only support repos with worktree.
        Ok(&self.worktree_dir)
    }

    /// Set the worktree directory to `worktree_dir`.
    pub fn set_worktree_dir(&mut self, worktree_dir: PathBuf) -> anyhow::Result<()> {
        let repo = gix::open_opts(&worktree_dir, gix::open::Options::isolated())?;
        self.worktree_dir = worktree_dir;
        self.git_dir = repo.git_dir().to_owned();
        Ok(())
    }

    /// Return the path to the directory that holds the repository data and that is associated with the current worktree.
    pub fn git_dir(&self) -> &Path {
        assert!(
            !self.git_dir.as_os_str().is_empty(),
            "BUG: must call `project.migrated()` before using the git_dir to have it initialised."
        );
        &self.git_dir
    }

    /// Return the path to the Git directory of the 'prime' repository, the one that holds all worktrees.
    pub fn common_git_dir(&self) -> anyhow::Result<PathBuf> {
        let repo = self.open_isolated_repo()?;
        Ok(repo.common_dir().to_owned())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, strum::Display)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum AddProjectOutcome {
    Added(Project),
    AlreadyExists(Project),
    PathNotFound,
    NotADirectory,
    BareRepository,
    NonMainWorktree,
    NoWorkdir,
    NoDotGitDirectory,
    NotAGitRepository(String),
}

impl AddProjectOutcome {
    /// This is for tests only.
    ///
    /// Unwraps the `Project` if the project was actually added.
    /// Panics if it was not.
    pub fn unwrap_project(self) -> Project {
        match self {
            AddProjectOutcome::Added(p) => p,
            _ => panic!("called `AddProjectOutcome::unwrap_project()` on a non-project outcome"),
        }
    }

    /// Try to get the `Project`, returning an error if it was not added.
    pub fn try_project(self) -> anyhow::Result<Project> {
        match self {
            AddProjectOutcome::Added(p) => Ok(p),
            AddProjectOutcome::AlreadyExists(_) => Err(anyhow::anyhow!("project already exists")),
            AddProjectOutcome::PathNotFound => Err(anyhow::anyhow!("project path not found")),
            AddProjectOutcome::NotADirectory => {
                Err(anyhow::anyhow!("project path is not a directory"))
            }
            AddProjectOutcome::BareRepository => {
                Err(anyhow::anyhow!("bare repositories are not supported"))
            }
            AddProjectOutcome::NonMainWorktree => {
                Err(anyhow::anyhow!("non-main worktrees are not supported"))
            }
            AddProjectOutcome::NoWorkdir => Err(anyhow::anyhow!("no workdir found for repository")),
            AddProjectOutcome::NoDotGitDirectory => {
                Err(anyhow::anyhow!("no .git directory found in repository"))
            }
            AddProjectOutcome::NotAGitRepository(msg) => {
                Err(anyhow::anyhow!("not a git repository: {}", msg))
            }
        }
    }
}
