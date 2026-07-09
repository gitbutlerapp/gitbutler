use but_core::extract_short_branch_name;

#[test]
fn remote_tracking_branch() {
    let ref_name: gix::refs::FullNameRef = "refs/remotes/origin/main".try_into().unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        Some("main".to_string()),
        "should extract branch name from simple remote tracking branch"
    );

    let ref_name: gix::refs::FullNameRef = "refs/remotes/origin/feature/cool-feature"
        .try_into()
        .unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        Some("feature/cool-feature".to_string()),
        "should extract branch name with slashes"
    );

    let ref_name: gix::refs::FullNameRef = "refs/remotes/upstream/develop".try_into().unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        Some("develop".to_string()),
        "should work with different remote names"
    );
}

#[test]
fn local_branch() {
    let ref_name: gix::refs::FullNameRef = "refs/heads/main".try_into().unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        Some("main".to_string()),
        "should extract name from local branch"
    );

    let ref_name: gix::refs::FullNameRef = "refs/heads/feature/new-feature"
        .try_into()
        .unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        Some("feature/new-feature".to_string()),
        "should extract name from local branch with slashes"
    );
}

#[test]
fn unsupported_refs() {
    let ref_name: gix::refs::FullNameRef = "refs/tags/v1.0.0".try_into().unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        None,
        "should return None for tags"
    );

    let ref_name: gix::refs::FullNameRef = "refs/notes/commits".try_into().unwrap();
    assert_eq!(
        extract_short_branch_name(&ref_name),
        None,
        "should return None for notes refs"
    );
}
