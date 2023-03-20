use crate::{projects, sessions};
use anyhow::{Context, Result};

pub struct Store {
    project: projects::Project,
    git_repository: git2::Repository,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            git_repository: git2::Repository::open(&self.project.path).unwrap(),
        }
    }
}

impl Store {
    pub fn new(git_repository: git2::Repository, project: projects::Project) -> Result<Self> {
        Ok(Self {
            project: project.clone(),
            git_repository,
        })
    }

    pub fn get(&self) -> Result<Option<sessions::Session>> {
        let session_path = self.project.session_path();
        let meta_path = session_path.join("meta");
        if !meta_path.exists() {
            return Ok(None);
        }

        let start_path = meta_path.join("start");
        let start_ts = std::fs::read_to_string(start_path.clone())?
            .parse::<u128>()
            .with_context(|| {
                format!(
                    "failed to parse start timestamp from {}",
                    start_path.display()
                )
            })?;

        let last_path = meta_path.join("last");
        let last_ts = std::fs::read_to_string(last_path.clone())?
            .parse::<u128>()
            .with_context(|| {
                format!(
                    "failed to parse last timestamp from {}",
                    last_path.display()
                )
            })?;

        let branch_path = meta_path.join("branch");
        let branch = match branch_path.exists() {
            true => std::fs::read_to_string(branch_path.clone())
                .with_context(|| {
                    format!("failed to read branch name from {}", branch_path.display())
                })?
                .into(),
            false => None,
        };

        let commit_path = meta_path.join("commit");
        let commit = match commit_path.exists() {
            true => std::fs::read_to_string(commit_path.clone())
                .with_context(|| {
                    format!("failed to read commit hash from {}", commit_path.display())
                })?
                .into(),
            false => None,
        };

        let activity_path = self.git_repository.path().join("logs/HEAD");
        let activity = match activity_path.exists() {
            true => std::fs::read_to_string(activity_path)
                .with_context(|| {
                    format!(
                        "failed to read reflog from {}",
                        self.git_repository.path().join("logs/HEAD").display()
                    )
                })?
                .lines()
                .filter_map(|line| sessions::activity::parse_reflog_line(line).ok())
                .filter(|activity| activity.timestamp_ms >= start_ts)
                .collect::<Vec<sessions::activity::Activity>>(),
            false => Vec::new(),
        };

        let id_path = meta_path.join("id");
        let id = std::fs::read_to_string(id_path.clone())
            .with_context(|| format!("failed to read session id from {}", id_path.display()))?;

        Ok(Some(sessions::Session {
            id,
            hash: None,
            activity,
            meta: sessions::Meta {
                start_timestamp_ms: start_ts,
                last_timestamp_ms: last_ts,
                branch,
                commit,
            },
        }))
    }
}
