use std::{path, sync::Arc, time};

use anyhow::{Context, Result};
use itertools::Itertools;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{
    gb_repository,
    git::{self, Oid, Repository},
    project_repository,
    projects::{self, CodePushState, ProjectId},
    users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<HandlerInner>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let projects = projects::Controller::try_from(value)?;
            let users = users::Controller::try_from(value)?;
            let inner = HandlerInner::new(app_data_dir, projects, users);
            let handler = Handler::new(inner);
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

impl Handler {
    fn new(inner: HandlerInner) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.handle(project_id).await
        } else {
            Ok(vec![])
        }
    }
}

pub struct HandlerInner {
    local_data_dir: path::PathBuf,
    project_store: projects::Controller,
    users: users::Controller,
    batch_size: usize,
}

impl HandlerInner {
    fn new(
        local_data_dir: path::PathBuf,
        project_store: projects::Controller,
        users: users::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            project_store,
            users,
            batch_size: 1000,
        }
    }

    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        if !project.is_sync_enabled() || !project.has_code_url() {
            return Ok(vec![]);
        }

        let user = self.users.get_user()?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;

        let gb_code_last_commit = project
            .gitbutler_code_push_state
            .as_ref()
            .map(|state| &state.id)
            .copied();

        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )?;
        let default_target = gb_repository
            .default_target()
            .context("failed to open gb repo")?
            .context("failed to get default target")?;

        let target_changed = !gb_code_last_commit
            .map(|id| id == default_target.sha)
            .unwrap_or_default();

        if target_changed {
            match self
                .push_target(
                    &project_repository,
                    &default_target,
                    gb_code_last_commit,
                    project_id,
                    &user,
                )
                .await
            {
                Ok(()) => {}
                Err(project_repository::RemoteError::Network) => return Ok(vec![]),
                Err(err) => return Err(err).context("failed to push"),
            };
        }

        match push_all_refs(&project_repository, &user, project_id) {
            Ok(()) => {}
            Err(project_repository::RemoteError::Network) => return Ok(vec![]),
            Err(err) => return Err(err).context("failed to push"),
        };

        // make sure last push time is updated
        self.update_project(project_id, &default_target.sha).await?;

        Ok(vec![])
    }

    async fn push_target(
        &self,
        project_repository: &project_repository::Repository,
        default_target: &crate::virtual_branches::target::Target,
        gb_code_last_commit: Option<Oid>,
        project_id: &crate::id::Id<projects::Project>,
        user: &Option<users::User>,
    ) -> Result<(), project_repository::RemoteError> {
        let ids = batch_rev_walk(
            &project_repository.git_repository,
            self.batch_size,
            default_target.sha,
            gb_code_last_commit,
        )?;

        tracing::info!(
            %project_id,
            batches=%ids.len(),
            "batches left to push",
        );

        let id_count = &ids.len();

        for (idx, id) in ids.iter().enumerate().rev() {
            let refspec = format!("+{}:refs/push-tmp/{}", id, project_id);

            project_repository.push_to_gitbutler_server(user.as_ref(), &[&refspec])?;

            self.update_project(project_id, id).await?;

            tracing::info!(
                %project_id,
                i = id_count.saturating_sub(idx),
                total = id_count,
                "project batch pushed",
            );
        }

        project_repository.push_to_gitbutler_server(
            user.as_ref(),
            &[&format!("+{}:refs/{}", default_target.sha, project_id)],
        )?;

        //TODO: remove push-tmp ref

        tracing::info!(
            %project_id,
            "project target ref fully pushed",
        );

        Ok(())
    }

    async fn update_project(
        &self,
        project_id: &crate::id::Id<projects::Project>,
        id: &Oid,
    ) -> Result<(), project_repository::RemoteError> {
        self.project_store
            .update(&projects::UpdateRequest {
                id: *project_id,
                gitbutler_code_push_state: Some(CodePushState {
                    id: *id,
                    timestamp: time::SystemTime::now(),
                }),
                ..Default::default()
            })
            .await
            .context("failed to update last push")?;

        Ok(())
    }
}

fn push_all_refs(
    project_repository: &project_repository::Repository,
    user: &Option<users::User>,
    project_id: &crate::id::Id<projects::Project>,
) -> Result<(), project_repository::RemoteError> {
    let gb_references = collect_refs(project_repository)?;

    let all_refs = gb_references
        .iter()
        .filter(|r| {
            matches!(
                r,
                git::Refname::Remote(_) | git::Refname::Virtual(_) | git::Refname::Local(_)
            )
        })
        .map(|r| format!("+{}:{}", r, r))
        .collect::<Vec<_>>();

    let all_refs = all_refs.iter().map(String::as_str).collect::<Vec<_>>();

    let anything_pushed =
        project_repository.push_to_gitbutler_server(user.as_ref(), all_refs.as_slice())?;

    if anything_pushed {
        tracing::info!(
            %project_id,
            "refs pushed",
        );
    }

    Ok(())
}

