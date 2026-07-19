use anyhow::bail;
use bstr::BString;
use but_core::{ChangeId, ref_metadata::StackId};
use but_graph::workspace::Stack;
use but_hunk_assignment::HunkAssignment;
use but_testsupport::{hex_to_id, hunk_header};
use snapbox::{assert_data_eq, prelude::*};

use crate::{CliId, IdMap, id::id_usage::UintId};

#[test]
fn uint_id_from_short_id() {
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
}

#[test]
fn uint_id_to_short_id() {
    assert_eq!(UintId(0).to_short_id(), "g0");
    assert_eq!(UintId(19).to_short_id(), "z0");
    assert_eq!(UintId(700).to_short_id(), "gz");
    assert_eq!(UintId(719).to_short_id(), "zz");
    assert_eq!(UintId(720).to_short_id(), "g00");
    assert_eq!(UintId(1420).to_short_id(), "gz0");
    assert_eq!(UintId(26639).to_short_id(), "zzz");
    assert_eq!(
        UintId(26640).to_short_id(),
        "00",
        "too big always yields this"
    );
    assert_eq!(
        UintId(26641).to_short_id(),
        "00",
        "too big always yields this"
    );
}

#[test]
fn commit_id_works_with_two_or_more_characters() -> anyhow::Result<()> {
    let id1 = id(1);
    let stacks = vec![stack([segment("not-important", [id1], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ no ]


"#]]
    );
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let expected = [CliId::Commit {
        commit_id: id1,
        id: "0".to_string(),
        change_id: None,
    }];
    assert_eq!(
        id_map.parse("0", Box::new(changed_paths_fn))?,
        expected,
        "one character is sufficient to parse a commit ID"
    );
    assert_eq!(
        id_map.parse("01", Box::new(changed_paths_fn))?,
        expected,
        "two characters work too"
    );
    Ok(())
}

#[test]
fn commit_id_appearing_multiple_times() -> anyhow::Result<()> {
    let id1 = id(1);
    let stacks = vec![
        stack([segment("branch1", [id(2), id1], None, [])]),
        stack([segment("branch2", [id(3), id1], None, [])]),
    ];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // The commit should only appear once with a short ID.
    snapbox::assert_data_eq!(
        id_map.parse("01", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        id: "01",
        change_id: None,
    },
]

"#]]
    );
    Ok(())
}

#[test]
fn commit_ids_become_longer_if_ambiguous() -> anyhow::Result<()> {
    let id1 = hex_to_id("21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let id2 = hex_to_id("21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let id3 = hex_to_id("21bccccccccccccccccccccccccccccccccccccc");
    let stacks = vec![stack([segment("not-important", [id1, id2, id3], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 3
branches: [ no ]


"#]]
    );
    snapbox::assert_data_eq!(
        id_map.all_ids().to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
        id: "21a",
        change_id: None,
    },
    Commit {
        commit_id: Sha1(21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
        id: "21bb",
        change_id: None,
    },
    Commit {
        commit_id: Sha1(21bccccccccccccccccccccccccccccccccccccc),
        id: "21bc",
        change_id: None,
    },
    Branch {
        name: "not-important",
        id: "no",
        stack_id: None,
    },
]

"#]]
    );
    let ids_as_shown_by_consumers = id_map
        .all_ids()
        .iter()
        .map(|id| id.to_short_string())
        .collect::<Vec<_>>();
    snapbox::assert_data_eq!(
        ids_as_shown_by_consumers.to_debug(),
        snapbox::str![[r#"
[
    "21a",
    "21bb",
    "21bc",
    "no",
]

"#]]
    );
    Ok(())
}

#[test]
fn branches_work_with_single_character() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("f", [id(1)], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ g0 ]


"#]]
    );

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
fn branches_avoid_uncommitted_area_id() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("zza", [id(1)], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ za ]


"#]]
    );

    let expected = [CliId::Branch {
        name: "zza".into(),
        id: "za".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("za", Box::new(changed_paths_fn))?,
        expected,
        "avoids uncommitted area ID (zz)"
    );
    Ok(())
}

#[test]
fn branches_avoid_invalid_ids() -> anyhow::Result<()> {
    let stacks = vec![stack([
        segment("x-yz_/hi", [id(1)], None, []),
        segment("0ax", [id(2)], None, []),
    ])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 2
branches: [ ax, yz ]


"#]]
    );
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let expected = [CliId::Branch {
        name: "x-yz_/hi".into(),
        id: "yz".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("yz", Box::new(changed_paths_fn))?,
        expected,
        "avoids non-alphanumeric, taking first alphanumeric pair"
    );
    let expected = [CliId::Branch {
        name: "0ax".into(),
        id: "ax".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("ax", Box::new(changed_paths_fn))?,
        expected,
        "avoids hexdigit pair which can be confused with a commit ID"
    );
    Ok(())
}

#[test]
fn branches_avoid_uncommitted_filenames() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("ghij", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("gh", None), hunk_assignment("hi", None)];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ ij ]
uncommitted_files: [ n, y ]
uncommitted_hunks: [ n:q, y:q ]


"#]]
    );

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
fn branch_that_is_substring_of_other_substring_still_gets_id() -> anyhow::Result<()> {
    let stacks = vec![
        stack([segment("substring", [id(1)], None, [])]),
        stack([segment("supersubstring", [id(2)], None, [])]),
    ];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 2
branches: [ su, up ]


"#]]
    );

    let expected = [CliId::Branch {
        name: "substring".into(),
        id: "su".into(),
        stack_id: None,
    }];
    assert_eq!(id_map.parse("su", Box::new(changed_paths_fn))?, expected,);
    let expected = [CliId::Branch {
        name: "supersubstring".into(),
        id: "up".into(),
        stack_id: None,
    }];
    assert_eq!(
        id_map.parse("supersubstring", Box::new(changed_paths_fn))?,
        expected,
        "'su' would collide with substring, so 'up' is chosen"
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
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ h0 ]
uncommitted_files: [ k, r ]
uncommitted_hunks: [ k:q, r:q#0-2, r:q#1-2 ]
stacks: [ j0 ]


"#]]
    );
    snapbox::assert_data_eq!(
        id_map.all_ids().to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0202020202020202020202020202020202020202),
        id: "0",
        change_id: None,
    },
    Branch {
        name: "h0",
        id: "h0",
        stack_id: Some(
            00000000-0000-0000-0000-000000000001,
        ),
    },
    Stack {
        id: "j0",
        stack_id: 00000000-0000-0000-0000-000000000001,
    },
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "k",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "uncommitted2.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: true,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "k:q",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "uncommitted2.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,2", "+1,2"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
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
                        branch_ref_bytes: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                ],
            },
            is_entire_file: true,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:q#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,2", "+1,2"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:q#1-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-3,2", "+3,2"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
    );

    Ok(())
}

