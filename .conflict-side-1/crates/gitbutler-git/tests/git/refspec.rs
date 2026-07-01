use gitbutler_git::RefSpec;

#[test]
fn parse_source_dest() {
    assert_eq!(
        RefSpec::parse("refs/heads/*:refs/remotes/origin/*").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/remotes/origin/*".to_owned()),
        }
    );
}

#[test]
fn parse_source_dest_force() {
    assert_eq!(
        RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap(),
        RefSpec {
            update_non_fastforward: true,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/remotes/origin/*".to_owned()),
        }
    );
}

#[test]
fn parse_single_colon() {
    assert_eq!(
        RefSpec::parse(":").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: None,
            destination: None,
        }
    );
}

#[test]
fn parse_single_colon_force() {
    assert_eq!(
        RefSpec::parse("+:").unwrap(),
        RefSpec {
            update_non_fastforward: true,
            source: None,
            destination: None,
        }
    );
}

#[test]
fn parse_empty() {
    assert_eq!(
        RefSpec::parse("").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: None,
            destination: None,
        }
    );
}

#[test]
fn parse_empty_force() {
    assert_eq!(
        RefSpec::parse("+").unwrap(),
        RefSpec {
            update_non_fastforward: true,
            source: None,
            destination: None,
        }
    );
}

#[test]
fn parse_single() {
    assert_eq!(
        RefSpec::parse("refs/heads/*").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/heads/*".to_owned()),
        }
    );
}

#[test]
fn parse_delete() {
    assert_eq!(
        RefSpec::parse(":refs/heads/experimental").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: None,
            destination: Some("refs/heads/experimental".to_owned()),
        }
    );
}

#[test]
fn parse_single_force() {
    assert_eq!(
        RefSpec::parse("+refs/heads/*").unwrap(),
        RefSpec {
            update_non_fastforward: true,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/heads/*".to_owned()),
        }
    );
}

#[test]
fn parse_delete_force() {
    assert_eq!(
        RefSpec::parse("+:refs/heads/experimental").unwrap(),
        RefSpec {
            update_non_fastforward: true,
            source: None,
            destination: Some("refs/heads/experimental".to_owned()),
        }
    );
}

#[test]
fn parse_name() {
    assert_eq!(
        RefSpec::parse("master").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: Some("master".to_owned()),
            destination: Some("master".to_owned()),
        }
    );
}

#[test]
fn parse_name_force() {
    assert_eq!(
        RefSpec::parse("+master").unwrap(),
        RefSpec {
            update_non_fastforward: true,
            source: Some("master".to_owned()),
            destination: Some("master".to_owned()),
        }
    );
}

#[test]
fn parse_source_only() {
    assert_eq!(
        RefSpec::parse("refs/heads/*:").unwrap(),
        RefSpec {
            update_non_fastforward: false,
            source: Some("refs/heads/*".to_owned()),
            destination: None,
        }
    );
}

#[test]
fn format_empty() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: false,
            source: None,
            destination: None,
        }
        .to_string(),
        ":".to_owned()
    );
}

#[test]
fn format_empty_force() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: true,
            source: None,
            destination: None,
        }
        .to_string(),
        "+:".to_owned()
    );
}

#[test]
fn format_source_only() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: false,
            source: Some("refs/heads/*".to_owned()),
            destination: None,
        }
        .to_string(),
        "refs/heads/*:".to_owned()
    );
}

#[test]
fn format_source_only_force() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: true,
            source: Some("refs/heads/*".to_owned()),
            destination: None,
        }
        .to_string(),
        "+refs/heads/*:".to_owned()
    );
}

#[test]
fn format_source_dest() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: false,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/remotes/origin/*".to_owned()),
        }
        .to_string(),
        "refs/heads/*:refs/remotes/origin/*".to_owned()
    );
}

#[test]
fn format_source_dest_force() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: true,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/remotes/origin/*".to_owned()),
        }
        .to_string(),
        "+refs/heads/*:refs/remotes/origin/*".to_owned()
    );
}

#[test]
fn format_dest_only() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: false,
            source: None,
            destination: Some("refs/heads/*".to_owned()),
        }
        .to_string(),
        ":refs/heads/*".to_owned()
    );
}

#[test]
fn format_dest_only_force() {
    assert_eq!(
        RefSpec {
            update_non_fastforward: true,
            source: None,
            destination: Some("refs/heads/*".to_owned()),
        }
        .to_string(),
        "+:refs/heads/*".to_owned()
    );
}

#[test]
fn tuple() {
    assert_eq!(
        RefSpec::from(("refs/heads/*", "refs/remotes/origin/*")),
        RefSpec {
            update_non_fastforward: false,
            source: Some("refs/heads/*".to_owned()),
            destination: Some("refs/remotes/origin/*".to_owned()),
        }
    );
}