fn collect_refs(
    project_repository: &project_repository::Repository,
) -> anyhow::Result<Vec<git::Refname>> {
    Ok(project_repository
        .git_repository
        .references_glob("refs/*")?
        .flatten()
        .filter_map(|r| r.name())
        .collect::<Vec<_>>())
}

fn batch_rev_walk(
    repo: &Repository,
    batch_size: usize,
    from: Oid,
    until: Option<Oid>,
) -> Result<Vec<Oid>> {
    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk
        .push(from.into())
        .context(format!("failed to push {}", from))?;

    if let Some(oid) = until {
        revwalk
            .hide(oid.into())
            .context(format!("failed to hide {}", oid))?;
    }
    let mut oids = Vec::new();
    oids.push(from);
    for batch in &revwalk.chunks(batch_size) {
        if let Some(oid) = batch.last() {
            let oid = oid.context("failed to get oid")?;
            if oid != from.into() {
                oids.push(oid.into());
            }
        }
    }
    Ok(oids)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::project_repository::LogUntil;
    use crate::tests::{Case, Suite};
    use crate::virtual_branches::set_test_target;

    use super::super::test_remote_repository;
    use super::*;

    fn log_walk(repo: &git2::Repository, head: git::Oid) -> Vec<git::Oid> {
        let mut walker = repo.revwalk().unwrap();
        walker.push(head.into()).unwrap();
        walker.map(|oid| oid.unwrap().into()).collect::<Vec<_>>()
    }

    #[tokio::test]
    async fn test_push_error() -> Result<()> {
        let suite = Suite::default();
        let Case { project, .. } = suite.new_case();

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: String::new(),
            code_git_url: Some(String::new()),
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        let listener = HandlerInner {
            local_data_dir: suite.local_app_data,
            project_store: suite.projects,
            users: suite.users,
            batch_size: 100,
        };

        let res = listener.handle(&project.id).await;

        res.unwrap_err();

        Ok(())
    }

    #[tokio::test]
    async fn test_push_simple() -> Result<()> {
        let suite = Suite::default();
        let Case {
            project,
            gb_repository,
            project_repository,
            ..
        } = suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

        suite.sign_in();

        set_test_target(&gb_repository, &project_repository).unwrap();

        let target_id = gb_repository.default_target().unwrap().unwrap().sha;

        let reference = project_repository.l(target_id, LogUntil::End).unwrap();

        let cloud_code = test_remote_repository()?;

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: String::new(),
            code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        cloud_code.find_commit(target_id.into()).unwrap_err();

        {
            let listener = HandlerInner {
                local_data_dir: suite.local_app_data,
                project_store: suite.projects.clone(),
                users: suite.users,
                batch_size: 10,
            };

            let res = listener.handle(&project.id).await.unwrap();
            assert!(res.is_empty());
        }

        cloud_code.find_commit(target_id.into()).unwrap();

        let pushed = log_walk(&cloud_code, target_id);
        assert_eq!(reference.len(), pushed.len());
        assert_eq!(reference, pushed);

        assert_eq!(
            suite
                .projects
                .get(&project.id)
                .unwrap()
                .gitbutler_code_push_state
                .unwrap()
                .id,
            target_id
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_push_remote_ref() -> Result<()> {
        let suite = Suite::default();
        let Case {
            project,
            gb_repository,
            project_repository,
            ..
        } = suite.new_case();

        suite.sign_in();

        set_test_target(&gb_repository, &project_repository).unwrap();

        let cloud_code: git::Repository = test_remote_repository()?.into();

        let remote_repo: git::Repository = test_remote_repository()?.into();

        let last_commit = create_initial_commit(&remote_repo);

        remote_repo
            .reference(
                &git::Refname::Local(git::LocalRefname::new("refs/heads/testbranch", None)),
                last_commit,
                false,
                "",
            )
            .unwrap();

        let mut remote = project_repository
            .git_repository
            .remote("tr", &remote_repo.path().to_str().unwrap().parse().unwrap())
            .unwrap();

        remote
            .fetch(&["+refs/heads/*:refs/remotes/tr/*"], None)
            .unwrap();

        project_repository
            .git_repository
            .find_commit(last_commit)
            .unwrap();

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: String::new(),
            code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        {
            let listener = HandlerInner {
                local_data_dir: suite.local_app_data,
                project_store: suite.projects.clone(),
                users: suite.users,
                batch_size: 10,
            };

            listener.handle(&project.id).await.unwrap();
        }

        cloud_code.find_commit(last_commit).unwrap();

        Ok(())
    }

    fn create_initial_commit(repo: &git::Repository) -> git::Oid {
        let signature = git::Signature::now("test", "test@email.com").unwrap();

        let mut index = repo.index().unwrap();
        let oid = index.write_tree().unwrap();

        repo.commit(
            None,
            &signature,
            &signature,
            "initial commit",
            &repo.find_tree(oid).unwrap(),
            &[],
        )
        .unwrap()
    }

    fn create_test_commits(repo: &git::Repository, commits: usize) -> git::Oid {
        let signature = git::Signature::now("test", "test@email.com").unwrap();

        let mut last = None;

        for i in 0..commits {
            let mut index = repo.index().unwrap();
            let oid = index.write_tree().unwrap();
            let head = repo.head().unwrap();

            last = Some(
                repo.commit(
                    Some(&head.name().unwrap()),
                    &signature,
                    &signature,
                    format!("commit {i}").as_str(),
                    &repo.find_tree(oid).unwrap(),
                    &[&repo
                        .find_commit(repo.refname_to_id("HEAD").unwrap())
                        .unwrap()],
                )
                .unwrap(),
            );
        }

        last.unwrap()
    }

    #[tokio::test]
    async fn test_push_batches() -> Result<()> {
        let suite = Suite::default();
        let Case {
            project,
            gb_repository,
            project_repository,
            ..
        } = suite.new_case();

        suite.sign_in();

        {
            let head: git::Oid = project_repository
                .get_head()
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .id();

            let reference = project_repository.l(head, LogUntil::End).unwrap();
            assert_eq!(reference.len(), 2);

            let head = create_test_commits(&project_repository.git_repository, 10);

            let reference = project_repository.l(head, LogUntil::End).unwrap();
            assert_eq!(reference.len(), 12);
        }

        set_test_target(&gb_repository, &project_repository).unwrap();

        let target_id = gb_repository.default_target().unwrap().unwrap().sha;

        let reference = project_repository.l(target_id, LogUntil::End).unwrap();

        let cloud_code = test_remote_repository()?;

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: String::new(),
            code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        {
            let listener = HandlerInner {
                local_data_dir: suite.local_app_data.clone(),
                project_store: suite.projects.clone(),
                users: suite.users.clone(),
                batch_size: 2,
            };

            listener.handle(&project.id).await.unwrap();
        }

        cloud_code.find_commit(target_id.into()).unwrap();

        let pushed = log_walk(&cloud_code, target_id);
        assert_eq!(reference.len(), pushed.len());
        assert_eq!(reference, pushed);

        assert_eq!(
            suite
                .projects
                .get(&project.id)
                .unwrap()
                .gitbutler_code_push_state
                .unwrap()
                .id,
            target_id
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_push_again_no_change() -> Result<()> {
        let suite = Suite::default();
        let Case {
            project,
            gb_repository,
            project_repository,
            ..
        } = suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

        suite.sign_in();

        set_test_target(&gb_repository, &project_repository).unwrap();

        let target_id = gb_repository.default_target().unwrap().unwrap().sha;

        let reference = project_repository.l(target_id, LogUntil::End).unwrap();

        let cloud_code = test_remote_repository()?;

        let api_project = projects::ApiProject {
            name: "test-sync".to_string(),
            description: None,
            repository_id: "123".to_string(),
            git_url: String::new(),
            code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
            created_at: 0_i32.to_string(),
            updated_at: 0_i32.to_string(),
            sync: true,
        };

        suite
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        cloud_code.find_commit(target_id.into()).unwrap_err();

        {
            let listener = HandlerInner {
                local_data_dir: suite.local_app_data,
                project_store: suite.projects.clone(),
                users: suite.users,
                batch_size: 10,
            };

            let res = listener.handle(&project.id).await.unwrap();
            assert!(res.is_empty());
        }

        cloud_code.find_commit(target_id.into()).unwrap();

        let pushed = log_walk(&cloud_code, target_id);
        assert_eq!(reference.len(), pushed.len());
        assert_eq!(reference, pushed);

        assert_eq!(
            suite
                .projects
                .get(&project.id)
                .unwrap()
                .gitbutler_code_push_state
                .unwrap()
                .id,
            target_id
        );

        Ok(())
    }
}
