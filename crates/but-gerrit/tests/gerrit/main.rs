use but_gerrit::{parse::PushOutput, record_push_metadata};
use snapbox::str;

#[test]
fn record_push_metadata_fallback_url() -> anyhow::Result<()> {
    let (repo, _tmp_writable_for_database) =
        but_testsupport::writable_scenario("one-commit-with-gerrit-remote");
    snapbox::assert_data_eq!(
        but_testsupport::visualize_commit_graph_all(&repo)?,
        str![[r#"
* 7923fae (HEAD -> main) commit with gitbutler change-id

"#]]
    );

    let repo = gix::open(repo.path())?;
    let commit_id = but_testsupport::id_by_rev(&repo, "7923fae");
    let candidate_ids = vec![commit_id.detach()];
    let push_output = PushOutput {
        success: true,
        warnings: vec![],
        changes: vec![], // Empty changes to trigger fallback
        processing_info: None,
    };

    let change_id = but_core::Commit::from_id(commit_id)?
        .headers()
        .expect("gb header are set")
        .change_id
        .expect("commit has change id");
    let mut ctx = but_ctx::Context::from_repo(repo)?;
    record_push_metadata(&mut ctx, candidate_ids, push_output)?;

    let db = ctx.db.get_mut()?;
    let db = db.gerrit_metadata();
    let meta = db
        .get(&change_id.to_string())?
        .expect("Metadata should be recorded");

    snapbox::assert_data_eq!(
        format!("{:#?}", meta),
        str![[r#"
GerritMeta {
    change_id: "00000000-0000-0000-0000-000000000001",
    commit_id: "7923faec4760ee74d7ad794892766d1b9b00ca96",
    review_url: "https://gerrithost/q/I10c56efd90c998f406d4e0b99d9c58feeaf896c5",
...
}
"#]]
    );

    Ok(())
}
