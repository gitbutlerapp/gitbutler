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
    let hash = Hunk::hash("hash".into());
    assert_eq!(
        format!("2-3-{hash:x}").parse::<Hunk>().unwrap(),
        Hunk::new(2, 3, Some(hash), None).unwrap()
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
fn hash() {
    let a_hash = Hunk::hash("a".into());
    let b_hash = Hunk::hash("b".into());
    assert_ne!(
        a_hash, b_hash,
        "even single-line input yields different hashes"
    );

    let a_hash = Hunk::hash("first\na".into());
    let b_hash = Hunk::hash("different-first\na".into());
    assert_eq!(
        a_hash, b_hash,
        "it skips the first line which is assumed to be a diff-header.\
        That way, the content is hashed instead"
    )
}

#[test]
fn eq() {
    let a_hash = Hunk::hash("a".into());
    let b_hash = Hunk::hash("b".into());
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
    ] {
        assert_eq!(a == b, expected, "comparing {} and {}", a, b);
    }
}
