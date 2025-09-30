mod workspace {
    use but_core::ref_metadata::Workspace;

    #[test]
    fn add_new_stack_if_not_present() {
        let mut ws = Workspace::default();
        assert_eq!(ws.stacks.len(), 0);

        let a_ref = r("refs/heads/A");
        assert!(ws.add_new_stack_if_not_present(a_ref));
        assert!(!ws.add_new_stack_if_not_present(a_ref));
        assert_eq!(ws.stacks.len(), 1);

        let b_ref = r("refs/heads/B");
        assert!(ws.add_new_stack_if_not_present(b_ref));
        assert_eq!(ws.stacks.len(), 2);
    }

    fn r(name: &str) -> &gix::refs::FullNameRef {
        name.try_into().expect("statically known ref")
    }
}
