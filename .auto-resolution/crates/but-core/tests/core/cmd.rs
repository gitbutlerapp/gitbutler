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
