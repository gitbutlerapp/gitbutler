use gitbutler_app::virtual_branches::branch::Hunk;

#[test]
fn to_from_string() {
    let hunk = "1-2".parse::<Hunk>().unwrap();
    assert_eq!("1-2", hunk.to_string());
}

#[test]
fn parse_invalid() {
    "3-2".parse::<Hunk>().unwrap_err();
}

#[test]
fn parse_with_hash() {
    assert_eq!(
        "2-3-hash".parse::<Hunk>().unwrap(),
        Hunk::new(2, 3, Some("hash".to_string()), None).unwrap()
    );
}

#[test]
fn parse_with_timestamp() {
    assert_eq!(
        "2-3--123".parse::<Hunk>().unwrap(),
        Hunk::new(2, 3, None, Some(123)).unwrap()
    );
}

#[test]
fn parse_invalid_2() {
    "3-2".parse::<Hunk>().unwrap_err();
}

#[test]
fn to_string_no_hash() {
    assert_eq!(
        "1-2--123",
        Hunk::new(1, 2, None, Some(123)).unwrap().to_string()
    );
}

#[test]
fn eq() {
    for (a, b, expected) in vec![
        (
            "1-2".parse::<Hunk>().unwrap(),
            "1-2".parse::<Hunk>().unwrap(),
            true,
        ),
        (
            "1-2".parse::<Hunk>().unwrap(),
            "2-3".parse::<Hunk>().unwrap(),
            false,
        ),
        (
            "1-2-abc".parse::<Hunk>().unwrap(),
            "1-2-abc".parse::<Hunk>().unwrap(),
            true,
        ),
        (
            "1-2-abc".parse::<Hunk>().unwrap(),
            "2-3-abc".parse::<Hunk>().unwrap(),
            false,
        ),
        (
            "1-2".parse::<Hunk>().unwrap(),
            "1-2-abc".parse::<Hunk>().unwrap(),
            true,
        ),
        (
            "1-2-abc".parse::<Hunk>().unwrap(),
            "1-2".parse::<Hunk>().unwrap(),
            true,
        ),
        (
            "1-2-abc".parse::<Hunk>().unwrap(),
            "1-2-bcd".parse::<Hunk>().unwrap(),
            false,
        ),
        (
            "1-2-abc".parse::<Hunk>().unwrap(),
            "2-3-bcd".parse::<Hunk>().unwrap(),
            false,
        ),
    ] {
        assert_eq!(a == b, expected, "comapring {} and {}", a, b);
    }
}
