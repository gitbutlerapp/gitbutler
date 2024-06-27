use gitbutler_core::virtual_branches::branch::Hunk;

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
    let hash = Hunk::hash("hash");
    assert_eq!(
        format!("2-3-{hash:x}").parse::<Hunk>().unwrap(),
        Hunk::new(2, 3, Some(hash)).unwrap()
    );
}

#[test]
fn parse_with_timestamp() {
    assert_eq!(
        "2-3--123".parse::<Hunk>().unwrap(),
        Hunk::new(2, 3, None).unwrap()
    );
}

#[test]
fn parse_invalid_2() {
    "3-2".parse::<Hunk>().unwrap_err();
}

#[test]
fn to_string_no_hash() {
    assert_eq!("1-2", Hunk::new(1, 2, None).unwrap().to_string());
}

#[test]
fn hash_diff_no_diff_header_is_normal_hash() {
    let actual = Hunk::hash_diff("a");
    let expected = Hunk::hash("a");
    assert_eq!(actual, expected)
}

#[test]
fn hash_diff_empty_is_fine() {
    let actual = Hunk::hash_diff("");
    let expected = Hunk::hash("");
    assert_eq!(
        actual, expected,
        "The special hash is the same as a normal one in case of empty input.\
        Don't yet know why that should be except that more works then"
    )
}

#[test]
fn hash_diff_content_hash() {
    let a_hash = Hunk::hash_diff("@@x\na");
    let b_hash = Hunk::hash_diff("@@y\na");
    assert_eq!(
        a_hash, b_hash,
        "it skips the first line which is assumed to be a diff-header.\
        That way, the content is hashed instead"
    )
}

#[test]
fn eq() {
    let a_hash = Hunk::hash("a");
    let b_hash = Hunk::hash("b");
    assert_ne!(a_hash, b_hash);
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
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            true,
        ),
        (
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            format!("2-3-{a_hash:x}").parse::<Hunk>().unwrap(),
            false,
        ),
        (
            "1-2".parse::<Hunk>().unwrap(),
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            true,
        ),
        (
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            "1-2".parse::<Hunk>().unwrap(),
            true,
        ),
        (
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            format!("1-2-{b_hash:x}").parse::<Hunk>().unwrap(),
            false,
        ),
        (
            format!("1-2-{a_hash:x}").parse::<Hunk>().unwrap(),
            format!("2-3-{b_hash:x}").parse::<Hunk>().unwrap(),
            false,
        ),
        (
            // Ensures unknown data is ignored
            format!("1-2-{a_hash:x}-unknown").parse::<Hunk>().unwrap(),
            "1-2".parse::<Hunk>().unwrap(),
            true,
        ),
    ] {
        assert_eq!(a == b, expected, "comparing {} and {}", a, b);
    }
}
