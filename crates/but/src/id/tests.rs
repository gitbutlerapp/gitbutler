use anyhow::bail;
use but_core::ref_metadata::StackId;
use but_hunk_assignment::HunkAssignment;
use but_testsupport::{hex_to_id, hunk_header};
use but_workspace::branch::Stack;

use crate::{CliId, IdMap, id::id_usage::UintId};

#[test]
fn uint_id_from_short_id() -> anyhow::Result<()> {
    assert_eq!(UintId::from_name(b"a".as_slice()), None);
    assert_eq!(UintId::from_name(b"a0".as_slice()), None);
    assert_eq!(UintId::from_name(b"--".as_slice()), None);
    assert_eq!(UintId::from_name(b"g0".as_slice()), Some(UintId(0)));
    assert_eq!(UintId::from_name(b"z0".as_slice()), Some(UintId(19)));
    assert_eq!(UintId::from_name(b"gz".as_slice()), Some(UintId(700)));
    assert_eq!(UintId::from_name(b"zz".as_slice()), Some(UintId(719)));
    assert_eq!(UintId::from_name(b"g00".as_slice()), Some(UintId(720)));
    assert_eq!(UintId::from_name(b"gz0".as_slice()), Some(UintId(1420)));
    assert_eq!(UintId::from_name(b"zzz".as_slice()), Some(UintId(26639)));
    assert_eq!(UintId::from_name(b"g000".as_slice()), None);
    Ok(())
}

#[test]
fn uint_id_to_short_id() -> anyhow::Result<()> {
    assert_eq!(UintId(0).to_short_id(), "g0");
    assert_eq!(UintId(19).to_short_id(), "z0");
    assert_eq!(UintId(700).to_short_id(), "gz");
    assert_eq!(UintId(719).to_short_id(), "zz");
    assert_eq!(UintId(720).to_short_id(), "g00");
    assert_eq!(UintId(1420).to_short_id(), "gz0");
    assert_eq!(UintId(26639).to_short_id(), "zzz");
    assert_eq!(UintId(26640).to_short_id(), "00", "too big always yields this");
    assert_eq!(UintId(26641).to_short_id(), "00", "too big always yields this");
    Ok(())
}

