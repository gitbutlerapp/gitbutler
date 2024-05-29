use super::*;

#[tokio::test]
async fn should_update_last_fetched() {
    let Test {
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let before_fetch = controller.get_base_branch_data(*project_id).await.unwrap();
    assert!(before_fetch.last_fetched_ms.is_none());

    let fetch = controller
        .fetch_from_remotes(*project_id, None)
        .await
        .unwrap();
    assert!(fetch.last_fetched_ms.is_some());

    let after_fetch = controller.get_base_branch_data(*project_id).await.unwrap();
    assert!(after_fetch.last_fetched_ms.is_some());
    assert_eq!(fetch.last_fetched_ms, after_fetch.last_fetched_ms);

    let second_fetch = controller
        .fetch_from_remotes(*project_id, None)
        .await
        .unwrap();
    assert!(second_fetch.last_fetched_ms.is_some());
    assert_ne!(fetch.last_fetched_ms, second_fetch.last_fetched_ms);

    let after_second_fetch = controller.get_base_branch_data(*project_id).await.unwrap();
    assert!(after_second_fetch.last_fetched_ms.is_some());
    assert_eq!(
        second_fetch.last_fetched_ms,
        after_second_fetch.last_fetched_ms
    );
}
