//! Explicit low-level tests for the IdMap.
//!
//! Some more complex state is very laborious and error prone to setup, such as for resolving
//! committed and uncommitted hunks. This is sparsely tested here. There are complimentary
//! `but`-level tests in `crates/but/tests/but/command/expand.rs` that validate this functionality
//! more closely.

use anyhow::bail;
use bstr::BString;
use but_core::{ChangeId, ref_metadata::StackId};
use but_graph::workspace::Stack;
use but_testsupport::{hex_to_id, hunk_header};
use snapbox::{assert_data_eq, prelude::*};

use crate::{
    CliId, IdMap,
    args::atoms::CliIdArg,
    id::{BranchId, ChangesInCommit, CommitId, OLD_UNCOMMITTED, UNCOMMITTED, id_usage::UintId},
    utils::change_source::ChangeSourceId,
};

struct TestChanges<T>(T);

impl<T> ChangesInCommit for TestChanges<T>
where
    T: Fn(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>,
{
    fn tree_changes(
        &self,
        commit_id: gix::ObjectId,
        parent_id: Option<gix::ObjectId>,
    ) -> anyhow::Result<Vec<but_core::TreeChange>> {
        self.0(commit_id, parent_id)
    }

    fn patch_for_tree_change(
        &self,
        _tree_change: &but_core::TreeChange,
        _context_lines: u32,
    ) -> anyhow::Result<Option<but_core::UnifiedPatch>> {
        unimplemented!("Test patch resolution not implemented!")
    }
}

#[test]
fn committed_hunk_equality() {
    let committed_hunk = |commit_id| super::CommittedHunk {
        committed_file: super::CommittedFileId {
            commit_id,
            path: BString::from("file.txt"),
            change_id: None,
        },
        id: "ignored by equality".into(),
        hunk: hunk("file.txt"),
    };

    assert_eq!(
        committed_hunk(id(1)),
        committed_hunk(id(1)),
        "same hunk in same commit is equal to itself"
    );

    assert_ne!(
        committed_hunk(id(1)),
        committed_hunk(id(2)),
        "identical hunks in different commits are not equal"
    );
}

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
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        commit: CommitId {
            commit_id: id1,
            change_id: None,
        },
        id: "0".to_string(),
    }];
    assert_eq!(
        id_map.parse("0", &TestChanges(changed_paths_fn))?,
        expected,
        "one character is sufficient to parse a commit ID"
    );
    assert_eq!(
        id_map.parse("01", &TestChanges(changed_paths_fn))?,
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
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // The commit should only appear once with a short ID.
    snapbox::assert_data_eq!(
        id_map
            .parse("01", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            change_id: None,
        },
        id: "01",
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
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        commit: CommitId {
            commit_id: Sha1(21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
            change_id: None,
        },
        id: "21a",
    },
    Commit {
        commit: CommitId {
            commit_id: Sha1(21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
            change_id: None,
        },
        id: "21bb",
    },
    Commit {
        commit: CommitId {
            commit_id: Sha1(21bccccccccccccccccccccccccccccccccccccc),
            change_id: None,
        },
        id: "21bc",
    },
    Branch(
        BranchId {
            name: "not-important",
            id: "no",
            stack_id: None,
        },
    ),
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
fn exact_branch_short_id_takes_priority() {
    let commit_id = id(1);
    let id_map = IdMap::new(
        vec![stack([segment("tp-branch", [commit_id], None, [])])],
        vec![],
        [(commit_id, ChangeId::from(BString::from("tpm")))]
            .into_iter()
            .collect(),
        Default::default(),
        3,
    )
    .unwrap();

    assert_eq!(
        id_map
            .parse("tp", &TestChanges(|_, _| unreachable!()))
            .unwrap(),
        [CliId::Branch(BranchId {
            name: "tp-branch".into(),
            id: "tp".into(),
            stack_id: None,
        })],
        "exact branch short ID wins over change ID prefix"
    );
}

#[test]
fn branches_work_with_single_character() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("f", [id(1)], None, [])])];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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

    let expected = [CliId::Branch(BranchId {
        name: "f".into(),
        id: "g0".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("f", &TestChanges(changed_paths_fn))?,
        expected,
        "it's OK to have a CliID that is longer, but it would be up to the UI to not show them"
    );
    assert_eq!(
        id_map.parse("g0", &TestChanges(changed_paths_fn))?,
        expected,
        "the ID also works"
    );
    Ok(())
}

#[test]
fn parsing_retired_uncommitted_area_errors() {
    let id_map = IdMap::new(
        Vec::new(),
        Vec::new(),
        Default::default(),
        Default::default(),
        3,
    )
    .unwrap();
    let parse_error = id_map
        .parse(
            OLD_UNCOMMITTED,
            &TestChanges(|_, _| bail!("shouldn't be used")),
        )
        .unwrap_err();

    assert_eq!(
        parse_error.to_string(),
        "The uncommitted area has been renamed from 'zz' to '@'. Repeat the command with '@' instead to proceed."
    )
}

#[test]
fn branches_avoid_retired_uncommitted_area_id() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("zza", [id(1)], None, [])])];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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

    let expected = [CliId::Branch(BranchId {
        name: "zza".into(),
        id: "za".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("za", &TestChanges(changed_paths_fn))?,
        expected,
        "avoids retired uncommitted area ID (zz)"
    );
    Ok(())
}

#[test]
fn uncommitted_files_avoid_retired_uncommitted_area_id() -> anyhow::Result<()> {
    // SHA-1("file-1594") starts with 0x00, which maps to reverse-hex `zz`.
    let id_map = IdMap::new(
        Vec::new(),
        vec![source_changes(
            ChangeSourceId::Head,
            vec![hunk("file-1594")],
        )],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 0
uncommitted_files: [ zzs ]
uncommitted_hunks: [ zzs:e ]


"#]]
    );
    Ok(())
}

#[test]
fn branches_avoid_invalid_ids() -> anyhow::Result<()> {
    let stacks = vec![stack([
        segment("x-yz_/hi", [id(1)], None, []),
        segment("0ax", [id(2)], None, []),
    ])];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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

    let expected = [CliId::Branch(BranchId {
        name: "x-yz_/hi".into(),
        id: "yz".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("yz", &TestChanges(changed_paths_fn))?,
        expected,
        "avoids non-alphanumeric, taking first alphanumeric pair"
    );
    let expected = [CliId::Branch(BranchId {
        name: "0ax".into(),
        id: "ax".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("ax", &TestChanges(changed_paths_fn))?,
        expected,
        "avoids hexdigit pair which can be confused with a commit ID"
    );
    Ok(())
}

#[test]
fn branches_avoid_uncommitted_filenames() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("ghij", [id(1)], None, [])])];
    let hunks = vec![hunk("gh"), hunk("hi")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
uncommitted_files: [ nx, yz ]
uncommitted_hunks: [ nx:e, yz:e ]


"#]]
    );

    let expected = [CliId::Branch(BranchId {
        name: "ghij".into(),
        id: "ij".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("ghij", &TestChanges(changed_paths_fn))?,
        expected,
        "avoids 'gh' and 'hi', which conflict with filenames"
    );
    Ok(())
}

#[test]
fn many_uncommitted_files_do_not_exhaust_generated_ids() -> anyhow::Result<()> {
    const FILE_COUNT: usize = 26_641;
    let stacks = vec![
        Stack {
            id: Some(StackId::from_number_for_testing(1)),
            ..stack([segment("0", [id(1)], None, [])])
        },
        Stack {
            id: Some(StackId::from_number_for_testing(2)),
            ..stack([segment("1", [id(2)], None, [])])
        },
    ];
    let hunks = (0..FILE_COUNT)
        .map(|index| hunk(&format!("untracked-{index}")))
        .collect();

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    assert_eq!(
        id_map.uncommitted_files.len(),
        FILE_COUNT,
        "all uncommitted files receive path-derived IDs"
    );
    let file_id = id_map
        .uncommitted_files
        .values()
        .next()
        .expect("at least one uncommitted file")
        .short_id
        .clone();
    // The issued file ID resolves to its file.
    snapbox::assert_data_eq!(
        (
            &file_id,
            id_map.parse(&file_id, &TestChanges(|_, _| unreachable!()))?
        )
            .to_debug(),
        snapbox::str![[r#"
(
    "kkkl",
    [
        UncommittedHunkOrFile(
            UncommittedHunkOrFile {
                id: "kkkl",
                hunks: NonEmpty {
                    head: IdAndHunk {
                        id: "kkkl:e",
                        hunk: SingleHunk {
                            hunk_header: None,
                            path: "untracked-9681",
                            diff: None,
                        },
                    },
                    tail: [],
                },
                is_entire_file: true,
                source: Head,
            },
        ),
    ],
)

"#]]
    );

    // Both branches and both stacks keep distinct generated IDs that resolve to themselves.
    let real_ids = id_map
        .branch_ids()
        .into_iter()
        .chain(id_map.stack_ids.values().map(CliId::to_short_string))
        .map(|real_id| {
            id_map
                .parse(&real_id, &TestChanges(|_, _| unreachable!()))
                .map(|resolved| (real_id, resolved))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    snapbox::assert_data_eq!(
        real_ids.to_debug(),
        snapbox::str![[r#"
[
    (
        "g0",
        [
            Branch(
                BranchId {
                    name: "0",
                    id: "g0",
                    stack_id: Some(
                        00000000-0000-0000-0000-000000000001,
                    ),
                },
            ),
        ],
    ),
    (
        "h0",
        [
            Branch(
                BranchId {
                    name: "1",
                    id: "h0",
                    stack_id: Some(
                        00000000-0000-0000-0000-000000000002,
                    ),
                },
            ),
        ],
    ),
    (
        "i0",
        [
            Stack {
                id: "i0",
                stack_id: 00000000-0000-0000-0000-000000000001,
            },
        ],
    ),
    (
        "j0",
        [
            Stack {
                id: "j0",
                stack_id: 00000000-0000-0000-0000-000000000002,
            },
        ],
    ),
]

"#]]
    );
    Ok(())
}

#[test]
fn branch_that_is_substring_of_other_substring_still_gets_id() -> anyhow::Result<()> {
    let stacks = vec![
        stack([segment("substring", [id(1)], None, [])]),
        stack([segment("supersubstring", [id(2)], None, [])]),
    ];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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

    let expected = [CliId::Branch(BranchId {
        name: "substring".into(),
        id: "su".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("su", &TestChanges(changed_paths_fn))?,
        expected,
    );
    let expected = [CliId::Branch(BranchId {
        name: "supersubstring".into(),
        id: "up".into(),
        stack_id: None,
    })];
    assert_eq!(
        id_map.parse("supersubstring", &TestChanges(changed_paths_fn))?,
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,2", "+1,2")),
            ..hunk("uncommitted1.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-3,2", "+3,2")),
            ..hunk("uncommitted1.txt")
        },
        hunk("uncommitted2.txt"),
    ];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 1
branches: [ h0 ]
uncommitted_files: [ kv, ro ]
uncommitted_hunks: [ kv:e, ro:e#0-2, ro:e#1-2 ]
stacks: [ j0 ]


"#]]
    );
    snapbox::assert_data_eq!(
        id_map.all_ids().to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            change_id: None,
        },
        id: "0",
    },
    Branch(
        BranchId {
            name: "h0",
            id: "h0",
            stack_id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
        },
    ),
    Stack {
        id: "j0",
        stack_id: 00000000-0000-0000-0000-000000000001,
    },
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kv",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kv:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "uncommitted2.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kv:e",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kv:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "uncommitted2.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:e#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "uncommitted1.txt",
                        diff: None,
                    },
                },
                tail: [
                    IdAndHunk {
                        id: "ro:e#1-2",
                        hunk: SingleHunk {
                            hunk_header: Some(
                                HunkHeader("-3,2", "+3,2"),
                            ),
                            path: "uncommitted1.txt",
                            diff: None,
                        },
                    },
                ],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:e#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:e#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "uncommitted1.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:e#1-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:e#1-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-3,2", "+3,2"),
                        ),
                        path: "uncommitted1.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
    );

    Ok(())
}

#[test]
fn uncommitted_file_to_id_qualifies_hunk_ids() -> anyhow::Result<()> {
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,2", "+1,2")),
            ..hunk("uncommitted.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-3,2", "+3,2")),
            ..hunk("uncommitted.txt")
        },
    ];
    let id_map = IdMap::new(
        Vec::new(),
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let uncommitted_file = id_map
        .uncommitted_files
        .values()
        .next()
        .expect("the map contains the uncommitted file");
    let expected_ids = uncommitted_file
        .hunks()
        .iter()
        .map(|(hunk_id, _)| format!("{}:{hunk_id}", uncommitted_file.short_id))
        .collect::<Vec<_>>();
    let CliId::UncommittedHunkOrFile(uncommitted) = uncommitted_file.to_id() else {
        panic!("an uncommitted file converts to an uncommitted CLI ID");
    };
    let actual_ids = uncommitted
        .hunks
        .iter()
        .map(|id_and_hunk| id_and_hunk.id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        actual_ids, expected_ids,
        "nested hunk IDs include their file ID"
    );
    Ok(())
}

#[test]
fn ids_are_case_sensitive() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("h0", [id(10)], Some(id(9)), [])])];
    let hunks = vec![hunk("uncommitted.txt")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
uncommitted_files: [ ln ]
uncommitted_hunks: [ ln:e ]


"#]]
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("0a", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
            change_id: None,
        },
        id: "0",
    },
]