#[test]
fn commit_id_works_with_two_or_more_characters() -> anyhow::Result<()> {
    let id1 = id(1);
    let stacks = vec![stack([segment("not-important", [id1], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new())?;
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 1
    branches: [ no ]
    ");
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };

    let expected = [CliId::Commit {
        commit_id: id1,
        id: "01".to_string(),
    }];
    assert_eq!(
        id_map.parse("01", Box::new(changed_paths_fn))?,
        expected,
        "two characters are sufficient to parse a commit ID"
    );
    assert_eq!(
        id_map.parse("010", Box::new(changed_paths_fn))?,
        expected,
        "three characters work too"
    );
    assert_eq!(
        id_map.parse("1", Box::new(changed_paths_fn)).unwrap_err().to_string(),
        "Id needs to be at least 2 characters long: '1'",
        "one character isn't enough"
    );
    Ok(())
}

#[test]
fn commit_ids_become_longer_if_ambiguous() -> anyhow::Result<()> {
    let id1 = hex_to_id("21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let id2 = hex_to_id("21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let id3 = hex_to_id("21bccccccccccccccccccccccccccccccccccccc");
    let stacks = vec![stack([segment("not-important", [id1, id2, id3], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new())?;
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 3
    branches: [ no ]
    ");
    insta::assert_debug_snapshot!(id_map.all_ids(), @r#"
    [
        Commit {
            commit_id: Sha1(21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
            id: "21a",
        },
        Commit {
            commit_id: Sha1(21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
            id: "21bb",
        },
        Commit {
            commit_id: Sha1(21bccccccccccccccccccccccccccccccccccccc),
            id: "21bc",
        },
        Branch {
            name: "not-important",
            id: "no",
            stack_id: None,
        },
    ]
    "#);
    let ids_as_shown_by_consumers = id_map
        .all_ids()
        .iter()
        .map(|id| id.to_short_string())
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(ids_as_shown_by_consumers, @r#"
    [
        "21a",
        "21bb",
        "21bc",
        "no",
    ]
    "#);
    Ok(())
}

#[test]
fn branches_work_with_single_character() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("f", [id(1)], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new())?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 1
    branches: [ g0 ]
    ");

    let expected = [CliId::Branch {
        name: "f".into(),
        id: "g0".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("f", Box::new(changed_paths_fn))?,
        expected,
        "it's OK to have a CliID that is longer, but it would be up to the UI to not show them"
    );
    assert_eq!(
        id_map.parse("g0", Box::new(changed_paths_fn))?,
        expected,
        "the ID also works"
    );
    Ok(())
}

#[test]
fn branches_match_by_substring() -> anyhow::Result<()> {
    let stacks = vec![stack([
        segment("foo-bar", [id(1)], None, []),
        segment("bar", [id(2)], None, []),
        segment("foo", [id(3)], None, []),
        segment("baz", [id(4)], None, []),
    ])];

    let id_map = IdMap::new(stacks, Vec::new())?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 4
    branches: [ az, g0, h0, i0 ]
    ");

    let expected = [
        CliId::Branch {
            name: "foo-bar".into(),
            id: "g0".into(),
            stack_id: None,
        },
        CliId::Branch {
            name: "foo".into(),
            id: "i0".into(),
            stack_id: None,
        },
    ];
    assert_eq!(
        id_map.parse("fo", Box::new(changed_paths_fn))?,
        expected,
        "substring searches can yield multiple items"
    );

    let expected = [CliId::Branch {
        name: "baz".into(),
        id: "az".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("az", Box::new(changed_paths_fn))?,
        expected,
        "We see the ID was generated from a substring directly"
    );
    Ok(())
}

#[test]
fn branches_avoid_unassigned_area_id() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("zza", [id(1)], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new())?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 1
    branches: [ za ]
    ");

    let expected = [CliId::Branch {
        name: "zza".into(),
        id: "za".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("za", Box::new(changed_paths_fn))?,
        expected,
        "avoids unassigned area ID (zz)"
    );
    Ok(())
}

#[test]
fn branches_avoid_invalid_ids() -> anyhow::Result<()> {
    let stacks = vec![stack([
        segment("x-yz_/hi", [id(1)], None, []),
        segment("0ax", [id(2)], None, []),
    ])];
    let id_map = IdMap::new(stacks, Vec::new())?;
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 2
    branches: [ ax, yz ]
    ");
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };

    let expected = [CliId::Branch {
        name: "x-yz_/hi".into(),
        id: "yz".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("x-yz", Box::new(changed_paths_fn))?,
        expected,
        "avoids non-alphanumeric, taking first alphanumeric pair"
    );
    let expected = [CliId::Branch {
        name: "0ax".into(),
        id: "ax".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("0ax", Box::new(changed_paths_fn))?,
        expected,
        "avoids hexdigit pair which can be confused with a commit ID"
    );
    Ok(())
}

#[test]
fn branches_avoid_uncommitted_filenames() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("ghij", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("gh", None), hunk_assignment("hi", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 1
    branches: [ ij ]
    uncommitted_files: [ nx, yz ]
    uncommitted_hunks: [ i0, j0 ]
    ");

    let expected = [CliId::Branch {
        name: "ghij".into(),
        id: "ij".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("ghij", Box::new(changed_paths_fn))?,
        expected,
        "avoids 'gh' and 'hi', which conflict with filenames"
    );
    Ok(())
}

#[test]
fn branch_cannot_generate_id() -> anyhow::Result<()> {
    let stacks = vec![
        stack([segment("substring", [id(1)], None, [])]),
        stack([segment("supersubstring", [id(2)], None, [])]),
    ];
    let id_map = IdMap::new(stacks, Vec::new())?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 2
    branches: [ g0, up ]
    ");

    let expected = [CliId::Branch {
        name: "substring".into(),
        id: "g0".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("substring", Box::new(changed_paths_fn))?,
        expected,
        "no unique ID, so take from pool of IDs (this one matched precisely)",
    );
    let expected = [CliId::Branch {
        name: "supersubstring".into(),
        id: "up".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("supersubstring", Box::new(changed_paths_fn))?,
        expected,
        "'su' would collide with substring, so 'up' is chosen (this one matched precisely)"
    );
    Ok(())
}

#[test]
fn non_commit_ids_do_not_collide() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("h0", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![
        HunkAssignment {
            hunk_header: Some(hunk_header("-1,2", "+1,2")),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        HunkAssignment {
            hunk_header: Some(hunk_header("-3,2", "+3,2")),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        hunk_assignment("uncommitted2.txt", None),
    ];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 1
    branches: [ h0 ]
    uncommitted_files: [ kv, ro ]
    uncommitted_hunks: [ j0, k0, l0 ]
    stacks: [ m0 ]
    ");
    insta::assert_debug_snapshot!(id_map.all_ids(), @r#"
    [
        Commit {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            id: "02",
        },
        Branch {
            name: "h0",
            id: "h0",
            stack_id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
        },
        Uncommitted(
            UncommittedCliId {
                id: "j0",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "uncommitted2.txt",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: false,
            },
        ),
        Uncommitted(
            UncommittedCliId {
                id: "k0",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "",
                        path_bytes: "uncommitted1.txt",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: false,
            },
        ),
        Uncommitted(
            UncommittedCliId {
                id: "kv",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "uncommitted2.txt",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
        Uncommitted(
            UncommittedCliId {
                id: "l0",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: Some(
                            HunkHeader("-3,2", "+3,2"),
                        ),
                        path: "",
                        path_bytes: "uncommitted1.txt",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: false,
            },
        ),
        Stack {
            id: "m0",
            stack_id: 00000000-0000-0000-0000-000000000001,
        },
        Uncommitted(
            UncommittedCliId {
                id: "ro",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "",
                        path_bytes: "uncommitted1.txt",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [
                        HunkAssignment {
                            id: None,
                            hunk_header: Some(
                                HunkHeader("-3,2", "+3,2"),
                            ),
                            path: "",
                            path_bytes: "uncommitted1.txt",
                            stack_id: None,
                            hunk_locks: None,
                            line_nums_added: None,
                            line_nums_removed: None,
                            diff: None,
                        },
                    ],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn ids_are_case_sensitive() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("h0", [id(10)], Some(id(9)), [])])];
    let hunk_assignments = vec![hunk_assignment("uncommitted.txt", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(10) && parent_id == Some(id(9)) {
                vec![tree_change_addition("committed.txt")]
            } else {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };
    insta::assert_debug_snapshot!(id_map.debug_state(), @r"
    workspace_and_remote_commits_count: 1
    branches: [ h0 ]
    uncommitted_files: [ ln ]
    uncommitted_hunks: [ i0 ]
    ");

    insta::assert_debug_snapshot!(id_map.parse("0a", Box::new(changed_paths_fn))?, @r#"
    [
        Commit {
            commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
            id: "0a",
        },
    ]
    "#);
    assert_eq!(
        id_map.parse("0A", Box::new(changed_paths_fn))?,
        [],
        "the case matters for commits"
    );

    insta::assert_debug_snapshot!(id_map.parse("h0", Box::new(changed_paths_fn))?, @r#"
    [
        Branch {
            name: "h0",
            id: "h0",
            stack_id: None,
        },
    ]
    "#);
    assert_eq!(
        id_map.parse("H0", Box::new(changed_paths_fn))?,
        [],
        "the case matters for branches"
    );

    insta::assert_debug_snapshot!(id_map.parse("ln", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "ln",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "uncommitted.txt",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);
    assert_eq!(
        id_map.parse("LN", Box::new(changed_paths_fn))?,
        [],
        "the case matters for uncommitted files"
    );

    insta::assert_debug_snapshot!(id_map.parse("0a:zt", Box::new(changed_paths_fn))?, @r#"
    [
        CommittedFile {
            commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
            path: "committed.txt",
            id: "0a:zt",
        },
    ]
    "#);
    assert_eq!(
        id_map.parse("0a:ZT", Box::new(changed_paths_fn))?,
        [],
        "the case matters for committed files"
    );

    Ok(())
}

#[test]
fn uncommitted_files_disambiguate_between_themselves() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("foo23", None), hunk_assignment("foo242", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(1) && parent_id.is_none() {
                vec![]
            } else {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };

    // Ambiguous ID returns every possible match
    insta::assert_debug_snapshot!(id_map.parse("kp", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "kpo",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo242",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
        Uncommitted(
            UncommittedCliId {
                id: "kpr",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo23",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    insta::assert_debug_snapshot!(id_map.parse("kpo", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "kpo",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo242",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);
    insta::assert_debug_snapshot!(id_map.parse("kpr", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "kpr",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo23",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn uncommitted_files_disambiguate_with_branch() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("fookpfoo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("foo23", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(1) && parent_id.is_none() {
                vec![]
            } else {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };

    // Only the branch is returned
    insta::assert_debug_snapshot!(id_map.parse("kp", Box::new(changed_paths_fn))?, @r#"
    [
        Branch {
            name: "fookpfoo",
            id: "ok",
            stack_id: None,
        },
    ]
    "#);

    // More characters must be specified to get the file
    insta::assert_debug_snapshot!(id_map.parse("kpr", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "kpr",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo23",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn longer_id_is_ok() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("foo23", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(1) && parent_id.is_none() {
                vec![]
            } else {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };

    // "kp" would be sufficient (see the "id" field in the output), but "kpr" works too
    insta::assert_debug_snapshot!(id_map.parse("kpr", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "kp",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo23",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn reverse_hex_filename_is_its_own_id() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("klmxyz", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(1) && parent_id.is_none() {
                vec![]
            } else {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };

    // "klmxyz" does not have an autogenerated ID
    insta::assert_debug_snapshot!(id_map.parse("kl", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "kl",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "klmxyz",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn branch_and_file_by_name() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("foo", None)];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(1) && parent_id.is_none() {
                vec![]
            } else {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };

    // Both branches and uncommitted, unassigned files match by name, and none
    // have priority over the other (i.e. if there is both a branch and a file
    // that matches, the result is ambiguous).
    insta::assert_debug_snapshot!(id_map.parse("foo", Box::new(changed_paths_fn))?, @r#"
    [
        Branch {
            name: "foo",
            id: "fo",
            stack_id: None,
        },
        Uncommitted(
            UncommittedCliId {
                id: "zo",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "foo",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn colon_uncommitted_filename() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("gggg", [id(2)], None, [])])
    }];
    let hunk_assignments = vec![
        hunk_assignment("unassigned", None),
        hunk_assignment("assigned", Some(StackId::from_number_for_testing(1))),
    ];
    let id_map = IdMap::new(stacks, hunk_assignments)?;
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        };

    // Short branch works
    insta::assert_debug_snapshot!(id_map.parse("gg@{stack}:assigned", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "mv",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "assigned",
                        stack_id: Some(
                            00000000-0000-0000-0000-000000000001,
                        ),
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    // Long branch works
    insta::assert_debug_snapshot!(id_map.parse("gggg@{stack}:assigned", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "mv",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "assigned",
                        stack_id: Some(
                            00000000-0000-0000-0000-000000000001,
                        ),
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    // Unassigned works
    insta::assert_debug_snapshot!(id_map.parse("zz:unassigned", Box::new(changed_paths_fn))?, @r#"
    [
        Uncommitted(
            UncommittedCliId {
                id: "wq",
                hunk_assignments: NonEmpty {
                    head: HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "unassigned",
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                    tail: [],
                },
                is_entire_file: true,
            },
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn committed_files_are_deduplicated_by_commit_oid_path() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("branch", [id(2)], Some(id(1)), [])])];
    let id_map = IdMap::new(stacks, Vec::new())?;

    // Simulate a changed_paths function that returns the same file twice
    // (which could happen due to a bug in the caller or data source)
    let changed_paths_fn =
        |commit_id: gix::ObjectId, parent_id: Option<gix::ObjectId>| -> anyhow::Result<Vec<but_core::TreeChange>> {
            Ok(if commit_id == id(2) && parent_id == Some(id(1)) {
                vec![
                    tree_change_addition("file.txt"),
                    tree_change_addition("file.txt"), // Duplicate!
                    tree_change_addition("other.txt"),
                ]
            } else {
                anyhow::bail!("unexpected IDs {commit_id} {parent_id:?}");
            })
        };

    // Verify we can look up both files both by ID and filename
    assert!(id_map.parse("02:uv", Box::new(changed_paths_fn))?.len() == 1);
    assert!(id_map.parse("02:xw", Box::new(changed_paths_fn))?.len() == 1);
    assert!(id_map.parse("02:file.txt", Box::new(changed_paths_fn))?.len() == 1);
    assert!(id_map.parse("02:other.txt", Box::new(changed_paths_fn))?.len() == 1);

    Ok(())
}

mod util {
    use std::{cmp::Ordering, fmt::Formatter};

    use anyhow::bail;
    use bstr::BString;
    use but_core::ref_metadata::StackId;
    use but_hunk_assignment::HunkAssignment;
    use but_workspace::{
        branch::Stack,
        ref_info::{Commit, LocalCommit, Segment},
    };
    use itertools::Itertools;

    use crate::{CliId, IdMap};

    pub fn id(byte: u8) -> gix::ObjectId {
        gix::ObjectId::try_from([byte].repeat(20).as_slice()).expect("could not generate ID")
    }

    pub fn segment<const N1: usize, const N2: usize>(
        shortened_branch_name: &str,
        local_commit_ids: [gix::ObjectId; N1],
        base: Option<gix::ObjectId>,
        remote_commit_ids: [gix::ObjectId; N2],
    ) -> Segment {
        fn commit(id: gix::ObjectId, parent_id: Option<gix::ObjectId>) -> Commit {
            Commit {
                id,
                parent_ids: parent_id.into_iter().collect::<Vec<gix::ObjectId>>(),
                tree_id: gix::index::hash::Kind::Sha1.empty_tree(),
                message: Default::default(),
                author: Default::default(),
                refs: Vec::new(),
                flags: Default::default(),
                has_conflicts: false,
                change_id: None,
            }
        }

        let ref_info = Some(but_graph::RefInfo {
            ref_name: gix::refs::FullName::try_from(format!("refs/heads/{shortened_branch_name}"))
                .expect("could not generate ref name"),
            worktree: None,
        });
        let mut commits: Vec<LocalCommit> = Vec::new();
        for (i, id) in local_commit_ids.iter().enumerate() {
            let parent_id = local_commit_ids.get(i + 1).or(base.as_ref());
            commits.push(LocalCommit {
                inner: commit(*id, parent_id.cloned()),
                relation: Default::default(),
            });
        }
        let mut commits_on_remote: Vec<Commit> = Vec::new();
        for id in remote_commit_ids {
            commits_on_remote.push(commit(id, None))
        }
        Segment {
            ref_info,
            id: Default::default(),
            remote_tracking_ref_name: None,
            commits,
            commits_on_remote,
            commits_outside: None,
            metadata: None,
            is_entrypoint: false,
            push_status: but_workspace::ui::PushStatus::NothingToPush,
            base,
        }
    }

    pub fn stack<const N: usize>(segments: [Segment; N]) -> Stack {
        Stack {
            id: None,
            base: None,
            segments: segments.into_iter().collect::<Vec<Segment>>(),
        }
    }

    pub fn hunk_assignment(path: &str, stack_id: Option<StackId>) -> HunkAssignment {
        HunkAssignment {
            id: None,
            hunk_header: None,
            path: String::new(),
            path_bytes: BString::from(path),
            stack_id,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        }
    }

    pub fn tree_change_addition(path: &str) -> but_core::TreeChange {
        but_core::TreeChange {
            path: BString::from(path),
            status: but_core::TreeStatus::Addition {
                state: but_core::ChangeState {
                    // `IdMap` only identifies a committed file by its commit ID
                    // and filename, so the object ID does not matter.
                    id: gix::ObjectId::null(gix::hash::Kind::Sha1),
                    kind: gix::objs::tree::EntryKind::Blob,
                },
                is_untracked: false,
            },
        }
    }

    impl IdMap {
        /// Display internal information to aid understanding and debugging
        pub fn debug_state(&self) -> DebugState<'_> {
            DebugState { inner: self }
        }

        /// Return a list of all branch CliIds.
        pub fn branch_ids(&self) -> Vec<String> {
            let mut short_ids = Vec::new();
            for stack_with_id in self.indexed_stacks.borrow_owner().iter() {
                for segment_with_id in stack_with_id.segments.iter() {
                    short_ids.push(segment_with_id.short_id.clone());
                }
            }
            short_ids
        }

        /// Return a list of all commit CliIds.
        pub fn commit_ids(&self) -> Vec<String> {
            let mut short_ids = Vec::new();
            for stack_with_id in self.indexed_stacks.borrow_owner().iter() {
                for segment_with_id in stack_with_id.segments.iter() {
                    for workspace_commit_with_id in segment_with_id.workspace_commits.iter() {
                        short_ids.push(workspace_commit_with_id.short_id.clone());
                    }
                    for remote_commit_with_id in segment_with_id.remote_commits.iter() {
                        short_ids.push(remote_commit_with_id.short_id.clone());
                    }
                }
            }
            short_ids
        }

        /// Return a sorted list of all CliIds we can provide, excluding unassigned.
        pub fn all_ids(&self) -> Vec<CliId> {
            let IdMap {
                indexed_stacks: _,
                stack_ids,
                unassigned: _,
                uncommitted_files,
                uncommitted_hunks,
            } = self;
            let changed_paths_fn = |commit_id: gix::ObjectId,
                                    parent_id: Option<gix::ObjectId>|
             -> anyhow::Result<Vec<but_core::TreeChange>> {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            };

            self.branch_ids()
                .into_iter()
                .chain(stack_ids.values().map(|id| id.to_short_string()))
                .chain(self.commit_ids())
                .chain(
                    uncommitted_files
                        .values()
                        .map(|uncommitted_file| uncommitted_file.short_id.clone()),
                )
                .chain(uncommitted_hunks.keys().cloned())
                .flat_map(|id| {
                    self.parse(&id, Box::new(changed_paths_fn))
                        .expect("BUG: valid ID means no error")
                })
                .sorted_by(id_cmp)
                .collect()
        }
    }

    pub struct DebugState<'a> {
        inner: &'a IdMap,
    }

    impl std::fmt::Debug for DebugState<'_> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            use itertools::Itertools;
            let IdMap {
                indexed_stacks: _,
                stack_ids,
                unassigned: _,
                uncommitted_files,
                uncommitted_hunks,
            } = self.inner;
            let commits_count = self.inner.commit_ids().len();
            writeln!(f, "workspace_and_remote_commits_count: {}", &commits_count)?;
            id_list_if_not_empty(f, "branches", self.inner.branch_ids().into_iter().sorted())?;
            id_list_if_not_empty(
                f,
                "uncommitted_files",
                uncommitted_files
                    .values()
                    .map(|uncommitted_file| uncommitted_file.short_id.clone())
                    .sorted(),
            )?;
            id_list_if_not_empty(f, "uncommitted_hunks", uncommitted_hunks.keys().sorted().cloned())?;
            id_list_if_not_empty(f, "stacks", stack_ids.values().map(|id| id.to_short_string()).sorted())?;
            Ok(())
        }
    }

    fn id_list_if_not_empty(f: &mut Formatter<'_>, field: &str, ids: impl Iterator<Item = String>) -> std::fmt::Result {
        let ids: Vec<_> = ids.collect();
        if !ids.is_empty() {
            writeln!(f, "{field}: [ {} ]", ids.join(", "))
        } else {
            Ok(())
        }
    }

    fn id_cmp(a: &CliId, b: &CliId) -> Ordering {
        a.to_short_string().cmp(&b.to_short_string())
    }
}
use util::{hunk_assignment, id, segment, stack, tree_change_addition};
