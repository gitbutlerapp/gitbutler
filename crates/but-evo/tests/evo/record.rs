use anyhow::Result;
use but_evo::{Marker, MarkerStore, Provenance, Rewrite, record};
use but_testsupport::{in_memory_db, read_only_in_memory_scenario};
use gix::bstr::ByteSlice as _;

fn repo() -> Result<gix::Repository> {
    read_only_in_memory_scenario("rewrites")
}

fn tag(repo: &gix::Repository, name: &str) -> gix::ObjectId {
    repo.rev_parse_single(name)
        .unwrap_or_else(|err| panic!("{name} is in the fixture: {err}"))
        .detach()
}

fn provenance() -> Provenance {
    Provenance {
        user: "Caleb Owens <caleb@gitbutler.com>".into(),
        date: gix::date::Time::new(1_700_000_000, 3_600),
        operation: Some("editor op".into()),
    }
}

fn ef1_of_replacing_prec_with(repo: &gix::Repository, succ: &str) -> Result<String> {
    let mut store = in_memory_db();
    let prec = tag(repo, "prec");
    let rewrite = Rewrite::Replace {
        prec,
        succ: tag(repo, succ),
    };
    record(&mut store, repo, &[rewrite], &provenance())?;
    let [marker] = store.find_by_prec(prec)?.try_into().expect("one marker");
    Ok(marker.meta[b"ef1".as_bstr()].to_string())
}

fn name_of(repo: &gix::Repository, id: gix::ObjectId) -> String {
    let references = repo.references().expect("readable");
    let name = references
        .tags()
        .expect("readable")
        .filter_map(Result::ok)
        .find(|tag| tag.target().try_id() == Some(id.as_ref()))
        .map(|tag| tag.name().shorten().to_string());
    name.unwrap_or_else(|| id.to_string())
}

