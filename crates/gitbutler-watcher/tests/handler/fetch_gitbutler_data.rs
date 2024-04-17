use std::time::SystemTime;

use gitbutler_core::projects;

use crate::handler::support::Fixture;
use crate::handler::test_remote_repository;
use gitbutler_testsupport::Case;

#[tokio::test]
async fn fetch_success() -> anyhow::Result<()> {
    let mut fixture = Fixture::default();
    {
        let handler = fixture.new_handler();
        let Case { project, .. } = &fixture.new_case();
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

        fixture
            .projects
            .update(&projects::UpdateRequest {
                id: project.id,
                api: Some(api_project.clone()),
                ..Default::default()
            })
            .await?;

        handler
            .fetch_gb_data(project.id, SystemTime::now())
            .await
            .unwrap();
    }
    assert_eq!(fixture.events().len(), 0);
    Ok(())
}

#[tokio::test]
async fn fetch_fail_no_sync() {
    let mut fixture = Fixture::default();
    {
        let handler = fixture.new_handler();
        let Case { project, .. } = &fixture.new_case();
        let res = handler.fetch_gb_data(project.id, SystemTime::now()).await;

        assert_eq!(&res.unwrap_err().to_string(), "sync disabled");
    }
    assert_eq!(fixture.events().len(), 0);
}
