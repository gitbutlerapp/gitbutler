mod extract_login_shell_environment {
    use but_core::cmd::extract_interactive_login_shell_environment;

    #[test]
    fn no_panic() {
        // cannot expect anything, except that invoking it shouldn't panic.
        let res = extract_interactive_login_shell_environment();
        if let Some(vars) = res {
            assert_ne!(
                vars.len(),
                0,
                "There is never 'no' variables, or else there is `None`"
            );
        }
    }
}

mod headers_parsing {
    use but_core::commit::Headers;
    use gix::actor::Signature;

    fn default_commit() -> gix::objs::Commit {
        gix::objs::Commit {
            tree: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            parents: vec![].into(),
            author: Signature::default(),
            committer: Signature::default(),
            encoding: None,
            message: b"".into(),
            extra_headers: vec![],
        }
    }

    #[test]
    fn if_no_extra_headers_none_is_returned() {
        let commit = default_commit();
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @"None");
    }

    #[test]
    fn if_a_old_change_id_is_provided_its_returned() {
        let mut commit = default_commit();
        commit.extra_headers = vec![(
            b"gitbutler-change-id".into(),
            b"96420be4-a3ed-4bea-b534-ff2160cbb848".into(),
        )];
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @r#"
        Some(
            Headers {
                change_id: Some(
                    "96420be4-a3ed-4bea-b534-ff2160cbb848",
                ),
                conflicted: None,
            },
        )
        "#);
    }

    #[test]
    fn if_a_new_change_id_is_provided_its_returned() {
        let mut commit = default_commit();
        commit.extra_headers = vec![(
            b"change-id".into(),
            b"zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx".into(),
        )];
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @r#"
        Some(
            Headers {
                change_id: Some(
                    "zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx",
                ),
                conflicted: None,
            },
        )
        "#);
    }

    #[test]
    fn if_a_conflict_header_is_provided_its_returned() {
        let mut commit = default_commit();
        commit.extra_headers = vec![(b"gitbutler-conflicted".into(), b"128".into())];
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @r"
        Some(
            Headers {
                change_id: None,
                conflicted: Some(
                    128,
                ),
            },
        )
        ");
    }

    #[test]
    fn both_can_be_provided() {
        let mut commit = default_commit();
        commit.extra_headers = vec![
            (
                b"change-id".into(),
                b"zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx".into(),
            ),
            (b"gitbutler-conflicted".into(), b"128".into()),
        ];
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @r#"
        Some(
            Headers {
                change_id: Some(
                    "zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx",
                ),
                conflicted: Some(
                    128,
                ),
            },
        )
        "#);
    }

    #[test]
    fn change_id_can_be_arbitrary_data() {
        let mut commit = default_commit();
        commit.extra_headers = vec![(b"change-id".into(), b"If a user can, a user will...".into())];
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @r#"
        Some(
            Headers {
                change_id: Some(
                    "If a user can, a user will...",
                ),
                conflicted: None,
            },
        )
        "#);
    }

    #[test]
    fn new_change_id_takes_precidence_over_old() {
        let mut commit = default_commit();
        commit.extra_headers = vec![
            (b"change-id".into(), b"If a user can, a user will...".into()),
            (
                b"gitbutler-change-id".into(),
                b"96420be4-a3ed-4bea-b534-ff2160cbb848".into(),
            ),
        ];
        insta::assert_debug_snapshot!(Headers::try_from_commit(&commit), @r#"
        Some(
            Headers {
                change_id: Some(
                    "If a user can, a user will...",
                ),
                conflicted: None,
            },
        )
        "#);
    }
}
