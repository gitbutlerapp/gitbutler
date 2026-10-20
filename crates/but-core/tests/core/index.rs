use bstr::ByteSlice;
use but_core::index::{ConflictStage, Edit, Stat};
use but_testsupport::writable_scenario;
use gix::index::entry::{Flags, Mode};

fn id(hex: &str) -> gix::ObjectId {
    gix::ObjectId::from_hex(hex.as_bytes()).expect("valid hex")
}

fn old_id() -> gix::ObjectId {
    id("1111111111111111111111111111111111111111")
}

fn new_id() -> gix::ObjectId {
    id("2222222222222222222222222222222222222222")
}

/// An index with an unconflicted entry for each of `paths`.
fn index_with(paths: &[&str]) -> gix::index::State {
    let mut state = gix::index::State::new(gix::hash::Kind::Sha1);
    for path in paths {
        state.dangerously_push_entry(
            Default::default(),
            old_id(),
            Flags::empty(),
            Mode::FILE,
            path.as_bytes().as_bstr(),
        );
    }
    state.sort_entries();
    state
}

/// Each entry as `<path> <stage> <id>`.
fn entries(state: &gix::index::State) -> Vec<String> {
    state
        .entries()
        .iter()
        .map(|entry| {
            format!(
                "{} {:?} {}",
                entry.path(state),
                entry.flags.stage(),
                entry.id.to_hex_with_len(4)
            )
        })
        .collect()
}

#[test]
fn added_paths_replace_entries_that_clash_as_file_or_directory() {
    let mut state = index_with(&["a", "b/c", "d"]);
    let mut edit = Edit::new(&mut state);
    edit.upsert("a/x".into(), new_id(), Mode::FILE, Stat::Unknown);
    edit.upsert("b".into(), new_id(), Mode::FILE, Stat::Unknown);

    assert!(edit.finish(), "entries were replaced");
    assert_eq!(
        entries(&state),
        [
            "a/x Unconflicted 2222",
            "b Unconflicted 2222",
            "d Unconflicted 1111"
        ],
        "an index can't hold a path as both a file and a directory"
    );
}

#[test]
fn later_edits_of_a_path_replace_earlier_ones() {
    let mut state = index_with(&["a", "b"]);
    let mut edit = Edit::new(&mut state);
    edit.remove("a".into());
    edit.upsert("a".into(), new_id(), Mode::FILE, Stat::Unknown);
    edit.set_conflict_stage("b".into(), ConflictStage::Theirs, Mode::FILE, new_id());
    edit.upsert("b".into(), new_id(), Mode::FILE, Stat::Unknown);

    assert!(edit.finish(), "both paths changed");
    assert_eq!(
        entries(&state),
        ["a Unconflicted 2222", "b Unconflicted 2222"],
        "only the last edit of each path applies"
    );
}

#[test]
fn later_additions_replace_earlier_ones_that_clash_as_file_or_directory() {
    for (first, second) in [("a", "a/b"), ("a/b", "a")] {
        let mut state = index_with(&[]);
        let mut edit = Edit::new(&mut state);
        edit.upsert(first.into(), old_id(), Mode::FILE, Stat::Unknown);
        edit.upsert(second.into(), new_id(), Mode::FILE, Stat::Unknown);
        edit.finish();

        assert_eq!(
            entries(&state),
            [format!("{second} Unconflicted 2222")],
            "an index can't hold {first} and {second}, so the later addition wins"
        );
    }

    let mut state = index_with(&[]);
    let mut edit = Edit::new(&mut state);
    edit.upsert("a".into(), old_id(), Mode::FILE, Stat::Unknown);
    edit.set_conflict_stage("a/b".into(), ConflictStage::Ours, Mode::FILE, new_id());
    edit.finish();
    assert_eq!(
        entries(&state),
        ["a Unconflicted 1111", "a/b Ours 2222"],
        "like Git, only entries at the same stage clash, so a conflict stage keeps an unconflicted addition"
    );
}

#[test]
fn a_file_and_a_directory_may_share_a_path_at_different_conflict_stages() {
    for (first, second) in [
        (("a", ConflictStage::Ours), ("a/b", ConflictStage::Theirs)),
        (("a/b", ConflictStage::Theirs), ("a", ConflictStage::Ours)),
    ] {
        let mut state = index_with(&[]);
        let mut edit = Edit::new(&mut state);
        edit.set_conflict_stage(first.0.into(), first.1, Mode::FILE, new_id());
        edit.set_conflict_stage(second.0.into(), second.1, Mode::FILE, new_id());
        edit.finish();

        assert_eq!(
            entries(&state),
            ["a Ours 2222", "a/b Theirs 2222"],
            "Git keeps both sides of a file/directory conflict, as they are at different stages"
        );
    }
}

#[test]
fn a_clashing_addition_replaces_only_the_stage_it_clashes_at() {
    let mut state = index_with(&[]);
    let mut edit = Edit::new(&mut state);
    edit.set_conflict_stage("a".into(), ConflictStage::Ours, Mode::FILE, old_id());
    edit.set_conflict_stage("a".into(), ConflictStage::Theirs, Mode::FILE, old_id());
    edit.set_conflict_stage("a/b".into(), ConflictStage::Ours, Mode::FILE, new_id());
    edit.finish();

    assert_eq!(
        entries(&state),
        ["a Theirs 1111", "a/b Ours 2222"],
        "like Git, a keeps the stage that a/b doesn't clash with"
    );
}

