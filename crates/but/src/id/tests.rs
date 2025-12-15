use crate::{CliId, IdMap, id::UintId};
use anyhow::bail;
use bstr::BString;

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
    Ok(())
}

#[test]
fn commit_id_works_with_two_characters() -> anyhow::Result<()> {
    let id1 = id(1);
    let stacks = &[stack([segment("foo", [id1], None, [])])];
    let id_map = IdMap::new_for_branches_and_commits(stacks)?;

    let expected = [CliId::Commit(id1)];
    assert_eq!(
        id_map.resolve_entity_to_ids("01")?,
        expected,
        "two characters are sufficient to parse a commit ID"
    );
    assert_eq!(
        id_map.resolve_entity_to_ids("010")?,
        expected,
        "three characters work too"
    );
    Ok(())
}

#[test]
fn multiple_zeroes_as_unassigned_area() -> anyhow::Result<()> {
    let stacks = &[stack([segment("foo", [id(1)], None, [])])];
    let id_map = IdMap::new_for_branches_and_commits(stacks)?;

    assert_eq!(
        id_map.resolve_entity_to_ids("000")?,
        [CliId::Unassigned { id: "00".into() }],
        "any number of 0s interpreted as the unassigned area"
    );
    Ok(())
}

#[test]
fn unassigned_area_id_is_unambiguous() -> anyhow::Result<()> {
    let stacks = &[stack([segment("branch001", [id(1)], None, [])])];
    let id_map = IdMap::new_for_branches_and_commits(stacks)?;

    assert_eq!(
        id_map.unassigned().to_short_string(),
        "000",
        "the ID of the unassigned area should have enough 0s to be unambiguous"
    );
    Ok(())
}

#[test]
fn branch_avoid_nonalphanumeric() -> anyhow::Result<()> {
    let stacks = &[stack([segment("x-yz", [id(1)], None, [])])];
    let id_map = IdMap::new_for_branches_and_commits(stacks)?;

    let expected = [CliId::Branch {
        name: "x-yz".into(),
        id: "yz".into(),
    }];
    assert_eq!(
        id_map.resolve_entity_to_ids("x-yz")?,
        expected,
        "avoids non-alphanumeric, taking first alphanumeric pair"
    );
    Ok(())
}

#[test]
fn branch_avoid_hexdigit() -> anyhow::Result<()> {
    let stacks = &[stack([segment("0ax", [id(1)], None, [])])];
    let id_map = IdMap::new_for_branches_and_commits(stacks)?;

    let expected = [CliId::Branch {
        name: "0ax".into(),
        id: "ax".into(),
    }];
    assert_eq!(
        id_map.resolve_entity_to_ids("0ax")?,
        expected,
        "avoids hexdigit pair which can be confused with a commit ID"
    );
    Ok(())
}

#[test]
fn branch_cannot_generate_id() -> anyhow::Result<()> {
    let stacks = &[
        stack([segment("substring", [id(1)], None, [])]),
        stack([segment("supersubstring", [id(2)], None, [])]),
    ];
    let id_map = IdMap::new_for_branches_and_commits(stacks)?;

    let expected = [CliId::Branch {
        name: "substring".into(),
        id: "g0".into(),
    }];
    assert_eq!(
        id_map.resolve_entity_to_ids("substring")?,
        expected,
        "no unique ID, so take from pool of IDs",
    );
    let expected = [CliId::Branch {
        name: "supersubstring".into(),
        id: "up".into(),
    }];
    assert_eq!(
        id_map.resolve_entity_to_ids("supersubstring")?,
        expected,
        "'su' would collide with substring, so 'up' is chosen"
    );
    Ok(())
}

#[test]
fn non_commit_ids_do_not_collide() -> anyhow::Result<()> {
    let stacks = &[stack([segment("h0", [id(2)], Some(id(1)), [])])];
    let mut id_map = IdMap::new_for_branches_and_commits(stacks)?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<BString>> {
        Ok(if commit_id == id(2) && parent_id == Some(id(1)) {
            vec![
                BString::from(b"committed1.txt"),
                BString::from(b"committed2.txt"),
            ]
        } else {
            bail!("unexpected IDs {} {:?}", commit_id, parent_id);
        })
    };
    let hunk_assignments = vec![
        hunk_assignment("uncommitted1.txt", None),
        hunk_assignment("uncommitted2.txt", None),
    ];
    id_map.add_file_info(changed_paths_fn, hunk_assignments)?;

    // Uncommitted files come first
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("g0")?, @r#"
    [
        UncommittedFile {
            assignment: None,
            path: "uncommitted1.txt",
            id: "g0",
        },
    ]
    "#);

    // uncommitted files do not collide with branches
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("h0")?, @r#"
    [
        Branch {
            name: "h0",
            id: "h0",
        },
    ]
    "#);

    // uncommitted files also don't collide with themselves
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("i0")?, @r#"
    [
        UncommittedFile {
            assignment: None,
            path: "uncommitted2.txt",
            id: "i0",
        },
    ]
    "#);

    // then come committed files, as per incremented prefix
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("j0")?, @r#"
    [
        CommittedFile {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            path: "committed1.txt",
            id: "j0",
        },
    ]
    "#);

    // committed files also don't collide with themselves
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("k0")?, @r#"
    [
        CommittedFile {
            commit_id: Sha1(0202020202020202020202020202020202020202),
            path: "committed2.txt",
            id: "k0",
        },
    ]
    "#);
    Ok(())
}

#[test]
fn ids_are_case_sensitive() -> anyhow::Result<()> {
    let stacks = &[stack([segment("h0", [id(10)], Some(id(9)), [])])];
    let mut id_map = IdMap::new_for_branches_and_commits(stacks)?;
    let changed_paths_fn = |commit_id: gix::ObjectId,
                            parent_id: Option<gix::ObjectId>|
     -> anyhow::Result<Vec<BString>> {
        Ok(if commit_id == id(10) && parent_id == Some(id(9)) {
            vec![BString::from(b"committed.txt")]
        } else {
            bail!("unexpected IDs {} {:?}", commit_id, parent_id);
        })
    };
    let hunk_assignments = vec![hunk_assignment("uncommitted.txt", None)];
    id_map.add_file_info(changed_paths_fn, hunk_assignments)?;

    // Commits
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("0a")?, @r"
    [
        Commit(
            Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
        ),
    ]
    ");
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("0A")?, @"[]");

    // Branches
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("h0")?, @r#"
    [
        Branch {
            name: "h0",
            id: "h0",
        },
    ]
    "#);
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("H0")?, @"[]");

    // Uncommitted files
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("g0")?, @r#"
    [
        UncommittedFile {
            assignment: None,
            path: "uncommitted.txt",
            id: "g0",
        },
    ]
    "#);
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("G0")?, @"[]");

    // Committed files
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("i0")?, @r#"
    [
        CommittedFile {
            commit_id: Sha1(0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a),
            path: "committed.txt",
            id: "i0",
        },
    ]
    "#);
    insta::assert_debug_snapshot!(id_map.resolve_entity_to_ids("I0")?, @"[]");

    Ok(())
}

mod util {
    use bstr::BString;
    use but_core::ref_metadata::StackId;
    use but_hunk_assignment::HunkAssignment;
    use but_workspace::branch::Stack;
    use but_workspace::ref_info::{Commit, LocalCommit, Segment};

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
            ref_name: gix::refs::FullName::try_from(format!(
                "refs/heads/{}",
                shortened_branch_name
            ))
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
}
use util::{hunk_assignment, id, segment, stack};
