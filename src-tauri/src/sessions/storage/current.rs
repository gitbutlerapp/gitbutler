use std::{
    sync::{Arc, Mutex},
    time,
};

use crate::{projects, sessions};
use anyhow::{Context, Result};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    project: projects::Project,
    git_repository: Arc<Mutex<git2::Repository>>,
}

impl Store {
    pub fn new(git_repository: Arc<Mutex<git2::Repository>>, project: projects::Project) -> Self {
        Self {
            project,
            git_repository,
        }
    }

    pub fn create(&self) -> Result<sessions::Session> {
        let git_repository = self.git_repository.lock().unwrap();

        let now_ts = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let activity = match std::fs::read_to_string(git_repository.path().join("logs/HEAD")) {
            Ok(reflog) => reflog
                .lines()
                .filter_map(|line| sessions::activity::parse_reflog_line(line).ok())
                .filter(|activity| activity.timestamp_ms >= now_ts)
                .collect::<Vec<sessions::activity::Activity>>(),
            Err(_) => Vec::new(),
        };

        let meta = match git_repository.head() {
            Ok(head) => sessions::Meta {
                start_timestamp_ms: now_ts,
                last_timestamp_ms: now_ts,
                branch: Some(head.name().unwrap().to_string()),
                commit: Some(head.peel_to_commit().unwrap().id().to_string()),
            },
            Err(_) => sessions::Meta {
                start_timestamp_ms: now_ts,
                last_timestamp_ms: now_ts,
                branch: None,
                commit: None,
            },
        };

        let session = sessions::Session {
            id: Uuid::new_v4().to_string(),
            hash: None,
            meta,
            activity,
        };

        let session_path = self.project.session_path();
        log::debug!("{}: Creating current session", session_path.display());
        let meta_path = session_path.join("meta");
        if meta_path.exists() {
            return Err(anyhow::anyhow!("session already exists"));
        }

        self.write(&session)?;
        Ok(session)
    }

    pub fn delete(&self) -> Result<()> {
        let session_path = self.project.session_path();
        log::debug!("{}: deleting current session", self.project.id);
        if session_path.exists() {
            std::fs::remove_dir_all(session_path)?;
        }
        Ok(())
    }

    pub fn update(&self, session: &sessions::Session) -> Result<()> {
        if session.hash.is_some() {
            return Err(anyhow::anyhow!("cannot update session that is not current"));
        }

        let session_path = self.project.session_path();
        log::debug!("{}: updating current session", self.project.id);
        if session_path.exists() {
            self.write(session)
        } else {
            Err(anyhow::anyhow!(
                "\"{}\" does not exist",
                session_path.display()
            ))
        }
    }

    fn write(&self, session: &sessions::Session) -> Result<()> {
        let session_path = self.project.session_path();
        if session.hash.is_some() {
            return Err(anyhow::anyhow!("cannot write session that is not current"));
        }

        let meta_path = session_path.join("meta");

        std::fs::create_dir_all(meta_path.clone()).with_context(|| {
            format!(
                "failed to create session meta directory {}",
                meta_path.display()
            )
        })?;

        let id_path = meta_path.join("id");
        std::fs::write(id_path.clone(), session.id.clone())
            .with_context(|| format!("failed to write session id to {}", id_path.display()))?;

        let start_path = meta_path.join("start");
        std::fs::write(
            start_path.clone(),
            session.meta.start_timestamp_ms.to_string(),
        )
        .with_context(|| {
            format!(
                "failed to write session start timestamp to {}",
                start_path.display()
            )
        })?;

        let last_path = meta_path.join("last");
        std::fs::write(
            last_path.clone(),
            session.meta.last_timestamp_ms.to_string(),
        )
        .with_context(|| {
            format!(
                "failed to write session last timestamp to {}",
                last_path.display()
            )
        })?;

        if let Some(branch) = session.meta.branch.clone() {
            let branch_path = meta_path.join("branch");
            std::fs::write(branch_path.clone(), branch).with_context(|| {
                format!(
                    "failed to write session branch to {}",
                    branch_path.display()
                )
            })?;
        }

        if let Some(commit) = session.meta.commit.clone() {
            let commit_path = meta_path.join("commit");
            std::fs::write(commit_path.clone(), commit).with_context(|| {
                format!(
                    "failed to write session commit to {}",
                    commit_path.display()
                )
            })?;
        }

        Ok(())
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

        let git_repository = self.git_repository.lock().unwrap();
        let activity_path = git_repository.path().join("logs/HEAD");
        let activity = match activity_path.exists() {
            true => std::fs::read_to_string(activity_path)
                .with_context(|| {
                    format!(
                        "failed to read reflog from {}",
                        git_repository.path().join("logs/HEAD").display()
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
