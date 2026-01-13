use but_testsupport::gix_testtools;

#[test]
fn is_conflicted() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let normal = but_core::Commit::from_id(repo.rev_parse_single("normal")?)?;
    assert!(!normal.is_conflicted());

    let conflicted = but_core::Commit::from_id(repo.rev_parse_single("conflicted")?)?;
    assert!(conflicted.is_conflicted());
    Ok(())
}

pub fn conflict_repo(name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("conflict-commits.sh")
        .map_err(anyhow::Error::from_boxed)?;
    Ok(gix::open_opts(
        root.join(name),
        gix::open::Options::isolated(),
    )?)
}

mod headers {
    mod try_from_commit {
        use but_core::commit::Headers;
        use gix::actor::Signature;

        #[test]
        fn without_any_header_information() {
            let commit = commit_with_header(None);
            assert_eq!(Headers::try_from_commit(&commit), None);
        }

        #[test]
        fn with_uuid_change_id() {
            let commit = commit_with_header([(
                "gitbutler-change-id",
                "96420be4-a3ed-4bea-b534-ff2160cbb848",
            )]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("old change ids parse for compatibility"), @r#"
            Headers {
                change_id: Some(
                    "96420be4-a3ed-4bea-b534-ff2160cbb848",
                ),
                conflicted: None,
            }
            "#);
        }

        #[test]
        fn with_reverse_hex_change_id() {
            let commit = commit_with_header([("change-id", "zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx")]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("reverse hex is parsed as well"), @r#"
            Headers {
                change_id: Some(
                    "zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx",
                ),
                conflicted: None,
            }
            "#);
        }

        #[test]
        fn with_conflict_header() {
            let commit = commit_with_header([("gitbutler-conflicted", "128")]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("a single conflict header is enough to parse as header"), @"
            Headers {
                change_id: None,
                conflicted: Some(
                    128,
                ),
            }
            ");
        }

        #[test]
        fn with_conflict_header_and_reverse_hex_change_id() {
            let commit = commit_with_header([
                ("change-id", "zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx"),
                ("gitbutler-conflicted", "128"),
            ]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("both fields are parsed"), @r#"
            Headers {
                change_id: Some(
                    "zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx",
                ),
                conflicted: Some(
                    128,
                ),
            }
            "#);
        }

        #[test]
        fn with_arbitrary_change_id_in_new_change_id_field() {
            let commit =
                commit_with_header([("change-id", "something-special-that-we-dont-produce")]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("the arbitrary change id is kept in the new field"), @r#"
            Headers {
                change_id: Some(
                    "something-special-that-we-dont-produce",
                ),
                conflicted: None,
            }
            "#);
        }

        #[test]
        fn with_arbitrary_change_id_in_old_change_id_field() {
            let commit = commit_with_header([(
                "gitbutler-change-id",
                "something-special-that-we-dont-produce",
            )]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("the arbitrary change id is kept in the old field"), @r#"
            Headers {
                change_id: Some(
                    "something-special-that-we-dont-produce",
                ),
                conflicted: None,
            }
            "#);
        }

        #[test]
        fn with_old_and_new_change_id_field() {
            let commit = commit_with_header([
                ("change-id", "the new change-id takes precedence"),
                (
                    "gitbutler-change-id",
                    "this one isn't used and it's fine that it's invalid",
                ),
            ]);
            insta::assert_debug_snapshot!(Headers::try_from_commit(&commit).expect("the new change-id field takes precedence"), @r#"
            Headers {
                change_id: Some(
                    "the new change-id takes precedence",
                ),
                conflicted: None,
            }
            "#);
        }

        fn commit_with_header(
            headers: impl IntoIterator<Item = (&'static str, &'static str)>,
        ) -> gix::objs::Commit {
            gix::objs::Commit {
                tree: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
                parents: vec![].into(),
                author: Signature::default(),
                committer: Signature::default(),
                encoding: None,
                message: b"".into(),
                extra_headers: headers
                    .into_iter()
                    .map(|(a, b)| (a.into(), b.into()))
                    .collect(),
            }
        }
    }
}
