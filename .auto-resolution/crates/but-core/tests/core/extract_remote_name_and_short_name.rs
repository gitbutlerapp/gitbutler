use but_core::extract_remote_name_and_short_name;
use but_testsupport::{read_only_in_memory_scenario, visualize_commit_graph_all};

#[test]
fn journey() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("multiple-remotes-with-tracking-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, 
            @"
    * 14f7926 (nested/remote-b/main, nested/remote-b/in-nested-remote-b, nested/remote-b/HEAD) init-c
    * 5efb67f (nested/remote/main, nested/remote/in-nested-remote, nested/remote/HEAD) init-b
    * 263500f (origin/normal-remote, origin/main, origin/HEAD) init-a
    ");

    let remote_names = repo.remote_names();
    let rn = "refs/remotes/origin/feature".try_into()?;
    let (remote_name, short_name) = extract_remote_name_and_short_name(rn, &remote_names).unwrap();
    assert_eq!(
        remote_name, "origin",
        "a normal remote can always be extracted"
    );
    assert_eq!(short_name, "feature");

    let rn = "refs/remotes/nested/non-existing/feature".try_into()?;
    assert_eq!(
        extract_remote_name_and_short_name(rn, &remote_names),
        None,
        "unregistered remotes aren't handled at all due to ambiguity when there are more than one slashes"
    );

    let rn = "refs/remotes/non-existing/feature".try_into()?;
    let (remote_name, short_name) = extract_remote_name_and_short_name(rn, &remote_names).unwrap();
    assert_eq!(
        remote_name, "non-existing",
        "here we know for sure, there is always remote/branch at least"
    );
    assert_eq!(short_name, "feature");

    let rn = "refs/remotes/nested/remote/feature/a".try_into()?;
    let (remote_name, short_name) = extract_remote_name_and_short_name(rn, &remote_names).unwrap();
    assert_eq!(
        remote_name, "nested/remote",
        "this works because we know remote names"
    );
    assert_eq!(short_name, "feature/a");

    let rn = "refs/remotes/nested/remote-b/feature/b".try_into()?;
    let (remote_name, short_name) = extract_remote_name_and_short_name(rn, &remote_names).unwrap();
    assert_eq!(remote_name, "nested/remote-b");
    assert_eq!(short_name, "feature/b");

    Ok(())
}
