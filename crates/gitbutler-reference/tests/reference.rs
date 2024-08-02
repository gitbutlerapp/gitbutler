mod normalize_branch_name {
    use gitbutler_reference::normalize_branch_name;

    #[test]
    fn valid_substitutions() {
        for (input, expected) in [
            ("a", "a"),
            ("a+b", "a-b"),
            ("a^b", "a-b"),
            ("a~b", "a-b"),
            ("a<b", "a-b"),
            ("a>b", "a-b"),
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
            assert_eq!(normalize_branch_name(input).expect("valid"), expected);
        }
    }

    #[test]
    fn clear_error_on_failure() {
        assert_eq!(
            normalize_branch_name("-").unwrap_err().to_string(),
            "Could not turn \"\" into a valid reference name"
        );
        assert_eq!(
            normalize_branch_name("#[test]").unwrap_err().to_string(),
            "Could not turn \"#[test]\" into a valid reference name"
        );
    }

    #[test]
    fn complex_valid() -> anyhow::Result<()> {
        assert_eq!(normalize_branch_name("feature/branch")?, "feature/branch");
        assert_eq!(normalize_branch_name("foo#branch")?, "foo#branch");
        assert_eq!(normalize_branch_name("foo!branch")?, "foo!branch");
        let input = r#"Revert "GitButler Integration Commit"

This reverts commit d6efa5fd96d36da445d5d1345b84163f05f5f229."#;
        assert_eq!(normalize_branch_name(input)?, "Revert-\"GitButler-Integration-Commit\"-This-reverts-commit-d6efa5fd96d36da445d5d1345b84163f05f5f229");
        Ok(())
    }
}
