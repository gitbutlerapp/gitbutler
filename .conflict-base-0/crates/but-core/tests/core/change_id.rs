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

mod decode_reverse_hex_bytes {
    use but_core::ChangeId;

    #[test]
    fn decode_reverse_hex_bytes_roundtrip_standard_reverse_hex_ids() {
        let bytes = b"\x00\x0c\xc0\xcc\xff\x12\x34\x56";
        let change_id = ChangeId::from_bytes(bytes);

        assert_eq!(
            change_id.decode_reverse_hex_bytes(),
            Some({
                let mut expected = bytes.to_vec();
                expected.resize(16, 0x00);
                expected
            })
        );
    }

    #[test]
    fn decode_reverse_hex_bytes_rejects_non_standard_length() {
        let change_id = ChangeId::from_number_for_testing(12345);

        assert_eq!(change_id.decode_reverse_hex_bytes(), None);
    }

    #[test]
    fn decode_reverse_hex_bytes_rejects_non_reverse_hex_characters() {
        let mut change_id = ChangeId::from_bytes(&[0xaa; 16]);
        change_id.prefix_with(b"not-reverse-hex-".iter().copied());

        assert_eq!(change_id.decode_reverse_hex_bytes(), None);
    }
}
