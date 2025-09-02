use gitbutler_stack::OwnershipClaim;

#[test]
fn parse_ownership() {
    let ownership: OwnershipClaim = "foo/bar.rs:1-2,4-5".parse().unwrap();
    assert_eq!(
        ownership,
        OwnershipClaim {
            file_path: "foo/bar.rs".into(),
            hunks: vec![(1..=2).into(), (4..=5).into()]
        }
    );
}

#[test]
fn parse_ownership_tricky_file_name() {
    assert_eq!("file:name:1-2,4-5".parse::<OwnershipClaim>().unwrap(), {
        OwnershipClaim {
            file_path: "file:name".into(),
            hunks: vec![(1..=2).into(), (4..=5).into()],
        }
    });
}

#[test]
fn parse_ownership_no_ranges() {
    "foo/bar.rs".parse::<OwnershipClaim>().unwrap_err();
}

#[test]
fn ownership_to_from_string() {
    let ownership = OwnershipClaim {
        file_path: "foo/bar.rs".into(),
        hunks: vec![(1..=2).into(), (4..=5).into()],
    };
    assert_eq!(ownership.to_string(), "foo/bar.rs:1-2,4-5".to_string());
    assert_eq!(
        ownership.to_string().parse::<OwnershipClaim>().unwrap(),
        ownership
    );
}

#[test]
fn plus() {
    vec![
        ("file.txt:1-10", "another.txt:1-5", "file.txt:1-10"),
        ("file.txt:1-10,3-14", "file.txt:3-14", "file.txt:3-14,1-10"),
        ("file.txt:5-10", "file.txt:1-5", "file.txt:1-5,5-10"),
        ("file.txt:1-10", "file.txt:1-5", "file.txt:1-5,1-10"),
        ("file.txt:1-5,2-2", "file.txt:1-10", "file.txt:1-10,1-5,2-2"),
        (
            "file.txt:1-10",
            "file.txt:8-15,20-25",
            "file.txt:20-25,8-15,1-10",
        ),
        ("file.txt:1-10", "file.txt:1-10", "file.txt:1-10"),
        ("file.txt:1-10,3-15", "file.txt:1-10", "file.txt:1-10,3-15"),
    ]
    .into_iter()
    .map(|(a, b, expected)| {
        (
            a.parse::<OwnershipClaim>().unwrap(),
            b.parse::<OwnershipClaim>().unwrap(),
            expected.parse::<OwnershipClaim>().unwrap(),
        )
    })
    .for_each(|(a, b, expected)| {
        let got = a.plus(b.clone());
        assert_eq!(
            got, expected,
            "{a} plus {b}, expected {expected}, got {got}"
        );
    });
}

#[test]
fn minus() {
    vec![
        (
            "file.txt:1-10",
            "another.txt:1-5",
            (None, Some("file.txt:1-10")),
        ),
        (
            "file.txt:1-10",
            "file.txt:1-5",
            (None, Some("file.txt:1-10")),
        ),
        (
            "file.txt:1-10",
            "file.txt:11-15",
            (None, Some("file.txt:1-10")),
        ),
        (
            "file.txt:1-10",
            "file.txt:1-10",
            (Some("file.txt:1-10"), None),
        ),
        (
            "file.txt:1-10,11-15",
            "file.txt:11-15",
            (Some("file.txt:11-15"), Some("file.txt:1-10")),
        ),
        (
            "file.txt:1-10,11-15,15-17",
            "file.txt:1-10,15-17",
            (Some("file.txt:1-10,15-17"), Some("file.txt:11-15")),
        ),
    ]
    .into_iter()
    .map(|(a, b, expected)| {
        (
            a.parse::<OwnershipClaim>().unwrap(),
            b.parse::<OwnershipClaim>().unwrap(),
            (
                expected.0.map(|s| s.parse::<OwnershipClaim>().unwrap()),
                expected.1.map(|s| s.parse::<OwnershipClaim>().unwrap()),
            ),
        )
    })
    .for_each(|(a, b, expected)| {
        let got = a.minus(&b);
        assert_eq!(
            got, expected,
            "{a} minus {b}, expected {expected:?}, got {got:?}"
        );
    });
}

#[test]
fn equal() {
    vec![
        ("file.txt:1-10", "file.txt:1-10", true),
        ("file.txt:1-10", "file.txt:1-11", false),
        ("file.txt:1-10,11-15", "file.txt:11-15,1-10", false),
        ("file.txt:1-10,11-15", "file.txt:1-10,11-15", true),
    ]
    .into_iter()
    .map(|(a, b, expected)| {
        (
            a.parse::<OwnershipClaim>().unwrap(),
            b.parse::<OwnershipClaim>().unwrap(),
            expected,
        )
    })
    .for_each(|(a, b, expected)| {
        assert_eq!(a == b, expected, "{a} == {b}, expected {expected}");
    });
}
