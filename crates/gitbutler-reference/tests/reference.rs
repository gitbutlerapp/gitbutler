mod normalize_branch_name {
    use gitbutler_reference::normalize_branch_name;

    #[test]
    fn valid_substitutions() {
        for (input, expected) in [
            ("a", "a"),
            ("a+b", "a+b"),
            ("a^b", "a-b"),
            ("a^^^b", "a-b"),
            ("a~b", "a-b"),
            ("a<b", "a<b"),
            ("a>b", "a>b"),
            ("a\\b", "a-b"),
            ("a:b", "a-b"),
            ("a*b", "a-b"),
            ("a b", "a-b"),
            ("a\tb", "a-b"),
            ("a\nb", "a-b"),
            ("a\rb", "a-b"),
            ("a\r\nb", "a-b"),
            ("-a-", "a"),
            ("/a/", "a"),
            ("-/a/-", "a"),
            ("/-a-/", "a"),
            (".a.", "a"),
        ] {
            assert_eq!(
                normalize_branch_name(input).expect("valid"),
                expected,
                "{input} -> {expected}"
            );
        }
    }

    #[test]
    fn clear_error_on_failure() {
        assert_eq!(
            normalize_branch_name("-").unwrap_err().to_string(),
            "Could not turn \"-\" into a valid reference name",
            "show the original value, not the processed one to be familiar to the user"
        );
    }

    #[test]
    fn complex_valid() -> anyhow::Result<()> {
        assert_eq!(normalize_branch_name("feature/branch")?, "feature/branch");
        assert_eq!(normalize_branch_name("#[test]")?, "#-test]");
        assert_eq!(normalize_branch_name("foo#branch")?, "foo#branch");
        assert_eq!(normalize_branch_name("foo!branch")?, "foo!branch");
        let input = r#"Revert "GitButler Workspace Commit"

This reverts commit d6efa5fd96d36da445d5d1345b84163f05f5f229."#;
        assert_eq!(
            normalize_branch_name(input)?,
            "Revert-\"GitButler-Workspace-Commit\"-This-reverts-commit-d6efa5fd96d36da445d5d1345b84163f05f5f229"
        );
        Ok(())
    }
}

mod remote_refname {
    mod eq_fullname_ref {
        use gitbutler_reference::RemoteRefname;
        use gix::refs::FullNameRef;

        fn fullname_ref(fullname: &str) -> &FullNameRef {
            fullname.try_into().expect("known to be valid")
        }

        #[test]
        fn comparison() {
            let origin_main = RemoteRefname::new("origin", "main");
            assert_eq!(origin_main, *fullname_ref("refs/remotes/origin/main"));

            assert_ne!(origin_main, *fullname_ref("refs/remotes/origin2/main"));
            assert_ne!(origin_main, *fullname_ref("refs/remotes/origim/main"));
            assert_ne!(origin_main, *fullname_ref("refs/remotes/origin/maim"));
            assert_ne!(origin_main, *fullname_ref("refs/abcdefg/origin/main"));

            assert_ne!(origin_main, *fullname_ref("refs/heads/origin/main"));
            assert_ne!(origin_main, *fullname_ref("refs/heads/main"));
            assert_ne!(origin_main, *fullname_ref("refs/remotes/origin"));
            assert_ne!(origin_main, *fullname_ref("refs/remotes/main"));

            let multi_slash = RemoteRefname::new("my/one", "feature");
            assert_eq!(multi_slash, *fullname_ref("refs/remotes/my/one/feature"));
        }
    }
}
