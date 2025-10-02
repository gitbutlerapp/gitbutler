mod workspace {
    use but_core::ref_metadata::Workspace;

    #[test]
    fn add_new_stack_if_not_present() {
        let mut ws = Workspace::default();
        assert_eq!(ws.stacks.len(), 0);

        let a_ref = r("refs/heads/A");
        assert!(ws.add_or_insert_new_stack_if_not_present(a_ref, Some(100)));
        assert!(!ws.add_or_insert_new_stack_if_not_present(a_ref, Some(200)));
        assert_eq!(ws.stacks.len(), 1);

        let b_ref = r("refs/heads/B");
        assert!(ws.add_or_insert_new_stack_if_not_present(b_ref, Some(0)));
        assert_eq!(ws.stack_names().collect::<Vec<_>>(), [b_ref, a_ref]);

        let c_ref = r("refs/heads/C");
        assert!(ws.add_or_insert_new_stack_if_not_present(c_ref, None));
        assert_eq!(ws.stack_names().collect::<Vec<_>>(), [b_ref, a_ref, c_ref]);
    }

    fn r(name: &str) -> &gix::refs::FullNameRef {
        name.try_into().expect("statically known ref")
    }
}
