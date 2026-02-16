mod generate {
    use but_core::ChangeId;

    #[test]
    fn returns_a_32_character_random_string() {
        let a = ChangeId::generate();
        assert_eq!(a.to_string().len(), 32);
        let b = ChangeId::generate();
        assert_ne!(a, b, "these are always different");
    }
}
