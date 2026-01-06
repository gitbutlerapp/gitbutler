use but_ctx::Context;
use but_gerrit::{GerritChangeId, parse::PushOutput, record_push_metadata};
use but_testsupport::CommandExt;

#[test]
fn test_record_push_metadata_fallback_url() {
    let temp_dir = tempfile::tempdir().unwrap();
    let gix_repo = gix::init(temp_dir.path()).unwrap();
    let mut ctx = Context::from_repo(gix_repo.clone()).unwrap();

    // 1. Create a commit with GitButler headers
    let change_uuid = uuid::Uuid::new_v4();

    let tree_id = gix_repo
        .write_object(&gix::objs::Tree::empty())
        .unwrap()
        .detach();
    let author = gix::actor::Signature {
        name: "Author".into(),
        email: "author@example.com".into(),
        time: gix::date::Time::now_local_or_utc(),
    };
    let committer = author.clone();

    let commit_obj = gix::objs::Commit {
        tree: tree_id,
        parents: Default::default(),
        author,
        committer,
        encoding: None,
        message: "Test commit".into(),
        extra_headers: vec![
            (b"gitbutler-headers-version".into(), b"2".into()),
            (
                b"gitbutler-change-id".into(),
                change_uuid.to_string().into(),
            ),
        ],
    };
    let commit_id = gix_repo.write_object(&commit_obj).unwrap().detach();

    // 2. Set up a remote so we can derive the host
    but_testsupport::git(&gix_repo)
        .args(["remote", "add", "origin", "https://gerrithost/project"])
        .run();

    let gix_repo = gix::open(gix_repo.path()).unwrap();
    let candidate_ids = vec![commit_id];
    let push_output = PushOutput {
        success: true,
        warnings: vec![],
        changes: vec![], // Empty changes to trigger fallback
        processing_info: None,
    };

    record_push_metadata(&mut ctx, &gix_repo, candidate_ids, push_output).unwrap();

    let mut db = ctx.db.get_mut().unwrap();
    let mut db = db.gerrit_metadata();
    let meta = db
        .get(&change_uuid.to_string())
        .unwrap()
        .expect("Metadata should be recorded");

    let gerrit_change_id = GerritChangeId::from(change_uuid);
    assert_eq!(
        meta.review_url,
        format!("https://gerrithost/q/{}", gerrit_change_id)
    );
}
