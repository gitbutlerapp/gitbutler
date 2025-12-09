use crate::legacy::id::{CliId, IdMap};
use anyhow::bail;
use bstr::BString;
use but_testsupport::Sandbox;

// TODO: make the IdMap API more testable, and making better tests should naturally lead to a better API.
//       This is just an example to avoid more integration tests.
#[test]
fn commit_ids_never_collide_due_to_hex_alphabet() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;
    let mut ctx = env.context()?;

    let mut id_map = IdMap::new_from_context(&ctx)?;
    id_map.add_file_info_from_context(&mut ctx)?;
    assert_eq!(id_map.workspace_and_remote_commit_ids().count(), 2);

    for commit_id in id_map.workspace_and_remote_commit_ids() {
        // TODO: fix this - should be read-only, but needs a `but-db` refactor to support read-only DB access.
        let actual = id_map.parse_str(&commit_id.to_hex_with_len(2).to_string())?;
        assert_eq!(actual.len(), 1, "The commit can be resolved");
        assert!(
            matches!(&actual[0], CliId::Commit { oid } if oid == commit_id,),
            "The id refers to the right commit"
        );
    }
    Ok(())
}

#[test]
fn unassigned_area_id_is_unambiguous() -> anyhow::Result<()> {
    let stacks = &[stack([segment("branch001", [id(1)], None, [])])];
    let id_map = IdMap::new(stacks)?;

    assert_eq!(
        id_map.unassigned().to_string(),
        "000",
        "the ID of the unassigned area should have enough 0s to be unambiguous"
    );
    Ok(())
}

#[test]
fn non_commit_ids_do_not_collide() -> anyhow::Result<()> {
    let stacks = &[stack([segment("h0", [id(2)], Some(id(1)), [])])];
    let mut id_map = IdMap::new(stacks)?;
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

    assert!(
        matches!(
        id_map.parse_str("g0")?.as_slice(),
        [CliId::UncommittedFile{path,..}] if path == b"uncommitted1.txt"),
        "Uncommitted files come first"
    );
    assert!(
        matches!(
        id_map.parse_str("h0")?.as_slice(),
        [CliId::Branch{name,..}] if name == "h0"),
        "uncommitted files do not collide with branches"
    );
    assert!(
        matches!(
        id_map.parse_str("i0")?.as_slice(),
        [CliId::UncommittedFile{path,..}] if path == b"uncommitted2.txt"),
        "uncommitted files also don't collide with themselves"
    );
    assert!(
        matches!(
        id_map.parse_str("j0")?.as_slice(),
        [CliId::CommittedFile{commit_oid, path,..}] if *commit_oid == id(2) && path == b"committed1.txt"),
        "then come committed files, as per incremented prefix"
    );
    assert!(
        matches!(
        id_map.parse_str("k0")?.as_slice(),
        [CliId::CommittedFile{commit_oid, path,..}] if *commit_oid == id(2) && path == b"committed2.txt"),
        "committed files also don't collide with themselves"
    );
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
