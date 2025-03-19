mod trailer {
    // use gitbutler_core::ops::entry::Trailer;
    use std::str::FromStr;

    use gitbutler_oplog::entry::Trailer;

    #[test]
    fn display() {
        let trailer = Trailer {
            key: "foo".to_string(),
            value: "bar".to_string(),
        };
        assert_eq!(format!("{}", trailer), "foo: bar");
    }

    #[test]
    fn from_str() {
        let s = "foo: bar";
        let trailer = Trailer::from_str(s).unwrap();
        assert_eq!(trailer.key, "foo");
        assert_eq!(trailer.value, "bar");
    }

    #[test]
    fn from_str_invalid() {
        let s = "foobar";
        let result = Trailer::from_str(s);
        assert!(result.is_err());
    }
}

mod version {
    use std::str::FromStr;

    use gitbutler_oplog::entry::{Trailer, Version};

    #[test]
    fn from_trailer() {
        let s = "Version: 3";
        let trailer = Trailer::from_str(s).unwrap();
        let version = Version::from_str(&trailer.value).unwrap();
        assert_eq!(version, Version::default());
    }

    #[test]
    fn non_default() {
        let s = "Version: 1";
        let trailer = Trailer::from_str(s).unwrap();
        let version = Version::from_str(&trailer.value).unwrap();
        assert_eq!(version, Version::from_str("1").unwrap());
    }

    #[test]
    fn invalid() {
        let s = "Version: -1";
        let trailer = Trailer::from_str(s).unwrap();
        let version = Version::from_str(&trailer.value);
        assert!(version.is_err());
    }
}

mod operation_kind {
    use std::str::FromStr;

    use gitbutler_oplog::entry::{OperationKind, SnapshotDetails, Trailer, Version};

    #[test]
    fn from_trailer() {
        let s = "Operation: CreateCommit";
        let trailer = Trailer::from_str(s).unwrap();
        let operation = OperationKind::from_str(&trailer.value).unwrap();
        assert_eq!(operation, OperationKind::CreateCommit);
    }

    #[test]
    fn unknown() {
        let commit_message = "Create a new snapshot\n\nBody text 1\nBody text2\n\nBody text 3\n\nVersion: 3\nOperation: Asdf\nFoo: Bar\n";
        let details = SnapshotDetails::from_str(commit_message).unwrap();
        assert_eq!(details.version, Version::default());
        assert_eq!(details.operation, OperationKind::Unknown);
        assert_eq!(details.title, "Create a new snapshot");
        assert_eq!(
            details.body,
            Some("Body text 1\nBody text2\n\nBody text 3".to_string())
        );
        assert_eq!(
            details.trailers,
            vec![Trailer {
                key: "Foo".to_string(),
                value: "Bar".to_string(),
            }]
        );
    }
}

mod snapshot_details {
    use std::{path::PathBuf, str::FromStr};

    use gitbutler_oplog::entry::{OperationKind, Snapshot, SnapshotDetails, Trailer, Version};

    #[test]
    fn new() {
        let commit_sha = git2::Oid::zero();
        let commit_message =
            "Create a new snapshot\n\nBody text 1\nBody text2\n\nBody text 3\n\nVersion: 3\nOperation: CreateCommit\nFoo: Bar\n".to_string();
        let timezone_offset_does_not_matter = 1234;
        let created_at = git2::Time::new(1234567890, timezone_offset_does_not_matter);
        let details = SnapshotDetails::from_str(&commit_message.clone()).unwrap();
        let snapshot = Snapshot {
            commit_id: commit_sha,
            created_at,
            lines_added: 1,
            lines_removed: 1,
            files_changed: vec![PathBuf::from("foo.txt")],
            details: Some(details),
        };
        assert_eq!(snapshot.commit_id, commit_sha);
        assert_eq!(snapshot.created_at, created_at);
        let details = snapshot.details.unwrap();
        assert_eq!(details.version, Version::default());
        assert_eq!(details.operation, OperationKind::CreateCommit);
        assert_eq!(details.title, "Create a new snapshot");
        assert_eq!(
            details.body,
            Some("Body text 1\nBody text2\n\nBody text 3".to_string())
        );
        assert_eq!(
            details.trailers,
            vec![Trailer {
                key: "Foo".to_string(),
                value: "Bar".to_string(),
            }]
        );
        assert_eq!(details.to_string(), commit_message);
    }

    #[test]
    fn new_with_newline_in_trailer() {
        let snapshot_details = new_details(Trailer {
            key: "Message".to_string(),
            value: "Header\n\nBody".to_string(),
        });
        let serialized = snapshot_details.to_string();
        let deserialized = SnapshotDetails::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized, snapshot_details,
            "this works because newlines are quoted"
        )
    }

    #[test]
    fn new_with_space_in_trailer_key() {
        for value in ["trailing-space ", " leading-space"] {
            let trailer = Trailer {
                key: value.to_string(),
                value: "anything".to_string(),
            };
            let mut snapshot_details = new_details(trailer);
            let trailer = &mut snapshot_details.trailers[0];
            trailer.key = trailer.key.trim().to_string();

            let serialized = snapshot_details.to_string();
            let deserialized = SnapshotDetails::from_str(&serialized).unwrap();
            assert_eq!(deserialized, snapshot_details, "values are trimmed")
        }
    }

    fn new_details(trailer: Trailer) -> SnapshotDetails {
        SnapshotDetails {
            version: Version::default(),
            operation: OperationKind::CreateCommit,
            title: "Create a new snapshot".to_string(),
            body: None,
            trailers: vec![trailer],
        }
    }
}
