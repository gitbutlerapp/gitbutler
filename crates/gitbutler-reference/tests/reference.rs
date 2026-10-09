mod remote_refname {
    mod parsing {
        use gitbutler_reference::RemoteRefname;

        #[test]
        fn remote_branch_with_slashes() {
            let name: RemoteRefname = "refs/remotes/origin/feature/topic".parse().unwrap();
            assert_eq!(
                name.remote(),
                Some("origin"),
                "the first component remains the remote"
            );
            assert_eq!(
                name.branch(),
                "feature/topic",
                "branch names can contain slashes"
            );
        }

        #[test]
        fn local_branch_roundtrip() {
            let name: RemoteRefname = "refs/heads/feature/topic".parse().unwrap();
            assert_eq!(name.remote(), None, "local branches have no remote");
            assert_eq!(
                name.branch(),
                "feature/topic",
                "the branch name is preserved"
            );
            assert_eq!(
                name.to_string(),
                "refs/heads/feature/topic",
                "display preserves the local namespace"
            );

            let full_name: gix::refs::FullName = (&name).try_into().unwrap();
            assert_eq!(
                name,
                *full_name.as_ref(),
                "local refs compare equal to their full name"
            );
            let remote_name: gix::refs::FullName =
                "refs/remotes/origin/feature/topic".try_into().unwrap();
            assert_ne!(
                name,
                *remote_name.as_ref(),
                "local and remote refs remain distinct"
            );
            let owned_full_name: gix::refs::FullName = name.clone().try_into().unwrap();
            assert_eq!(
                owned_full_name, full_name,
                "owned conversion preserves the local namespace"
            );

            let renamed = name.with_branch("other");
            assert_eq!(
                renamed.remote(),
                None,
                "changing the branch preserves the absence of a remote"
            );
            assert_eq!(
                renamed.to_string(),
                "refs/heads/other",
                "renamed branches remain local"
            );

            let generic = gitbutler_reference::Refname::from(&name);
            assert!(
                matches!(generic, gitbutler_reference::Refname::Local(_)),
                "conversion classifies the ref as local"
            );
            assert_eq!(generic.remote(), None, "the generic ref has no remote");
            assert_eq!(
                generic.to_string(),
                name.to_string(),
                "generic conversion preserves the ref"
            );
        }

        #[test]
        fn tag_is_not_remote() {
            let err = "refs/tags/v1".parse::<RemoteRefname>().unwrap_err();
            assert_eq!(
                err.to_string(),
                "branch is not remote: refs/tags/v1",
                "tags are not remote branches"
            );
        }

        #[test]
        fn invalid_remote_branch_is_rejected() {
            let err = "refs/remotes/origin/bad..name"
                .parse::<RemoteRefname>()
                .unwrap_err();
            assert_eq!(
                err.to_string(),
                "branch name is invalid: refs/remotes/origin/bad..name",
                "Git refname validation rejects double dots"
            );
        }
    }

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
