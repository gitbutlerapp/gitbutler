use but_forge::ForgeName;

#[test]
fn from_override_str_is_case_and_whitespace_insensitive() {
    assert_eq!(
        ForgeName::from_override_str("gitlab"),
        Some(ForgeName::GitLab)
    );
    assert_eq!(
        ForgeName::from_override_str("  GitLab "),
        Some(ForgeName::GitLab)
    );
    // "default" (the unset sentinel) and anything unrecognized are no-override.
    assert_eq!(ForgeName::from_override_str("default"), None);
    assert_eq!(ForgeName::from_override_str("subversion"), None);
}
