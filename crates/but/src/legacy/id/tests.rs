use crate::legacy::id::{CliId, IdMap};
use crate::utils::tests::{legacy_minit, writable_scenario};

// TODO: make the IdMap API more testable, and making better tests should naturally lead to a better API.
//       This is just an example to avoid more integration tests.
#[test]
fn commit_ids_never_collide_due_to_hex_alphabet() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = writable_scenario("two-stacks");
    legacy_minit(&ctx)?;

    let id_map = IdMap::new(&mut ctx)?;
    assert_eq!(id_map.commit_ids.len(), 2);
    for commit_id in &id_map.commit_ids {
        // TODO: fix this - should be read-only, but needs a `but-db` refactor to support read-only DB access.
        let actual = id_map.parse_str(&mut ctx, &commit_id.to_hex_with_len(2).to_string())?;
        assert_eq!(actual.len(), 1, "The commit can be resolved");
        assert!(
            matches!(&actual[0], CliId::Commit { oid } if oid == commit_id,),
            "The id refers to the right commit"
        );
    }
    Ok(())
}
