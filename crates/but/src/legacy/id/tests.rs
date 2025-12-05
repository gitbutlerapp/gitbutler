use crate::legacy::id::{CliId, IdMap};
use but_hunk_assignment::HunkAssignmentRequest;
use but_testsupport::Sandbox;

// TODO: make the IdMap API more testable, and making better tests should naturally lead to a better API.
//       This is just an example to avoid more integration tests.
#[test]
fn commit_ids_never_collide_due_to_hex_alphabet() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;
    let mut ctx = env.context()?;

    let id_map = IdMap::new(&mut ctx)?;
    assert_eq!(id_map.commit_ids.len(), 2);
    for commit_id in &id_map.commit_ids {
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

// TODO: be more specific, this is mostly a sample for how to setup assignments as part of the test.
#[test]
fn assignments_work() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;
    let stack_ids = env.setup_metadata(&["A", "B"])?;

    let a_path = "for-A";
    env.file(a_path, "A");
    let b_path = "for-B";
    env.file(b_path, "B");

    let mut ctx = env.context()?;
    let a_stack_id = stack_ids[0];
    let b_stack_id = stack_ids[1];
    but_hunk_assignment::assign(
        &mut ctx,
        vec![
            HunkAssignmentRequest {
                hunk_header: None,
                path_bytes: a_path.into(),
                stack_id: a_stack_id.into(),
            },
            HunkAssignmentRequest {
                hunk_header: None,
                path_bytes: b_path.into(),
                stack_id: b_stack_id.into(),
            },
        ],
        None,
    )?;

    let id_map = IdMap::new(&mut ctx)?;
    assert_eq!(
        id_map
            .uncommitted_file(Some(a_stack_id), a_path.into())
            .to_string(),
        "g0"
    );
    assert_eq!(
        id_map
            .uncommitted_file(Some(b_stack_id), b_path.into())
            .to_string(),
        "h0"
    );
    Ok(())
}
