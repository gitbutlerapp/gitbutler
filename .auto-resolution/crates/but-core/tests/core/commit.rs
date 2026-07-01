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
    mod ensure_change_id {
        use but_core::commit::Headers;

        #[test]
        fn sets_change_id_from_commit_id_if_missing() {
            let commit_id = gix::ObjectId::from_hex(b"0123456789abcdef0123456789abcdef01234567")
                .expect("valid sha1 object id");

            let headers = Headers::default().ensure_change_id(commit_id);

            assert_eq!(
                headers.change_id.map(|id| id.to_string()),
                Some("ltpxnvrzksowmuqyltpxnvrzksowmuqy".to_owned())
            );
            assert_eq!(headers.conflicted, None);
        }

        #[test]
        fn keeps_existing_change_id() {
            let commit_id = gix::ObjectId::from_hex(b"0123456789abcdef0123456789abcdef01234567")
                .expect("valid sha1 object id");

            let headers = Headers {
                change_id: Some(but_core::ChangeId::from_number_for_testing(42)),
                conflicted: Some(3),
            }
            .ensure_change_id(commit_id);

            assert_eq!(
                headers.change_id.map(|id| id.to_string()),
                Some("42".to_owned())
            );
            assert_eq!(headers.conflicted, Some(3));
        }

        #[test]
        fn generates_random_change_id_if_commit_id_is_unknown() {
            let headers = Headers::default().ensure_change_id(None);

            let change_id = headers.change_id.expect("change-id should be generated");
            assert_eq!(change_id.len(), 32);
            assert_eq!(headers.conflicted, None);
        }
    }

    mod synthetic_change_id_from_commit_id {
        use but_core::commit::Headers;

        #[test]
        fn supports_sha1_commit_ids() {
            let commit_id = gix::ObjectId::from_hex(b"0123456789abcdef0123456789abcdef01234567")
                .expect("valid sha1 object id");

            assert_eq!(
                Headers::synthetic_change_id_from_commit_id(commit_id).to_string(),
                "ltpxnvrzksowmuqyltpxnvrzksowmuqy"
            );
        }
    }

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

mod create {
    use anyhow::Context as _;
    use bstr::ByteSlice;
    use but_core::commit::{self, SignCommit};
    use but_error::Code;
    use but_testsupport::{writable_scenario, writable_scenario_with_ssh_key};
    use gix::{objs::commit::SIGNATURE_FIELD_NAME, refs};

    #[test]
    fn signs_commits_when_enabled() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario_with_ssh_key("single-signed");

        let oid = commit::create(
            &repo,
            commit_from_head(&repo, "signed again")?,
            None,
            SignCommit::IfSignCommitsEnabled,
        )?;
        let commit = repo.find_commit(oid)?;
        let commit = commit.decode()?;