#[test]
fn ids_are_case_sensitive() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("h0", [id(10)], Some(id(9)), [])])];
    let hunk_assignments = vec![hunk_assignment("uncommitted.txt", None)];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(10) && parent_id == Some(id(9)) {
            vec![tree_change_addition("committed.txt")]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ h0 ]
uncommitted_files: [ l ]
uncommitted_hunks: [ l:q ]


"#]]
    );

    snapbox::assert_data_eq!(
        id_map.parse("0a", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
        id: "0",
        change_id: None,
    },
]

"#]]
    );
    assert_eq!(
        id_map.parse("0A", Box::new(changed_paths_fn))?,
        [],
        "the case matters for commits"
    );

    snapbox::assert_data_eq!(
        id_map.parse("h0", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    Branch {
        name: "h0",
        id: "h0",
        stack_id: None,
    },
]

"#]]
    );
    assert_eq!(
        id_map.parse("H0", Box::new(changed_paths_fn))?,
        [],
        "the case matters for branches"
    );

    snapbox::assert_data_eq!(
        id_map.parse("ln", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "l",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "uncommitted.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );
    assert_eq!(
        id_map.parse("LN", Box::new(changed_paths_fn))?,
        [],
        "the case matters for uncommitted files"
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("0a:zt", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    CommittedFile {
        commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
        path: "committed.txt",
        id: "0:z",
    },
]

"#]]
    );
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
    let hunk_assignments = vec![
        hunk_assignment("foo23", None),
        hunk_assignment("foo242", None),
    ];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(1) && parent_id.is_none() {
            vec![]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };

    // Ambiguous ID returns every possible match
    snapbox::assert_data_eq!(
        id_map.parse("kp", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpo",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "foo242",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: true,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpr",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "foo23",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    snapbox::assert_data_eq!(
        id_map.parse("kpo", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpo",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "foo242",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );
    snapbox::assert_data_eq!(
        id_map.parse("kpr", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpr",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "foo23",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    Ok(())
}

/// Branch names and short IDs can be prefixes of the reverse hex IDs of file paths for uncommitted
/// files.
///
/// The current solution to this is to only match against uncommitted file short IDs if there are no
/// other matches. So even on overlapping prefixes, we can still match out a branch short ID.
///
/// This needs to be extended further or reconsidered once commits can be matched via change ID, as
/// change IDs do not provide the convenience of being hexadecimal.
#[test]
fn uncommitted_files_disambiguate_with_branch() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("qsy", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("file", None)];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(1) && parent_id.is_none() {
            vec![]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };

    // Only the branch is returned when querying by short ID
    snapbox::assert_data_eq!(
        id_map.parse("qs", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    Branch {
        name: "qsy",
        id: "qs",
        stack_id: None,
    },
]

"#]]
    );

    // Still only the branch when querying by full name
    snapbox::assert_data_eq!(
        id_map.parse("qsy", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    Branch {
        name: "qsy",
        id: "qs",
        stack_id: None,
    },
]

"#]]
    );

    // More characters must be specified to get the file
    snapbox::assert_data_eq!(
        id_map.parse("qsyn", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "qsy",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "file",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    Ok(())
}

#[test]
fn longer_id_is_ok() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("foo23", None)];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(1) && parent_id.is_none() {
            vec![]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };

    // "kp" would be sufficient (see the "id" field in the output), but "kpr" works too
    snapbox::assert_data_eq!(
        id_map.parse("kpr", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "k",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "foo23",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    Ok(())
}

#[test]
fn reverse_hex_filename_is_its_own_id() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("klmxyz", None)];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(1) && parent_id.is_none() {
            vec![]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };

    // "klmxyz" does not have an autogenerated ID
    snapbox::assert_data_eq!(
        id_map.parse("kl", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "k",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "klmxyz",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    Ok(())
}

#[test]
fn branch_and_file_by_name() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![hunk_assignment("foo", None)];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(1) && parent_id.is_none() {
            vec![]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };

    // Both branches and uncommitted, uncommitted files match by name, and none
    // have priority over the other (i.e. if there is both a branch and a file
    // that matches, the result is ambiguous).
    snapbox::assert_data_eq!(
        id_map.parse("foo", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    Branch {
        name: "foo",
        id: "fo",
        stack_id: None,
    },
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "zo",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "foo",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    Ok(())
}

#[test]
fn colon_uncommitted_filename() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("gggg", [id(2)], None, [])])
    }];
    let hunk_assignments = vec![
        hunk_assignment("uncommitted", None),
        hunk_assignment("assigned", Some(StackId::from_number_for_testing(1))),
    ];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Short branch works
    snapbox::assert_data_eq!(
        id_map
            .parse("gg@{stack}:assigned", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "m",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "assigned",
                    stack_id: Some(
                        00000000-0000-0000-0000-000000000001,
                    ),
                    branch_ref_bytes: None,
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

"#]]
    );

    // Long branch works
    snapbox::assert_data_eq!(
        id_map
            .parse("gggg@{stack}:assigned", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "m",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "assigned",
                    stack_id: Some(
                        00000000-0000-0000-0000-000000000001,
                    ),
                    branch_ref_bytes: None,
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

"#]]
    );

    // Uncommitted works
    snapbox::assert_data_eq!(
        id_map
            .parse("zz:uncommitted", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "p",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "uncommitted",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    Ok(())
}

#[test]
fn uncommitted_path() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![
        hunk_assignment("prefixx", None),
        hunk_assignment("prefix/a", None),
        hunk_assignment("prefix/b", None),
    ];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Returns one ID with all hunk assignments
    snapbox::assert_data_eq!(
        id_map
            .parse("prefix/", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    PathPrefix {
        id: "prefix/",
        hunk_assignments: NonEmpty {
            head: (
                "y:q",
                HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "prefix/a",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
            ),
            tail: [
                (
                    "u:q",
                    HunkAssignment {
                        id: None,
                        hunk_header: None,
                        path: "",
                        path_bytes: "prefix/b",
                        stack_id: None,
                        branch_ref_bytes: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    },
                ),
            ],
        },
    },
]

"#]]
    );

    // If nothing matches, returns no ID
    snapbox::assert_data_eq!(
        id_map
            .parse("doesnotmatch/", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    Ok(())
}

#[test]
fn committed_files_are_deduplicated_by_commit_oid_path() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("branch", [id(2)], Some(id(1)), [])])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;

    // Simulate a changed_paths function that returns the same file twice
    // (which could happen due to a bug in the caller or data source)
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
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
    assert!(
        id_map
            .parse("02:file.txt", Box::new(changed_paths_fn))?
            .len()
            == 1
    );
    assert!(
        id_map
            .parse("02:other.txt", Box::new(changed_paths_fn))?
            .len()
            == 1
    );

    Ok(())
}

#[test]
fn committed_file_can_be_referenced_by_either_change_id_or_commit_id() {
    let id = id(1);
    let stacks = vec![stack([segment("branch", [id], None, [])])];
    let commit_id_to_change_id: gix::hashtable::HashMap<gix::ObjectId, ChangeId> = [
        (id, ChangeId::from_bytes("sv".as_bytes())), // swstzzzz...
    ]
    .into_iter()
    .collect();
    let id_map = IdMap::new(stacks, Vec::new(), commit_id_to_change_id).unwrap();

    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        if commit_id == id {
            Ok(vec![
                tree_change_addition("file.txt"),
                tree_change_addition("other_file.txt"),
            ])
        } else {
            anyhow::bail!("unexpected IDs {commit_id} {parent_id:?}");
        }
    };

    assert_data_eq!(
        id_map
            .parse("0:u", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    CommittedFile {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        path: "file.txt",
        id: "s:u",
    },
]

"#]]
    );
    assert_data_eq!(
        id_map
            .parse("s:u", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    CommittedFile {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        path: "file.txt",
        id: "s:u",
    },
]

"#]]
    );
}

#[test]
fn short_uncommitted_files_are_properly_reverse_hexed() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunk_assignments = vec![
        hunk_assignment("k", None),
        hunk_assignment("kl", None),
        hunk_assignment("klm", None),
    ];
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        Ok(if commit_id == id(1) && parent_id.is_none() {
            vec![]
        } else {
            bail!("unexpected IDs {commit_id} {parent_id:?}");
        })
    };

    snapbox::assert_data_eq!(
        id_map.parse("k", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ky",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "k",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    snapbox::assert_data_eq!(
        id_map.parse("kl", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "klx",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "kl",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );

    snapbox::assert_data_eq!(
        id_map.parse("klm", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "klml",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "klm",
                    stack_id: None,
                    branch_ref_bytes: None,
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

"#]]
    );
    Ok(())
}

#[test]
fn uncommitted_hunks_by_numeric_index() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
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
    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("uncommitted1.txt:#0", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:q#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,2", "+1,2"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
    );
    // Short IDs for the filename part also work; should return exactly the same as above
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:#0", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:q#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,2", "+1,2"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
    );
    // Files can also be accessed through zz
    snapbox::assert_data_eq!(
        id_map
            .parse("zz:uncommitted1.txt:#0", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:q#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,2", "+1,2"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
    );

    Ok(())
}

#[test]
fn uncommitted_hunks_by_id() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![
        HunkAssignment {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        // Same context lines as first hunk, but different diff
        HunkAssignment {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+there\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        // Same diff as first hunk, but different context lines
        HunkAssignment {
            hunk_header: Some(hunk_header("-60,6", "+62,7")),
            diff: Some(BString::new(
                "@@ -60,6 +62,7 @@\n 46\n 47\n 48\n+hello\n 49\n 50\n 51\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        hunk_assignment("hunk_without_diff.txt", None),
    ];

    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map.parse("ro:3", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:3",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map.parse("ro:f", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:f",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-23,6", "+24,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+there\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map.parse("ro:1", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:1",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-60,6", "+62,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -60,6 +62,7 @@\n 46\n 47\n 48\n+hello\n 49\n 50\n 51\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    // hunk without diff gets q identifier
    snapbox::assert_data_eq!(
        id_map
            .parse("hunk_without_diff.txt:q", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "w:q",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: "",
                    path_bytes: "hunk_without_diff.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
    );

    Ok(())
}

#[test]
fn uncommitted_hunks_by_id_increase_id_length_as_necessary() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![
        HunkAssignment {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        HunkAssignment {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
    ];

    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:78", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:78",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:79", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:79",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-23,6", "+24,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    Ok(())
}

/// If there are two hunks with IDs that have common leading characters, the short ID is lengthened
/// to disambiguate. If one of the hunks is subsequently committed, discarded or changed s.t. it no
/// longer clashes with the prefix of the other hunk, that other hunk's short ID is shortened to the
/// minimum necessary for uniqueness.
///
/// Because of this property, it's important that we can match the hunk by _any prefix_ of its full
/// ID. That way, if a short hunk ID is shortened, the longer version still works as a reference.
/// It's just as unique (sometimes more so) as any prefix of it, so there's no reason it wouldn't
/// work.
#[test]
fn uncommitted_hunks_overspecifying_id_prefix() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![HunkAssignment {
        hunk_header: Some(hunk_header("-1,6", "+1,7")),
        diff: Some(BString::new(
            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                .as_bytes()
                .to_vec(),
        )),
        ..hunk_assignment("uncommitted1.txt", None)
    }];

    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:78", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:7",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    Ok(())
}

/// Same reasoning as [`uncommitted_hunks_can_be_referenced_by_longer_prefix_than_short_id`], but
/// including collision disambiguation.
#[test]
fn uncommitted_hunks_overspecifying_id_prefix_with_collision_disambiguation() -> anyhow::Result<()>
{
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![
        HunkAssignment {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        // Same context lines as first hunk, but different diff
        HunkAssignment {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
    ];

    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3eeb#0-2", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:3#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn underspecifying_hunk_ids() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![
        HunkAssignment {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        HunkAssignment {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        HunkAssignment {
            hunk_header: Some(hunk_header("-33,6", "+35,7")),
            diff: Some(BString::new(
                "@@ -33,6 +35,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
    ];

    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Underspecifying with just first character finds all hunks
    snapbox::assert_data_eq!(
        id_map.parse("ro:7", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:78#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:79",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-23,6", "+24,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:78#1-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-33,6", "+35,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -33,6 +35,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    // Underspecifying with collision index only finds hunk with precisely matching collision index.
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:7#0-2", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:78#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    // An entirely empty prefix matches nothing
    snapbox::assert_data_eq!(
        id_map.parse("ro:", Box::new(changed_paths_fn))?.to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    // Omitting only specifying collision index matches nothing - we don't allow omitting the prefix
    // unless you are explicitly indexing into the file's hunks
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:#0-2", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    Ok(())
}

#[test]
fn uncommitted_hunks_by_id_collision_handling() -> anyhow::Result<()> {
    let stacks = vec![Stack {
        id: Some(StackId::from_number_for_testing(1)),
        ..stack([segment("foo", [id(2)], Some(id(1)), [])])
    }];
    let hunk_assignments = vec![
        HunkAssignment {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
        HunkAssignment {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk_assignment("uncommitted1.txt", None)
        },
    ];

    let id_map = IdMap::new(stacks, hunk_assignments, gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3#0-2", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:3#0-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-1,6", "+1,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3#1-2", Box::new(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "r:3#1-2",
            hunk_assignments: NonEmpty {
                head: HunkAssignment {
                    id: None,
                    hunk_header: Some(
                        HunkHeader("-23,6", "+24,7"),
                    ),
                    path: "",
                    path_bytes: "uncommitted1.txt",
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: Some(
                        "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                    ),
                },
                tail: [],
            },
            is_entire_file: false,
        },
    ),
]

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn commit_matches_are_deduplicated_by_commit_oid() -> anyhow::Result<()> {
    let commit_id = id(2);
    let stacks = vec![stack([segment(
        "branch",
        [commit_id],
        Some(id(1)),
        [commit_id],
    )])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let matches = id_map.parse("02", Box::new(changed_paths_fn))?;
    assert_eq!(matches.len(), 1);
    assert!(
        matches
            .iter()
            .any(|m| matches!(m, CliId::Commit { commit_id: id, .. } if *id == commit_id)),
        "same commit reachable through local and remote views should not be ambiguous"
    );

    Ok(())
}

#[test]
fn dedupe_does_not_hide_ambiguity_between_distinct_commits() -> anyhow::Result<()> {
    let id1 = hex_to_id("21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let id2 = hex_to_id("21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let stacks = vec![stack([segment("branch", [id1, id2], None, [])])];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let matches = id_map.parse("21", Box::new(changed_paths_fn))?;
    assert_eq!(
        matches.len(),
        2,
        "distinct commits sharing a prefix must remain ambiguous"
    );
    assert!(
        matches
            .iter()
            .any(|m| matches!(m, CliId::Commit { commit_id, .. } if *commit_id == id1))
    );
    assert!(
        matches
            .iter()
            .any(|m| matches!(m, CliId::Commit { commit_id, .. } if *commit_id == id2))
    );

    Ok(())
}

#[test]
fn dedupe_does_not_hide_ambiguity_between_branches_in_different_stacks() -> anyhow::Result<()> {
    let stacks = vec![
        Stack {
            id: Some(StackId::from_number_for_testing(1)),
            ..stack([segment("foo", [id(1)], None, [])])
        },
        Stack {
            id: Some(StackId::from_number_for_testing(2)),
            ..stack([segment("foo", [id(2)], None, [])])
        },
    ];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let matches = id_map.parse("foo", Box::new(changed_paths_fn))?;
    assert_eq!(
        matches.len(),
        2,
        "same branch name across different stacks must remain ambiguous"
    );
    assert!(
        matches
            .iter()
            .any(|m| matches!(m, CliId::Branch { name, stack_id, .. } if name == "foo" && *stack_id == Some(StackId::from_number_for_testing(1))))
    );
    assert!(
        matches
            .iter()
            .any(|m| matches!(m, CliId::Branch { name, stack_id, .. } if name == "foo" && *stack_id == Some(StackId::from_number_for_testing(2))))
    );

    Ok(())
}

#[test]
fn dedupe_does_not_hide_ambiguity_between_unmanaged_branches_with_same_name() -> anyhow::Result<()>
{
    use std::collections::HashSet;

    let stacks = vec![
        stack([segment("foo", [id(1)], None, [])]),
        stack([segment("foo", [id(2)], None, [])]),
    ];
    let id_map = IdMap::new(stacks, Vec::new(), gix::hashtable::HashMap::default())?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let matches = id_map.parse("foo", Box::new(changed_paths_fn))?;
    assert_eq!(
        matches.len(),
        2,
        "same branch name across unmanaged stacks must remain ambiguous"
    );
    assert!(matches.iter().all(
        |m| matches!(m, CliId::Branch { name, stack_id, .. } if name == "foo" && stack_id.is_none())
    ));

    let unique_ids: HashSet<_> = matches.iter().map(CliId::to_short_string).collect();
    assert_eq!(
        unique_ids.len(),
        2,
        "the two unmanaged branches must stay distinct"
    );
    Ok(())
}

#[test]
fn find_commits_by_change_id() {
    let id1 = id(1);
    let id2 = id(2);
    let stacks = vec![stack([segment("not-important", [id1, id2], None, [])])];

    let commit_id_to_change_id: gix::hashtable::HashMap<gix::ObjectId, ChangeId> = [
        (id1, ChangeId::from_bytes("sv".as_bytes())), // swstzzzz...
        (id2, ChangeId::from_bytes("sx".as_bytes())), // swsrzzzz...
    ]
    .into_iter()
    .collect();

    let id_map = IdMap::new(stacks, Vec::new(), commit_id_to_change_id).unwrap();
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 2
branches: [ no ]


"#]]
    );
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Should match both commits if we use a common prefix
    snapbox::assert_data_eq!(
        id_map
            .parse("sws", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        id: "01",
        change_id: Some(
            "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
    Commit {
        commit_id: Sha1(0202020202020202020202020202020202020202),
        id: "02",
        change_id: Some(
            "swsrzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("swst", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        id: "01",
        change_id: Some(
            "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("swsr", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0202020202020202020202020202020202020202),
        id: "02",
        change_id: Some(
            "swsrzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
]

"#]],
    )
}

#[test]
fn uncommitted_selector_is_not_shadowed_by_commit_change_id() -> anyhow::Result<()> {
    let changed_paths_fn = || {
        Box::new(
            |commit_id: gix::ObjectId,
             parent_id: Option<gix::ObjectId>|
             -> anyhow::Result<Vec<but_core::TreeChange>> {
                bail!("unexpected IDs {commit_id} {parent_id:?}");
            },
        )
    };

    // Discover the ID the file gets while no commit competes with it — the ID
    // an agent copies from `but diff` before committing.
    let commitless = IdMap::new(
        vec![stack([segment("not-important", [], None, [])])],
        vec![hunk_assignment("README.md", None)],
        gix::hashtable::HashMap::default(),
    )?;
    let file_id = commitless
        .all_ids()
        .into_iter()
        .find_map(|cli_id| match cli_id {
            CliId::UncommittedHunkOrFile(uncommitted) => Some(uncommitted.id.clone()),
            _ => None,
        })
        .expect("one uncommitted file");

    // A commit is created whose random change ID starts with that file ID.
    let id1 = id(1);
    let colliding_change_id = ChangeId::from(BString::from(format!(
        "{file_id}{}",
        "z".repeat(32 - file_id.len())
    )));
    let commit_id_to_change_id: gix::hashtable::HashMap<gix::ObjectId, ChangeId> =
        [(id1, colliding_change_id)].into_iter().collect();
    let id_map = IdMap::new(
        vec![stack([segment("not-important", [id1], None, [])])],
        vec![hunk_assignment("README.md", None)],
        commit_id_to_change_id,
    )?;

    // In the full namespace the commit shadows the previously issued file ID.
    let full = id_map.parse(&file_id, changed_paths_fn())?;
    assert!(
        matches!(full.as_slice(), [CliId::Commit { .. }]),
        "the commit change ID shadows the file ID in the full namespace: {full:?}"
    );

    // Scoped to uncommitted files, the same selector still finds the file.
    let scoped = id_map.parse_uncommitted(&file_id, changed_paths_fn())?;
    match scoped.as_slice() {
        [CliId::UncommittedHunkOrFile(uncommitted)] => {
            assert_eq!(
                uncommitted.hunk_assignments.first().path_bytes,
                "README.md",
                "the selector resolves to the file it was issued for"
            );
        }
        other => panic!("expected the uncommitted file, got {other:?}"),
    }

    // Hunk selectors under the file keep working too.
    let hunk = id_map.parse_uncommitted(&format!("{file_id}:q"), changed_paths_fn())?;
    assert!(
        matches!(hunk.as_slice(), [CliId::UncommittedHunkOrFile(_)]),
        "hunk selector resolves in the scoped namespace: {hunk:?}"
    );

    Ok(())
}

#[test]
fn uncommitted_scope_resolves_a_file_literally_named_zz() -> anyhow::Result<()> {
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    let id_map = IdMap::new(
        vec![stack([segment("not-important", [], None, [])])],
        vec![hunk_assignment("zz", None)],
        gix::hashtable::HashMap::default(),
    )?;

    // The full parser returns the filename match before considering the `zz`
    // sentinel; the scoped parser must agree instead of reporting an
    // ambiguity the full parser does not have.
    let scoped = id_map.parse_uncommitted("zz", Box::new(changed_paths_fn))?;
    match scoped.as_slice() {
        [CliId::UncommittedHunkOrFile(uncommitted)] => {
            assert_eq!(uncommitted.hunk_assignments.first().path_bytes, "zz");
        }
        other => panic!("expected exactly the file named zz, got {other:?}"),
    }
    Ok(())
}

#[test]
fn uncommitted_scope_does_not_prefix_match_a_branch_short_id() -> anyhow::Result<()> {
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    // File "foo242" gets reverse-hex ID "kpo…"; a branch literally named "kp"
    // takes short ID "kp", which the ID assigner deliberately allows to be a
    // prefix of the file's ID (branches win in the full namespace).
    let id_map = IdMap::new(
        vec![stack([segment("kp", [id(1)], None, [])])],
        vec![hunk_assignment("foo242", None)],
        gix::hashtable::HashMap::default(),
    )?;

    let full = id_map.parse("kp", Box::new(changed_paths_fn))?;
    assert!(
        matches!(full.as_slice(), [CliId::Branch { .. }]),
        "precondition: the full namespace resolves 'kp' to the branch: {full:?}"
    );

    // The scoped parser must NOT silently resolve the displayed branch ID to
    // the file by hex-prefix accident — an empty result lets callers produce
    // the targeted "is a branch" error via their full-namespace fallback.
    let scoped = id_map.parse_uncommitted("kp", Box::new(changed_paths_fn))?;
    assert_eq!(
        scoped,
        vec![],
        "a displayed branch ID never resolves to a file in the uncommitted scope"
    );

    // A longer prefix that no branch owns still resolves the file.
    let scoped = id_map.parse_uncommitted("kpo", Box::new(changed_paths_fn))?;
    assert!(
        matches!(scoped.as_slice(), [CliId::UncommittedHunkOrFile(_)]),
        "file prefixes beyond the branch ID keep resolving: {scoped:?}"
    );
    Ok(())
}

#[test]
fn change_ids_are_disambiguated_on_collision() {
    let id1 = id(1);
    let id2 = id(2);
    let stacks = vec![stack([segment("not-important", [id1, id2], None, [])])];

    let commit_id_to_change_id: gix::hashtable::HashMap<gix::ObjectId, ChangeId> = [
        (id1, ChangeId::from_bytes("sv".as_bytes())), // swstzzzz...
        (id2, ChangeId::from_bytes("sv".as_bytes())), // swstzzzz...
    ]
    .into_iter()
    .collect();

    let id_map = IdMap::new(stacks, Vec::new(), commit_id_to_change_id).unwrap();
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 2
branches: [ no ]


"#]]
    );
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Should match both commits if we use a common prefix
    snapbox::assert_data_eq!(
        id_map
            .parse("sws", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        id: "01",
        change_id: Some(
            "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
    Commit {
        commit_id: Sha1(0202020202020202020202020202020202020202),
        id: "02",
        change_id: Some(
            "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("s#0", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0101010101010101010101010101010101010101),
        id: "01",
        change_id: Some(
            "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("s#1", Box::new(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit_id: Sha1(0202020202020202020202020202020202020202),
        id: "02",
        change_id: Some(
            "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ),
    },
]

"#]],
    )
}

mod util {
    use std::{cmp::Ordering, fmt::Formatter};

    use anyhow::bail;
    use bstr::BString;
    use but_core::ref_metadata::StackId;
    use but_graph::workspace::{Stack, StackCommit, StackSegment};
    use but_hunk_assignment::HunkAssignment;
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
    ) -> StackSegment {
        fn commit(id: gix::ObjectId, parent_id: Option<gix::ObjectId>) -> StackCommit {
            StackCommit {
                id,
                parent_ids: parent_id.into_iter().collect::<Vec<gix::ObjectId>>(),
                refs: Vec::new(),
                flags: Default::default(),
            }
        }

        let ref_info = Some(but_graph::RefInfo {
            ref_name: gix::refs::FullName::try_from(format!("refs/heads/{shortened_branch_name}"))
                .expect("could not generate ref name"),
            commit_id: local_commit_ids.first().copied(),
            worktree: None,
        });
        let mut commits: Vec<StackCommit> = Vec::new();
        for (i, id) in local_commit_ids.iter().enumerate() {
            let parent_id = local_commit_ids.get(i + 1).or(base.as_ref());
            commits.push(commit(*id, parent_id.cloned()));
        }
        let mut commits_on_remote: Vec<StackCommit> = Vec::new();
        for id in remote_commit_ids {
            commits_on_remote.push(commit(id, None))
        }
        StackSegment {
            ref_info,
            remote_tracking_ref_name: None,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            id: Default::default(),
            commits,
            commits_outside: None,
            base,
            base_segment_id: None,
            commits_by_segment: Vec::new(),
            commits_on_remote,
            metadata: None,
            is_entrypoint: false,
        }
    }

    pub fn stack<const N: usize>(segments: [StackSegment; N]) -> Stack {
        Stack {
            id: None,
            segments: segments.into_iter().collect::<Vec<StackSegment>>(),
        }
    }

    pub fn hunk_assignment(path: &str, stack_id: Option<StackId>) -> HunkAssignment {
        HunkAssignment {
            id: None,
            hunk_header: None,
            path: String::new(),
            path_bytes: BString::from(path),
            stack_id,
            branch_ref_bytes: None,
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

        /// Return a sorted list of all CliIds we can provide, excluding uncommitted.
        pub fn all_ids(&self) -> Vec<CliId> {
            let IdMap {
                indexed_stacks: _,
                stack_ids,
                uncommitted: _,
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
                uncommitted: _,
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
            id_list_if_not_empty(
                f,
                "uncommitted_hunks",
                uncommitted_hunks.keys().sorted().cloned(),
            )?;
            id_list_if_not_empty(
                f,
                "stacks",
                stack_ids.values().map(|id| id.to_short_string()).sorted(),
            )?;
            Ok(())
        }
    }

    fn id_list_if_not_empty(
        f: &mut Formatter<'_>,
        field: &str,
        ids: impl Iterator<Item = String>,
    ) -> std::fmt::Result {
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
