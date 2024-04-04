use std::time::SystemTime;

use gitbutler_core::projects;
use gitbutler_tauri::watcher::handlers::fetch_gitbutler_data::Handler;
use pretty_assertions::assert_eq;

use crate::{
    shared::{Case, Suite},
    watcher::handler::test_remote_repository,
};

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

    let listener = Handler::new(
        suite.local_app_data().into(),
        suite.projects.clone(),
        suite.users.clone(),
    );
    listener
        .handle(&project.id, &SystemTime::now())
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
async fn fetch_fail_no_sync() {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let listener = Handler::new(
        suite.local_app_data().into(),
        suite.projects.clone(),
        suite.users.clone(),
    );
    let res = listener.handle(&project.id, &SystemTime::now()).await;

    assert_eq!(&res.unwrap_err().to_string(), "sync disabled");
}
