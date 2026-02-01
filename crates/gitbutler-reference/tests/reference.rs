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
