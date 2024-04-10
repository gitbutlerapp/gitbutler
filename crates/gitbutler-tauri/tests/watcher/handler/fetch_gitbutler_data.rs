use std::time::SystemTime;

use gitbutler_core::projects;
use gitbutler_tauri::watcher::Handler;
use pretty_assertions::assert_eq;

use crate::watcher::handler::test_remote_repository;
use gitbutler_testsupport::{Case, Suite};

#[tokio::test]
async fn fetch_success() -> anyhow::Result<()> {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let (cloud, _tmp) = test_remote_repository()?;
    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: cloud.path().to_str().unwrap().to_string(),
        code_git_url: None,
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

    Handler::fetch_gb_data_pure(
        suite.local_app_data(),
        &suite.projects,
        &suite.users,
        project.id,
        SystemTime::now(),
    )
    .await
    .unwrap();

    Ok(())
}

#[tokio::test]
async fn fetch_fail_no_sync() {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let res = Handler::fetch_gb_data_pure(
        suite.local_app_data(),
        &suite.projects,
        &suite.users,
        project.id,
        SystemTime::now(),
    )
    .await;

    assert_eq!(&res.unwrap_err().to_string(), "sync disabled");
}