#[test]
fn a_displaced_addition_still_replaces_the_entries_of_its_path() {
    let mut state = index_with(&[]);
    let mut edit = Edit::new(&mut state);
    edit.set_conflict_stage("a".into(), ConflictStage::Ours, Mode::FILE, old_id());
    edit.set_conflict_stage("a".into(), ConflictStage::Theirs, Mode::FILE, old_id());
    edit.finish();

    let mut edit = Edit::new(&mut state);
    edit.upsert("a".into(), new_id(), Mode::FILE, Stat::Unknown);
    edit.upsert("a/b".into(), new_id(), Mode::FILE, Stat::Unknown);
    assert!(
        !edit.contains("a".into()),
        "a was resolved, then replaced by the directory a"
    );
    edit.finish();

    assert_eq!(
        entries(&state),
        ["a/b Unconflicted 2222"],
        "the conflict stages of a don't come back when its resolution is displaced"
    );
}

#[test]
fn existing_entries_stay_when_an_addition_clashes_only_at_another_stage() {
    let mut state = index_with(&["a", "c/d"]);
    let mut edit = Edit::new(&mut state);
    edit.set_conflict_stage("a/b".into(), ConflictStage::Ours, Mode::FILE, new_id());
    edit.set_conflict_stage("c".into(), ConflictStage::Theirs, Mode::FILE, new_id());

    assert!(
        edit.contains("a".into()) && edit.contains("c/d".into()),
        "unconflicted entries don't clash with conflict stages"
    );
    edit.finish();
    assert_eq!(
        entries(&state),
        [
            "a Unconflicted 1111",
            "a/b Ours 2222",
            "c Theirs 2222",
            "c/d Unconflicted 1111"
        ]
    );
}

#[test]
fn contains_sees_entries_displaced_by_pending_additions() {
    let mut state = index_with(&["a/b", "c"]);
    let mut edit = Edit::new(&mut state);
    edit.upsert("a".into(), new_id(), Mode::FILE, Stat::Unknown);
    edit.upsert("c/d".into(), new_id(), Mode::FILE, Stat::Unknown);

    assert!(
        !edit.contains("a/b".into()),
        "adding the file a removes what was in the directory a"
    );
    assert!(
        !edit.contains("c".into()),
        "adding c/d removes the file that was at c"
    );
    assert!(edit.contains("a".into()) && edit.contains("c/d".into()));
}

#[test]
fn contains_sees_pending_edits() {
    let mut state = index_with(&["a", "b"]);
    let mut edit = Edit::new(&mut state);
    edit.remove("a".into());
    edit.upsert("c".into(), new_id(), Mode::FILE, Stat::Unknown);

    assert!(!edit.contains("a".into()), "a removed path is gone");
    assert!(edit.contains("b".into()), "an untouched path stays");
    assert!(edit.contains("c".into()), "an added path is there");
    assert!(!edit.contains("d".into()), "an unknown path isn't");
}

#[test]
fn upserts_record_the_stat_they_are_given() {
    let mut state = index_with(&[]);
    let stat = gix::index::entry::Stat {
        size: 42,
        ..Default::default()
    };
    let mut edit = Edit::new(&mut state);
    edit.upsert("known".into(), new_id(), Mode::FILE, Stat::Known(stat));
    edit.upsert("unknown".into(), new_id(), Mode::FILE, Stat::Unknown);
    edit.finish();

    let stat_of = |path: &str| state.entry_by_path(path.into()).expect("recorded").stat;
    assert_eq!(
        stat_of("known"),
        stat,
        "a known stat lets Git skip re-hashing the file"
    );
    assert_eq!(
        stat_of("unknown"),
        Default::default(),
        "an unknown stat makes Git compare the content"
    );
}

#[test]
fn finish_reports_no_change_when_nothing_changed() {
    let mut state = index_with(&["a"]);
    let mut edit = Edit::new(&mut state);
    edit.remove("missing".into());

    assert!(
        !edit.finish(),
        "removing a path the index doesn't have changes nothing, so the index needn't be written"
    );
    assert_eq!(entries(&state), ["a Unconflicted 1111"]);
}

#[test]
fn edits_drop_the_cached_data_they_make_stale() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");
    but_testsupport::invoke_bash(
        r#"
        git_commit() { git -c user.name=a -c user.email=a@example.com commit -q "$@"; }
        echo base >f && git add f && git_commit -m base
        git checkout -qb other && echo theirs >f && git_commit -am theirs
        git checkout -q - && echo ours >f && git_commit -am ours
        git -c user.name=a -c user.email=a@example.com merge -q other >/dev/null 2>&1 || true
        echo resolved >f && git add f && git write-tree >/dev/null
        "#,
        &repo,
    );
    let mut index = repo.open_index()?;
    assert!(
        index.tree().is_some() && index.resolve_undo().is_some(),
        "Git cached the trees and remembers the resolved conflict"
    );

    let mut edit = Edit::new(&mut index);
    edit.upsert("g".into(), new_id(), Mode::FILE, Stat::Unknown);
    assert!(edit.finish(), "a path was added");

    assert!(
        index.tree().is_none(),
        "cached trees no longer match the entries, and Git would trust them when committing"
    );
    assert!(
        index.resolve_undo().is_none(),
        "the recorded conflict no longer matches the entries"
    );
    Ok(())
}
