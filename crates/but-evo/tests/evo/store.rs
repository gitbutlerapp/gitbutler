use std::collections::BTreeMap;

use but_evo::{Marker, MarkerStore};
use but_testsupport::{hex_to_id, in_memory_db};

fn id(digit: char) -> gix::ObjectId {
    hex_to_id(&digit.to_string().repeat(40))
}

fn marker() -> Marker {
    Marker {
        prec: id('a'),
        succs: vec![id('c'), id('b')],
        meta: BTreeMap::from([
            ("ef1".into(), "8".into()),
            ("user".into(), "Caleb Owens <caleb@gitbutler.com>".into()),
        ]),
        date: gix::date::Time::new(1_700_000_000, -3_600),
        parents: Some(vec![id('e'), id('d')]),
    }
}

#[test]
fn a_marker_comes_back_unchanged_from_both_sides() -> anyhow::Result<()> {
    let mut store = in_memory_db();
    store.write(&[marker()])?;

    assert_eq!(store.find_by_prec(id('a'))?, vec![marker()]);
    assert_eq!(store.find_by_succ(id('b'))?, vec![marker()]);
    assert_eq!(store.find_by_succ(id('c'))?, vec![marker()]);
    assert_eq!(store.find_by_prec(id('b'))?, vec![]);
    Ok(())
}

#[test]
fn writing_a_marker_twice_stores_it_once() -> anyhow::Result<()> {
    let mut store = in_memory_db();
    store.write(&[marker(), marker()])?;
    store.write(&[marker()])?;

    assert_eq!(store.find_by_prec(id('a'))?, vec![marker()]);
    Ok(())
}

#[test]
fn any_differing_field_is_another_marker() -> anyhow::Result<()> {
    let mut store = in_memory_db();
    let variants = [
        Marker {
            succs: vec![id('b'), id('c')],
            ..marker()
        },
        Marker {
            date: gix::date::Time::new(1_700_000_000, 0),
            ..marker()
        },
        Marker {
            meta: BTreeMap::new(),
            ..marker()
        },
        Marker {
            parents: Some(vec![id('d'), id('e')]),
            ..marker()
        },
        Marker {
            parents: Some(vec![]),
            ..marker()
        },
        Marker {
            parents: None,
            ..marker()
        },
    ];
    store.write(&variants)?;
    store.write(&[marker()])?;

    assert_eq!(
        store.find_by_prec(id('a'))?.len(),
        variants.len() + 1,
        "all of these hash differently, including the two ways of having no parents"
    );
    Ok(())
}

#[test]
fn the_storage_id_never_changes() -> anyhow::Result<()> {
    let mut store = in_memory_db();
    let unrecorded_parents = Marker {
        parents: None,
        ..marker()
    };
    store.write(&[marker(), unrecorded_parents])?;

    let ids: Vec<_> = store
        .obs_markers()
        .find_by_prec(id('a').as_bytes())?
        .into_iter()
        .map(|row| gix::ObjectId::try_from(row.id.as_slice()).map(|id| id.to_string()))
        .collect::<Result<_, _>>()?;
    // Changing this means a marker stops deduplicating against rows that are already stored.
    snapbox::assert_data_eq!(
        format!("{ids:#?}"),
        snapbox::str![[r#"
[
    "672a2fc1e12279399b292cd82833459671aeba4e",
    "1aa0761bf0c519663e71c4d8d6167ebd58c99fec",
]
"#]]
    );
    Ok(())
}