"#]]
    );
    assert_eq!(
        id_map.parse("0A", &TestChanges(changed_paths_fn))?,
        [],
        "the case matters for commits"
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("h0", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Branch(
        BranchId {
            name: "h0",
            id: "h0",
            stack_id: None,
        },
    ),
]

"#]]
    );
    assert_eq!(
        id_map.parse("H0", &TestChanges(changed_paths_fn))?,
        [],
        "the case matters for branches"
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ln", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ln",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ln:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "uncommitted.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );
    assert_eq!(
        id_map.parse("LN", &TestChanges(changed_paths_fn))?,
        [],
        "the case matters for uncommitted files"
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("0a:zt", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    CommittedFile {
        committed_file: CommittedFileId {
            commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
            path: "committed.txt",
            change_id: None,
        },
        id: "0:z",
    },
]

"#]]
    );
    assert_eq!(
        id_map.parse("0a:ZT", &TestChanges(changed_paths_fn))?,
        [],
        "the case matters for committed files"
    );

    Ok(())
}

#[test]
fn uncommitted_files_disambiguate_between_themselves() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunks = vec![hunk("foo23"), hunk("foo242")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        id_map
            .parse("kp", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpo",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kpo:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo242",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpr",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kpr:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo23",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("kpo", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpo",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kpo:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo242",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );
    snapbox::assert_data_eq!(
        id_map
            .parse("kpr", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpr",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kpr:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo23",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    Ok(())
}

/// The same path dirty in several checkouts gets one distinct ID per checkout,
/// formatted exactly like any other uncommitted file ID.
///
/// This is what the source mixed into [`super::create_reverse_hex_id`] buys: without
/// it all three would hash alike and two of them would be unreachable.
#[test]
fn same_path_in_several_sources_gets_distinct_ids() -> anyhow::Result<()> {
    let id_map = IdMap::new(
        Vec::new(),
        vec![
            source_changes(ChangeSourceId::Head, vec![hunk("file")]),
            source_changes(ChangeSourceId::Worktree("wt-a".into()), vec![hunk("file")]),
            source_changes(ChangeSourceId::Worktree("wt-b".into()), vec![hunk("file")]),
        ],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    let ids: Vec<_> = id_map
        .uncommitted_files
        .values()
        .map(|file| (file.short_id.clone(), file.source.clone()))
        .collect();

    // Three IDs, none of them carrying a collision index: the source separates
    // them by hash, so no `#N` disambiguation is needed.
    snapbox::assert_data_eq!(
        ids.to_debug(),
        snapbox::str![[r#"
[
    (
        "pw",
        Worktree(
            "wt-b",
        ),
    ),
    (
        "qs",
        Head,
    ),
    (
        "rp",
        Worktree(
            "wt-a",
        ),
    ),
]

"#]]
    );

    Ok(())
}

/// A linked worktree gets its own CLI ID, resolves by name, and scopes a
/// filename to its own checkout the way `@` does for the main worktree.
#[test]
fn worktree_container_id() -> anyhow::Result<()> {
    let id_map = IdMap::new(
        Vec::new(),
        vec![
            source_changes(ChangeSourceId::Head, vec![hunk("file")]),
            source_changes(ChangeSourceId::Worktree("wt-a".into()), vec![hunk("file")]),
            // A worktree without changes still gets an ID, so it can be listed.
            source_changes(ChangeSourceId::Worktree("wt-b".into()), Vec::new()),
        ],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let by_name = id_map.parse("wt-a", &TestChanges(changed_paths_fn))?;
    snapbox::assert_data_eq!(
        by_name.to_debug(),
        snapbox::str![[r#"
[
    Worktree {
        id: "wt",
        name: "wt-a",
    },
]

"#]]
    );

    // The short ID resolves to the same worktree.
    let short_id = by_name[0].to_short_string();
    snapbox::assert_data_eq!(
        id_map
            .parse(&short_id, &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Worktree {
        id: "wt",
        name: "wt-a",
    },
]

"#]]
    );

    // Worktree short IDs require an exact match, so a one-character prefix finds nothing.
    snapbox::assert_data_eq!(
        id_map
            .parse(&short_id[..1], &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    // `<worktree>:<path>` disambiguates a path that is dirty in several checkouts,
    // keeping the named checkout's copy.
    snapbox::assert_data_eq!(
        id_map
            .parse("wt-a:file", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "rp",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "rp:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "file",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Worktree(
                "wt-a",
            ),
        },
    ),
]

"#]]
    );

    // The container expands to every file in that checkout, which is what
    // `but commit <worktree>` commits.
    snapbox::assert_data_eq!(
        id_map
            .uncommitted_files_in(&ChangeSourceId::Worktree("wt-a".into()))
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile {
        id: "rp",
        hunks: NonEmpty {
            head: IdAndHunk {
                id: "rp:e",
                hunk: SingleHunk {
                    hunk_header: None,
                    path: "file",
                    diff: None,
                },
            },
            tail: [],
        },
        is_entire_file: true,
        source: Worktree(
            "wt-a",
        ),
    },
]

"#]]
    );
    // A clean worktree expands to nothing.
    snapbox::assert_data_eq!(
        id_map
            .uncommitted_files_in(&ChangeSourceId::Worktree("wt-b".into()))
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    Ok(())
}

/// `<worktree>:@` names that worktree's uncommitted area, the way `@` names the
/// main worktree's, and is distinct from the worktree reference itself.
#[test]
fn worktree_uncommitted_area_id() -> anyhow::Result<()> {
    let id_map = IdMap::new(
        Vec::new(),
        vec![
            source_changes(ChangeSourceId::Head, vec![hunk("file")]),
            source_changes(ChangeSourceId::Worktree("wt-a".into()), vec![hunk("file")]),
        ],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let by_short_id = id_map.parse("wt:@", &TestChanges(changed_paths_fn))?;
    snapbox::assert_data_eq!(
        by_short_id.to_debug(),
        snapbox::str![[r#"
[
    WorktreeUncommitted {
        id: "wt:@",
        name: "wt-a",
    },
]

"#]]
    );

    // The full name reaches the same area, so a printed `<name>:@` hint resolves.
    snapbox::assert_data_eq!(
        id_map
            .parse("wt-a:@", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    WorktreeUncommitted {
        id: "wt:@",
        name: "wt-a",
    },
]

"#]]
    );

    // The rendered ID round-trips, which is what makes it copy-pasteable from `but status`.
    snapbox::assert_data_eq!(
        id_map
            .parse(
                &by_short_id[0].to_short_string(),
                &TestChanges(changed_paths_fn),
            )?
            .to_debug(),
        snapbox::str![[r#"
[
    WorktreeUncommitted {
        id: "wt:@",
        name: "wt-a",
    },
]

"#]]
    );

    // The reference and its area are different entities, not two spellings of one.
    let reference = id_map.parse("wt", &TestChanges(changed_paths_fn))?;
    snapbox::assert_data_eq!(
        reference.to_debug(),
        snapbox::str![[r#"
[
    Worktree {
        id: "wt",
        name: "wt-a",
    },
]

"#]]
    );

    // `@` alone stays the main worktree's area and never reaches into a linked one.
    let main = id_map.parse(UNCOMMITTED, &TestChanges(changed_paths_fn))?;
    snapbox::assert_data_eq!(
        main.to_debug(),
        snapbox::str![[r#"
[
    Uncommitted {
        id: "@",
    },
]

"#]]
    );

    // Each area names its own checkout; the reference holds no changes.
    let areas = [&by_short_id[0], &reference[0], &main[0]]
        .map(|id| (id.to_short_string(), id.uncommitted_area()));
    snapbox::assert_data_eq!(
        areas.to_debug(),
        snapbox::str![[r#"
[
    (
        "wt:@",
        Some(
            Worktree(
                "wt-a",
            ),
        ),
    ),
    (
        "wt",
        None,
    ),
    (
        "@",
        Some(
            Head,
        ),
    ),
]

"#]]
    );

    Ok(())
}

/// Branches and worktrees draw from the same name-derived short-ID namespace.
#[test]
fn branch_and_worktree_short_ids_do_not_collide() -> anyhow::Result<()> {
    let id_map = IdMap::new(
        vec![stack([segment("work-branch", [], None, [])])],
        vec![source_changes(
            ChangeSourceId::Worktree("worktree-01".into()),
            Vec::new(),
        )],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 0
branches: [ wo ]
worktrees: [ or worktree-01 ]


"#]]
    );

    Ok(())
}

/// Generated IDs for named and anonymous segments remain reserved when worktree IDs are allocated
/// later, while retaining their distinct CLI ID kinds.
#[test]
fn generated_segment_ids_do_not_collide_with_worktree_names() -> anyhow::Result<()> {
    let mut anonymous_segment = segment("unused", [], None, []);
    anonymous_segment.ref_info = None;
    let id_map = IdMap::new(
        vec![stack([segment("ab", [], None, []), anonymous_segment])],
        vec![
            source_changes(ChangeSourceId::Worktree("g0-worktree".into()), Vec::new()),
            source_changes(ChangeSourceId::Worktree("h0-worktree".into()), Vec::new()),
        ],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    // The named fallback and anonymous segment use generated IDs, which stay
    // reserved: the worktrees named after them get other IDs.
    snapbox::assert_data_eq!(
        id_map.debug_state().to_debug(),
        snapbox::str![[r#"
workspace_and_remote_commits_count: 0
branches: [ g0, h0 ]
worktrees: [ or h0-worktree, wo g0-worktree ]


"#]]
    );

    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    // The named segment resolves as a branch, the anonymous one as its own CLI ID kind.
    let segment_ids = id_map
        .branch_ids()
        .iter()
        .map(|id| id_map.parse(id, &TestChanges(changed_paths_fn)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    snapbox::assert_data_eq!(
        segment_ids.to_debug(),
        snapbox::str![[r#"
[
    [
        Branch(
            BranchId {
                name: "ab",
                id: "g0",
                stack_id: None,
            },
        ),
    ],
    [
        AnonymousSegment(
            AnonymousSegmentId {
                id: "h0",
                stack_id: None,
                anchor_commit_id: None,
            },
        ),
    ],
]

"#]]
    );

    // Each displayed worktree ID resolves to its worktree.
    let worktree_ids = id_map
        .worktrees
        .values()
        .map(|worktree| id_map.parse(&worktree.short_id, &TestChanges(changed_paths_fn)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    snapbox::assert_data_eq!(
        worktree_ids.to_debug(),
        snapbox::str![[r#"
[
    [
        Worktree {
            id: "wo",
            name: "g0-worktree",
        },
    ],
    [
        Worktree {
            id: "or",
            name: "h0-worktree",
        },
    ],
]

"#]]
    );

    Ok(())
}

/// `@` names the main worktree, so `@:<path>` must not reach into a linked
/// worktree that happens to have the same path dirty.
#[test]
fn at_scopes_filenames_to_the_main_worktree() -> anyhow::Result<()> {
    let id_map = IdMap::new(
        Vec::new(),
        vec![
            source_changes(ChangeSourceId::Head, vec![hunk("file")]),
            source_changes(ChangeSourceId::Worktree("wt-a".into()), vec![hunk("file")]),
        ],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Bare `@` names the main worktree's whole uncommitted area.
    snapbox::assert_data_eq!(
        id_map
            .parse("@", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Uncommitted {
        id: "@",
    },
]

"#]]
    );

    // `@` only ever matches the main worktree, so scoping keeps its copy.
    snapbox::assert_data_eq!(
        id_map
            .parse("@:file", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "qs",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "qs:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "file",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    // A bare filename is deliberately unscoped, so it reports every checkout it
    // is dirty in and the caller turns that into an ambiguity error.
    snapbox::assert_data_eq!(
        id_map
            .parse("file", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "qs",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "qs:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "file",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "rp",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "rp:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "file",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Worktree(
                "wt-a",
            ),
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
    let hunks = vec![hunk("file")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        id_map
            .parse("qs", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Branch(
        BranchId {
            name: "qsy",
            id: "qs",
            stack_id: None,
        },
    ),
]

"#]]
    );

    // Still only the branch when querying by full name
    snapbox::assert_data_eq!(
        id_map
            .parse("qsy", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Branch(
        BranchId {
            name: "qsy",
            id: "qs",
            stack_id: None,
        },
    ),
]

"#]]
    );

    // More characters must be specified to get the file
    snapbox::assert_data_eq!(
        id_map
            .parse("qsyn", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "qsy",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "qsy:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "file",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
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
    let hunks = vec![hunk("foo23")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        id_map
            .parse("kpr", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kp",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kp:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo23",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
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
    let hunks = vec![hunk("klmxyz")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        id_map
            .parse("kl", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kl",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kl:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "klmxyz",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
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
    let hunks = vec![hunk("foo")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        id_map
            .parse("foo", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Branch(
        BranchId {
            name: "foo",
            id: "fo",
            stack_id: None,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "zo",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "zo:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
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
    let hunks = vec![hunk("uncommitted"), hunk("assigned")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Short branch works
    snapbox::assert_data_eq!(
        id_map
            .parse("gg@{stack}:assigned", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "nv",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "nv:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "assigned",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    // Long branch works
    snapbox::assert_data_eq!(
        id_map
            .parse("gggg@{stack}:assigned", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "nv",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "nv:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "assigned",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    // Uncommitted works
    snapbox::assert_data_eq!(
        id_map
            .parse("@:uncommitted", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "pv",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "pv:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "uncommitted",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
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
    let hunks = vec![hunk("prefixx"), hunk("prefix/a"), hunk("prefix/b")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Returns one ID with all hunk assignments
    snapbox::assert_data_eq!(
        id_map
            .parse("prefix/", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    PathPrefix {
        id: "prefix/",
        hunks: NonEmpty {
            head: IdAndHunk {
                id: "yz:e",
                hunk: SingleHunk {
                    hunk_header: None,
                    path: "prefix/a",
                    diff: None,
                },
            },
            tail: [
                IdAndHunk {
                    id: "uo:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "prefix/b",
                        diff: None,
                    },
                },
            ],
        },
        source: Head,
    },
]

"#]]
    );

    // If nothing matches, returns no ID
    snapbox::assert_data_eq!(
        id_map
            .parse("doesnotmatch/", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    Ok(())
}

/// This test represents a bad state: This shouldn't really happened, but if it does we don't want
/// to just lose changes.
///
/// See [super::FileInfo::changes] for details on this situation.
#[test]
fn duplicate_tree_changes_for_committed_files_are_coalesced_for_short_id_assignment()
-> anyhow::Result<()> {
    let first = tree_change_addition("file.txt");
    let second = but_core::TreeChange {
        path: BString::from("file.txt"),
        status: but_core::TreeStatus::Deletion {
            previous_state: first.status.state().unwrap(),
        },
    };

    let changes = super::short_ids_from_tree_changes(vec![first, second])?;

    // Both tree changes for the path go into a single bucket, each retained in order.
    snapbox::assert_data_eq!(
        changes.to_debug(),
        snapbox::str![[r#"
[
    (
        NonEmpty {
            head: TreeChange {
                path: "file.txt",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            tail: [
                TreeChange {
                    path: "file.txt",
                    status: Deletion {
                        previous_state: ChangeState {
                            id: Sha1(0000000000000000000000000000000000000000),
                            kind: Blob,
                        },
                    },
                },
            ],
        },
        "uvwtvwskpzypsmwlvymvtsvympuvovuy",
        "u",
    ),
]

"#]]
    );
    Ok(())
}

#[test]
fn committed_files_are_deduplicated_by_commit_oid_path() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("branch", [id(2)], Some(id(1)), [])])];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

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

    // Both files resolve exactly once, by ID and by filename.
    let lookups = ["02:uv", "02:xw", "02:file.txt", "02:other.txt"]
        .into_iter()
        .map(|selector| {
            id_map
                .parse(selector, &TestChanges(changed_paths_fn))
                .map(|matches| (selector, matches))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    snapbox::assert_data_eq!(
        lookups.to_debug(),
        snapbox::str![[r#"
[
    (
        "02:uv",
        [
            CommittedFile {
                committed_file: CommittedFileId {
                    commit_id: Sha1(0202020202020202020202020202020202020202),
                    path: "file.txt",
                    change_id: None,
                },
                id: "0:u",
            },
        ],
    ),
    (
        "02:xw",
        [
            CommittedFile {
                committed_file: CommittedFileId {
                    commit_id: Sha1(0202020202020202020202020202020202020202),
                    path: "other.txt",
                    change_id: None,
                },
                id: "0:x",
            },
        ],
    ),
    (
        "02:file.txt",
        [
            CommittedFile {
                committed_file: CommittedFileId {
                    commit_id: Sha1(0202020202020202020202020202020202020202),
                    path: "file.txt",
                    change_id: None,
                },
                id: "0:u",
            },
        ],
    ),
    (
        "02:other.txt",
        [
            CommittedFile {
                committed_file: CommittedFileId {
                    commit_id: Sha1(0202020202020202020202020202020202020202),
                    path: "other.txt",
                    change_id: None,
                },
                id: "0:x",
            },
        ],
    ),
]

"#]]
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
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        commit_id_to_change_id,
        Default::default(),
        3,
    )
    .unwrap();

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
            .parse("0:u", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    CommittedFile {
        committed_file: CommittedFileId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            path: "file.txt",
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "s:u",
    },
]

"#]]
    );
    assert_data_eq!(
        id_map
            .parse("s:u", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    CommittedFile {
        committed_file: CommittedFileId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            path: "file.txt",
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "s:u",
    },
]

"#]]
    );
}

#[test]
fn short_uncommitted_files_are_properly_reverse_hexed() -> anyhow::Result<()> {
    let stacks = vec![stack([segment("foo", [id(1)], None, [])])];
    let hunks = vec![hunk("k"), hunk("kl"), hunk("klm")];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
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
        id_map
            .parse("k", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ky",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ky:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "k",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("kl", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "klx",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "klx:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "kl",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("klm", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "klml",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "klml:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "klm",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,2", "+1,2")),
            ..hunk("uncommitted1.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-3,2", "+3,2")),
            ..hunk("uncommitted1.txt")
        },
        hunk("uncommitted2.txt"),
    ];
    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("uncommitted1.txt:#0", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:e#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:e#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "uncommitted1.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
    );
    // Short IDs for the filename part also work; should return exactly the same as above
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:#0", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:e#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:e#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "uncommitted1.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
    );
    // Files can also be accessed through @.
    snapbox::assert_data_eq!(
        id_map
            .parse("@:uncommitted1.txt:#0", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:e#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:e#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,2", "+1,2"),
                        ),
                        path: "uncommitted1.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        // Same context lines as first hunk, but different diff
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+there\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        // Same diff as first hunk, but different context lines
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-60,6", "+62,7")),
            diff: Some(BString::new(
                "@@ -60,6 +62,7 @@\n 46\n 47\n 48\n+hello\n 49\n 50\n 51\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        hunk("hunk_without_diff.txt"),
    ];

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:3",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:3",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:f", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:f",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:f",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-23,6", "+24,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+there\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:1", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:1",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:1",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-60,6", "+62,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -60,6 +62,7 @@\n 46\n 47\n 48\n+hello\n 49\n 50\n 51\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    // Hunk without diff gets an identifier from the empty-content prefix.
    snapbox::assert_data_eq!(
        id_map
            .parse("hunk_without_diff.txt:e", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "wp:e",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "wp:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "hunk_without_diff.txt",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
    ];

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:78", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:78",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:78",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:79", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:79",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:79",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-23,6", "+24,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
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
    let hunks = vec![but_core::SingleHunk {
        hunk_header: Some(hunk_header("-1,6", "+1,7")),
        diff: Some(BString::new(
            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                .as_bytes()
                .to_vec(),
        )),
        ..hunk("uncommitted1.txt")
    }];

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:78", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:7",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:7",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        // Same context lines as first hunk, but different diff
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
    ];

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3eeb#0-2", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:3#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:3#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-33,6", "+35,7")),
            diff: Some(BString::new(
                "@@ -33,6 +35,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
    ];

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Underspecifying with just first character finds all hunks
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:7", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:78#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:78#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:79",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:79",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-23,6", "+24,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hellooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:78#1-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:78#1-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-33,6", "+35,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -33,6 +35,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    // Underspecifying with collision index only finds hunk with precisely matching collision index.
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:7#0-2", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:78#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:78#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hellooooo\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    // An entirely empty prefix matches nothing
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    // Omitting only specifying collision index matches nothing - we don't allow omitting the prefix
    // unless you are explicitly indexing into the file's hunks
    snapbox::assert_data_eq!(
        id_map
            .parse("ro:#0-2", &TestChanges(changed_paths_fn))?
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
    let hunks = vec![
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-1,6", "+1,7")),
            diff: Some(BString::new(
                "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
        but_core::SingleHunk {
            hunk_header: Some(hunk_header("-23,6", "+24,7")),
            diff: Some(BString::new(
                "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n"
                    .as_bytes()
                    .to_vec(),
            )),
            ..hunk("uncommitted1.txt")
        },
    ];

    let id_map = IdMap::new(
        stacks,
        vec![source_changes(ChangeSourceId::Head, hunks)],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3#0-2", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:3#0-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:3#0-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-1,6", "+1,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -1,6 +1,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("ro:3#1-2", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "ro:3#1-2",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "ro:3#1-2",
                    hunk: SingleHunk {
                        hunk_header: Some(
                            HunkHeader("-23,6", "+24,7"),
                        ),
                        path: "uncommitted1.txt",
                        diff: Some(
                            "@@ -23,6 +24,7 @@\n 1\n 2\n 3\n+hello\n 4\n 5\n 6\n",
                        ),
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
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
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let matches = id_map.parse("02", &TestChanges(changed_paths_fn))?;
    assert_eq!(matches.len(), 1);
    assert!(
        matches.iter().any(
            |m| matches!(m, CliId::Commit { commit: CommitId { commit_id: id, .. }, id: _ } if *id == commit_id)
        ),
        "same commit reachable through local and remote views should not be ambiguous"
    );

    Ok(())
}

#[test]
fn dedupe_does_not_hide_ambiguity_between_distinct_commits() -> anyhow::Result<()> {
    let id1 = hex_to_id("21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let id2 = hex_to_id("21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let stacks = vec![stack([segment("branch", [id1, id2], None, [])])];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // Distinct commits sharing a prefix remain ambiguous.
    snapbox::assert_data_eq!(
        id_map
            .parse("21", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
            change_id: None,
        },
        id: "21a",
    },
    Commit {
        commit: CommitId {
            commit_id: Sha1(21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
            change_id: None,
        },
        id: "21b",
    },
]

"#]]
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
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    // The same branch name across different stacks remains ambiguous.
    snapbox::assert_data_eq!(
        id_map
            .parse("foo", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Branch(
        BranchId {
            name: "foo",
            id: "fo",
            stack_id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
        },
    ),
    Branch(
        BranchId {
            name: "foo",
            id: "oo",
            stack_id: Some(
                00000000-0000-0000-0000-000000000002,
            ),
        },
    ),
]

"#]]
    );

    Ok(())
}

#[test]
fn dedupe_treats_unmanaged_branches_with_same_name_as_the_same_branch() -> anyhow::Result<()> {
    let stacks = vec![
        stack([segment("foo", [id(1)], None, [])]),
        stack([segment("foo", [id(2)], None, [])]),
    ];
    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };

    let matches = id_map.parse("foo", &TestChanges(changed_paths_fn))?;
    assert!(
        matches!(
            matches.as_slice(),
            [CliId::Branch(branch)] if branch.name == "foo" && branch.stack_id.is_none()
        ),
        "unmanaged branches with the same name have the same identity"
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

    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        commit_id_to_change_id,
        Default::default(),
        3,
    )
    .unwrap();
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
            .parse("sws", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "01",
    },
    Commit {
        commit: CommitId {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            change_id: Some(
                "swsrzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "02",
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("swst", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "01",
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("swsr", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            change_id: Some(
                "swsrzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "02",
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
        vec![source_changes(
            ChangeSourceId::Head,
            vec![hunk("README.md")],
        )],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;
    let file_id = commitless
        .all_ids()
        .into_iter()
        .find_map(|cli_id| match cli_id {
            CliId::UncommittedHunkOrFile(uncommitted) => Some(uncommitted.id.clone()),
            _ => None,
        })
        .expect("one uncommitted file");
    snapbox::assert_data_eq!(file_id.as_str(), snapbox::str!["rl"]);

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
        vec![source_changes(
            ChangeSourceId::Head,
            vec![hunk("README.md")],
        )],
        commit_id_to_change_id,
        Default::default(),
        3,
    )?;

    // In the full namespace the commit shadows the previously issued file ID.
    snapbox::assert_data_eq!(
        id_map
            .parse(&file_id, &TestChanges(changed_paths_fn()))?
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            change_id: Some(
                "rlzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "0",
    },
]

"#]]
    );

    // Scoped to uncommitted files, the same selector still finds the file it was issued for.
    snapbox::assert_data_eq!(
        id_map
            .parse_uncommitted(&file_id, &TestChanges(changed_paths_fn()))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "rln",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "rln:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "README.md",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    // Hunk selectors under the file keep working too.
    let hunk_selector = format!("{file_id}:e");
    snapbox::assert_data_eq!(
        id_map
            .parse_uncommitted(&hunk_selector, &TestChanges(changed_paths_fn()))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "rln:e",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "rln:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "README.md",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ),
]

"#]]
    );

    let tmp = tempfile::TempDir::new()?;
    let repo = gix::init(tmp.path())?;
    // The issued hunk selector still resolves to exactly that hunk.
    snapbox::assert_data_eq!(
        CliIdArg(hunk_selector.clone())
            .try_resolve_uncommitted(&repo, &id_map)
            .expect("selector resolution succeeds")
            .to_debug(),
        snapbox::str![[r#"
Some(
    [
        UncommittedHunkOrFile {
            id: "rln:e",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "rln:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "README.md",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: false,
            source: Head,
        },
    ],
)

"#]]
    );

    // The shared uncommitted-purpose resolver keeps the issued hunk.
    snapbox::assert_data_eq!(
        CliIdArg(hunk_selector)
            .resolve_in_workspace(
                &repo,
                &id_map,
                crate::args::atoms::Purpose::Uncommitted,
                None,
            )
            .expect("a command requiring an uncommitted selector resolves the issued hunk")
            .to_debug(),
        snapbox::str![[r#"
UncommittedHunkOrFile(
    UncommittedHunkOrFile {
        id: "rln:e",
        hunks: NonEmpty {
            head: IdAndHunk {
                id: "rln:e",
                hunk: SingleHunk {
                    hunk_header: None,
                    path: "README.md",
                    diff: None,
                },
            },
            tail: [],
        },
        is_entire_file: false,
        source: Head,
    },
)

"#]]
    );

    Ok(())
}

#[test]
fn a_file_literally_named_at_competes_with_the_uncommitted_area() -> anyhow::Result<()> {
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    let id_map = IdMap::new(
        vec![stack([segment("not-important", [], None, [])])],
        vec![source_changes(ChangeSourceId::Head, vec![hunk("@")])],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    // `@` names the whole area, so a dirty file of the same name must surface
    // as a competing match for the resolver to report - not silently win, the
    // same way a file named after a worktree competes with the worktree.
    let scoped = id_map.parse_uncommitted("@", &TestChanges(changed_paths_fn))?;
    match scoped.as_slice() {
        [
            CliId::UncommittedHunkOrFile(uncommitted),
            CliId::Uncommitted { .. },
        ] => {
            assert_eq!(uncommitted.hunks.first().hunk.path, "@");
        }
        other => panic!("expected the file named @ and the area sentinel, got {other:?}"),
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
        vec![source_changes(ChangeSourceId::Head, vec![hunk("foo242")])],
        gix::hashtable::HashMap::default(),
        Default::default(),
        3,
    )?;

    // Precondition: the full namespace resolves `kp` to the branch.
    snapbox::assert_data_eq!(
        id_map
            .parse("kp", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    Branch(
        BranchId {
            name: "kp",
            id: "kp",
            stack_id: None,
        },
    ),
]

"#]]
    );

    // The scoped parser must NOT silently resolve the displayed branch ID to
    // the file by hex-prefix accident: an empty result lets callers produce
    // the targeted "is a branch" error via their full-namespace fallback.
    snapbox::assert_data_eq!(
        id_map
            .parse_uncommitted("kp", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    // A longer prefix that no branch owns still resolves the file.
    snapbox::assert_data_eq!(
        id_map
            .parse_uncommitted("kpo", &TestChanges(changed_paths_fn))?
            .to_debug(),
        snapbox::str![[r#"
[
    UncommittedHunkOrFile(
        UncommittedHunkOrFile {
            id: "kpo",
            hunks: NonEmpty {
                head: IdAndHunk {
                    id: "kpo:e",
                    hunk: SingleHunk {
                        hunk_header: None,
                        path: "foo242",
                        diff: None,
                    },
                },
                tail: [],
            },
            is_entire_file: true,
            source: Head,
        },
    ),
]

"#]]
    );

    let tmp = tempfile::TempDir::new()?;
    let repo = gix::init(tmp.path())?;
    // When the uncommitted-only lookup deliberately yields nothing, the full
    // lookup preserves the branch kind for the caller's targeted error.
    snapbox::assert_data_eq!(
        CliIdArg("kp".to_owned())
            .resolve_in_workspace(
                &repo,
                &id_map,
                crate::args::atoms::Purpose::Uncommitted,
                None,
            )
            .expect("the full lookup preserves a non-uncommitted match")
            .to_debug(),
        snapbox::str![[r#"
Branch(
    BranchArg(
        "kp",
    ),
)

"#]]
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

    let id_map = IdMap::new(
        stacks,
        Vec::new(),
        commit_id_to_change_id,
        Default::default(),
        3,
    )
    .unwrap();
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
            .parse("sws", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "01",
    },
    Commit {
        commit: CommitId {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "02",
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("s#0", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0101010101010101010101010101010101010101),
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "01",
    },
]

"#]],
    );

    snapbox::assert_data_eq!(
        id_map
            .parse("s#1", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            change_id: Some(
                "swstzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
        },
        id: "02",
    },
]

"#]],
    )
}

/// Commits owned by a linked worktree share the commit and change-ID namespace with the
/// workspace stacks: they resolve by change ID and hex prefix like workspace commits, and
/// colliding prefixes on either side lengthen the IDs of both.
#[test]
fn worktree_commits_share_the_commit_namespace() -> anyhow::Result<()> {
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<but_core::TreeChange>> {
        bail!("unexpected IDs {commit_id} {parent_id:?}");
    };
    // The hashes share the prefix "21", so neither commit may print it bare.
    let ws_commit = hex_to_id("21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let wt_commit = hex_to_id("21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let stacks = vec![stack([segment("branch", [ws_commit], None, [])])];
    let commit_id_to_change_id = [
        (ws_commit, ChangeId::from(BString::from(&b"swst"[..]))),
        (wt_commit, ChangeId::from(BString::from(&b"swsr"[..]))),
    ]
    .into_iter()
    .collect();
    // The worktree registers as a change source even when clean, which is what keys its
    // commits into the map.
    let sources = vec![source_changes(
        ChangeSourceId::Worktree("wt-a".into()),
        Vec::new(),
    )];
    let worktree_commits = [(
        BString::from("wt-a"),
        vec![but_graph::workspace::StackCommit {
            id: wt_commit,
            parent_ids: Vec::new(),
            refs: Vec::new(),
            flags: Default::default(),
        }],
    )]
    .into_iter()
    .collect();
    let id_map = IdMap::new(stacks, sources, commit_id_to_change_id, worktree_commits, 3)?;

    // The worktree commit resolves by its change ID, disambiguated against the
    // workspace commit's "swst".
    snapbox::assert_data_eq!(
        id_map
            .parse("swsr", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(21bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
            change_id: Some(
                "swsr",
            ),
        },
        id: "21b",
    },
]

"#]],
    );

    // The shared hex prefix is ambiguous across the two sets.
    assert_eq!(
        id_map
            .parse("21", &TestChanges(changed_paths_fn))
            .unwrap()
            .len(),
        2,
        "a hex prefix shared by a workspace and a worktree commit matches both"
    );

    // One more nybble singles out the workspace commit, whose short ID grew to match.
    snapbox::assert_data_eq!(
        id_map
            .parse("21a", &TestChanges(changed_paths_fn))
            .unwrap()
            .to_debug(),
        snapbox::str![[r#"
[
    Commit {
        commit: CommitId {
            commit_id: Sha1(21aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
            change_id: Some(
                "swst",
            ),
        },
        id: "21a",
    },
]

"#]],
    );
    Ok(())
}

mod util {
    use std::{cmp::Ordering, fmt::Formatter};

    use super::TestChanges;

    use anyhow::bail;
    use bstr::BString;
    use but_graph::workspace::{Stack, StackCommit, StackSegment};
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

    /// A source whose `changes` are left empty: [`IdMap`] reads only `hunks`, as
    /// tree statuses are a status-rendering concern.
    pub fn source_changes(
        source: crate::ChangeSourceId,
        hunks: Vec<but_core::SingleHunk>,
    ) -> crate::utils::change_source::SourceChanges {
        crate::utils::change_source::SourceChanges {
            source,
            changes: Vec::new(),
            hunks,
        }
    }

    pub fn hunk(path: &str) -> but_core::SingleHunk {
        but_core::SingleHunk {
            hunk_header: None,
            path: BString::from(path),
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
                worktrees: _,
                diff_context_lines: _,
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
                    self.parse(&id, &TestChanges(changed_paths_fn))
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
                worktrees,
                diff_context_lines: _,
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
                "worktrees",
                worktrees
                    .values()
                    .map(|worktree| format!("{} {}", worktree.short_id, worktree.name))
                    .sorted(),
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
use util::{hunk, id, segment, source_changes, stack, tree_change_addition};