        assert!(
            commit.extra_headers().find(SIGNATURE_FIELD_NAME).is_some(),
            "a new signature should be written, the new commit needs to be signed"
        );
        Ok(())
    }

    #[test]
    fn sign_commit_yes_forcefully_signs_commit() -> anyhow::Result<()> {
        let (mut repo, _tmp) = writable_scenario_with_ssh_key("single-signed");
        repo.config_snapshot_mut()
            .set_raw_value("gitbutler.signCommits", "false")?;
        repo.config_snapshot_mut()
            .set_raw_value("commit.gpgSign", "false")?;

        let oid = commit::create(
            &repo,
            commit_from_head(&repo, "should sign")?,
            None,
            SignCommit::Yes,
        )?;
        let commit = repo.find_commit(oid)?;
        let commit = commit.decode()?;

        assert!(
            commit.extra_headers().pgp_signature().is_some(),
            "despite the settings saying everything is disabled, it still signs the commit"
        );
        Ok(())
    }

    #[test]
    fn sign_commit_yes_without_signing_key_errors() -> anyhow::Result<()> {
        let (mut repo, _tmp) = writable_scenario("single-unsigned");

        let err = commit::create(
            &repo,
            commit_from_head(&repo, "should not sign")?,
            None,
            SignCommit::Yes,
        )
        .expect_err("commit creation should fail as there is no signing key configured");

        let error_code: Option<&Code> = err.downcast_ref();
        assert_eq!(error_code, Some(&Code::CommitSigningFailed));

        repo.reload()?;
        assert_eq!(
            repo.config_snapshot().boolean("gitbutler.signCommits"),
            None,
            "we do not touch this setting in this mode"
        );

        Ok(())
    }

    #[test]
    fn sign_commit_if_enabled_without_signing_key_errors_and_sets_config() -> anyhow::Result<()> {
        let (mut repo, _tmp) = writable_scenario_with_ssh_key("single-signed");
        repo.config_snapshot_mut()
            .set_raw_value("user.signingKey", "BAD-key")?;

        let err = commit::create(
            &repo,
            commit_from_head(&repo, "should not sign")?,
            None,
            SignCommit::IfSignCommitsEnabled,
        )
        .expect_err("commit creation should fail as there is no signing key configured");

        let error_code: Option<&Code> = err.downcast_ref();
        assert_eq!(error_code, Some(&Code::CommitSigningFailed));

        repo.reload()?;
        assert_eq!(
            repo.config_snapshot().boolean("gitbutler.signCommits"),
            Some(false),
            "prevent future failures by disabling signing for gitbutler"
        );

        Ok(())
    }

    #[test]
    fn removes_existing_signature_when_not_signing() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario_with_ssh_key("single-signed");
        assert!(
            repo.head_commit()?
                .decode()?
                .extra_headers()
                .find(SIGNATURE_FIELD_NAME)
                .is_some(),
            "the fixture starts with a signed commit"
        );

        let oid = commit::create(
            &repo,
            commit_from_head(&repo, "unsigned")?,
            None,
            SignCommit::No,
        )?;
        let commit = repo.find_commit(oid)?;
        let commit = commit.decode()?;

        assert!(
            commit.extra_headers().find(SIGNATURE_FIELD_NAME).is_none(),
            "old signatures should be stripped when signing is disabled, as they would be invalid otherwise"
        );
        Ok(())
    }

    #[test]
    fn updates_the_requested_reference() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario_with_ssh_key("single-signed");
        let update_ref =
            refs::Category::LocalBranch.to_full_name("created-by-helper".as_bytes().as_bstr())?;

        let oid = commit::create(
            &repo,
            commit_from_head(&repo, "updates ref")?,
            Some(update_ref.as_ref()),
            SignCommit::No,
        )?;
        let reference = repo.find_reference(update_ref.as_ref())?;

        assert_eq!(
            reference
                .try_id()
                .context("newly created direct reference")?,
            oid
        );
        let previous = repo.find_reference("refs/heads/main")?.id();
        assert_ne!(previous, oid, "the helper should not rewrite other refs");
        Ok(())
    }

    #[test]
    fn keeps_existing_headers_and_encoding() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario_with_ssh_key("single-signed");
        let mut new_commit = commit_from_head(&repo, "preserves metadata")?;
        new_commit.encoding = Some("ISO-8859-1".into());
        new_commit
            .extra_headers
            .push(("x-test-header".into(), "kept".into()));

        let oid = commit::create(&repo, new_commit, None, SignCommit::No)?;
        let commit = repo.find_commit(oid)?;
        let commit = commit.decode()?;

        assert_eq!(commit.encoding, Some("ISO-8859-1".as_bytes().as_bstr()));
        assert_eq!(
            commit.extra_headers().find("x-test-header"),
            Some("kept".as_bytes().as_bstr())
        );
        assert!(
            commit.extra_headers().find(SIGNATURE_FIELD_NAME).is_none(),
            "the inherited signature should still be removed"
        );
        Ok(())
    }

    fn commit_from_head(
        repo: &gix::Repository,
        message: &str,
    ) -> anyhow::Result<gix::objs::Commit> {
        let head = repo.head_commit()?;
        let mut commit = head.decode()?.to_owned()?;
        commit.tree = head.tree_id()?.detach();
        commit.parents = [head.id].into();
        commit.message = message.into();
        Ok(commit)
    }
}