fn render(repo: &gix::Repository, markers: &[Marker]) -> String {
    let names = |ids: &[gix::ObjectId]| {
        ids.iter()
            .map(|id| name_of(repo, *id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = String::new();
    for marker in markers {
        let parents = names(marker.parents.as_deref().expect("always recorded"));
        out.push_str(&format!(
            "{} -> [{}] parents: [{}] date: {}\n",
            name_of(repo, marker.prec),
            names(&marker.succs),
            parents,
            marker.date.seconds,
        ));
        for (key, value) in &marker.meta {
            out.push_str(&format!("  {key}: {value}\n"));
        }
    }
    out
}

#[test]
fn ef1_names_what_changed() -> Result<()> {
    let repo = repo()?;
    let cases = [
        ("an untracked header", "with-headers-version", "0"),
        ("a change id", "with-change-id", "2"),
        ("a legacy change id", "with-legacy-change-id", "2"),
        ("description", "reworded", "1"),
        ("extra header", "with-header", "2"),
        ("parents, same blobs", "rebased-over-unrelated-file", "4"),
        (
            "parents, blobs differ but the hunk doesn't",
            "rebased-with-shifted-lines",
            "4",
        ),
        (
            "parents and the context of the hunk",
            "rebased-over-context-change",
            "12",
        ),
        ("content", "amended", "8"),
        ("author", "other-author", "16"),
        ("author date", "other-date", "32"),
        (
            "a rename isn't the same file change",
            "renames-the-file",
            "8",
        ),
        (
            "a mode change isn't the same file change",
            "also-executable",
            "8",
        ),
        ("one more file", "also-adds-a-file", "8"),
    ];
    for (what, succ, expected) in cases {
        assert_eq!(ef1_of_replacing_prec_with(&repo, succ)?, expected, "{what}");
    }
    Ok(())
}

#[test]
fn a_prune_has_no_effect_flags_and_is_anchored_to_its_parents() -> Result<()> {
    let repo = repo()?;
    let mut store = in_memory_db();
    let prec = tag(&repo, "prec");
    record(
        &mut store,
        &repo,
        &[Rewrite::Prune { prec }],
        &Provenance {
            operation: None,
            ..provenance()
        },
    )?;
    // Without an operation there's no `operation` key at all.
    snapbox::assert_data_eq!(
        render(&repo, &store.find_by_prec(prec)?),
        snapbox::str![[r#"
prec -> [] parents: [base] date: 1700000000
  ef1: 0
  user: Caleb Owens <caleb@gitbutler.com>

"#]]
    );
    Ok(())
}

#[test]
fn a_root_commit_has_recorded_empty_parents() -> Result<()> {
    let repo = repo()?;
    let mut store = in_memory_db();
    let prec = tag(&repo, "base");
    record(&mut store, &repo, &[Rewrite::Prune { prec }], &provenance())?;

    let [marker] = store.find_by_prec(prec)?.try_into().expect("one marker");
    assert_eq!(marker.parents, Some(vec![]), "not `None`, which is unknown");
    Ok(())
}

#[test]
fn a_fold_writes_one_numbered_marker_per_prec() -> Result<()> {
    let repo = repo()?;
    let mut store = in_memory_db();
    let succ = tag(&repo, "amended");
    let rewrite = Rewrite::Fold {
        precs: vec![tag(&repo, "reworded"), tag(&repo, "empty")],
        succ,
    };
    record(&mut store, &repo, &[rewrite], &provenance())?;
    // `fold-id` is shared, and `ef1` is computed against each `prec` on its own.
    snapbox::assert_data_eq!(
        render(&repo, &store.find_by_succ(succ)?),
        snapbox::str![[r#"
reworded -> [amended] parents: [base] date: 1700000000
  ef1: 9
  fold-id: c4f70aa8
  fold-idx: 1
  fold-size: 2
  operation: editor op
  user: Caleb Owens <caleb@gitbutler.com>
empty -> [amended] parents: [base] date: 1700000000
  ef1: 9
  fold-id: c4f70aa8
  fold-idx: 2
  fold-size: 2
  operation: editor op
  user: Caleb Owens <caleb@gitbutler.com>

"#]]
    );
    Ok(())
}

#[test]
fn a_split_ors_its_effects_across_successors() -> Result<()> {
    let repo = repo()?;
    let mut store = in_memory_db();
    let prec = tag(&repo, "prec");
    let rewrite = Rewrite::Split {
        prec,
        succs: vec![tag(&repo, "other-date"), tag(&repo, "reworded")],
    };
    record(&mut store, &repo, &[rewrite], &provenance())?;
    // Successors stay in the given order, and `ef1` is 32 | 1.
    snapbox::assert_data_eq!(
        render(&repo, &store.find_by_prec(prec)?),
        snapbox::str![[r#"
prec -> [other-date, reworded] parents: [base] date: 1700000000
  ef1: 33
  operation: editor op
  user: Caleb Owens <caleb@gitbutler.com>

"#]]
    );
    Ok(())
}

#[test]
fn a_prec_that_is_also_a_successor_survived_and_gets_no_marker() -> Result<()> {
    let repo = repo()?;
    let mut store = in_memory_db();
    let (kept, other) = (tag(&repo, "prec"), tag(&repo, "reworded"));
    let rewrites = [
        Rewrite::Replace {
            prec: kept,
            succ: kept,
        },
        Rewrite::Split {
            prec: kept,
            succs: vec![kept, other],
        },
        Rewrite::Fold {
            precs: vec![kept, other],
            succ: kept,
        },
    ];
    record(&mut store, &repo, &rewrites, &provenance())?;

    assert_eq!(store.find_by_prec(kept)?, vec![], "nothing replaced it");
    // A fold that lost its self-edge has a single `prec`, so it's no longer numbered.
    snapbox::assert_data_eq!(
        render(&repo, &store.find_by_prec(other)?),
        snapbox::str![[r#"
reworded -> [prec] parents: [base] date: 1700000000
  ef1: 1
  operation: editor op
  user: Caleb Owens <caleb@gitbutler.com>

"#]]
    );
    Ok(())
}

#[test]
fn recording_the_same_rewrite_twice_stores_it_once() -> Result<()> {
    let repo = repo()?;
    let mut store = in_memory_db();
    let prec = tag(&repo, "prec");
    let rewrites = [Rewrite::Prune { prec }];
    record(&mut store, &repo, &rewrites, &provenance())?;
    record(&mut store, &repo, &rewrites, &provenance())?;

    assert_eq!(store.find_by_prec(prec)?.len(), 1, "markers are values");
    Ok(())
}
