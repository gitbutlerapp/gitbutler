/// Tests for gix blob-merge behaviour.
///
/// Some tests in this module are expected to **fail** until an upstream gix bug
/// is resolved.  Those tests are marked `#[should_panic]` so the test suite
/// stays green in the interim; once gix is fixed the annotation must be removed.
use gix::merge::blob::{
    Resolution,
    builtin_driver::text::{Conflict, ConflictStyle, Labels, Options},
};

fn fixtures() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merge");
    assert!(dir.exists(), "fixtures directory missing: {dir:?}");
    dir
}

/// gix's Myers blob merge produces a false conflict on certain inputs where
/// `git merge-file` (also Myers-based) resolves cleanly.
///
/// Scenario (minimal reproduction – 5/4/4 lines)
/// -----------------------------------------------
/// * **base** – `alpha_x`, blank, `bravo_x`, `charlie_x`, blank
/// * **ours** – blank, blank, `bravo_x`, `charlie_x`
///   (i.e. `alpha_x` deleted, one blank added, trailing blank removed)
/// * **theirs** – `alpha_x`, blank, `charlie_x`, blank
///   (i.e. `bravo_x` deleted)
///
/// `git merge-file -p ours base theirs` exits 0 (clean merge).
/// imara-diff's Myers implementation chooses different hunk boundaries when
/// blank lines create alignment ambiguity, causing the 3-way merge to see
/// the `bravo_x` deletion as overlapping with the `alpha_x` area changes.
///
/// Upstream issue: https://github.com/GitoxideLabs/gitoxide/issues/2475
///
/// Remove `#[should_panic]` once the upstream fix lands.
#[test]
#[should_panic(expected = "https://github.com/GitoxideLabs/gitoxide/issues/2475")]
fn myers_blob_merge_false_conflict_with_large_insertion_and_adjacent_deletion() {
    let dir = fixtures();
    let base = std::fs::read(dir.join("base.txt")).unwrap();
    let ours = std::fs::read(dir.join("ours.txt")).unwrap();
    let theirs = std::fs::read(dir.join("theirs.txt")).unwrap();

    let labels = Labels {
        ancestor: Some("base".into()),
        current: Some("ours".into()),
        other: Some("theirs".into()),
    };
    let options = Options {
        diff_algorithm: gix::diff::blob::Algorithm::Myers,
        conflict: Conflict::Keep {
            style: ConflictStyle::Merge,
            marker_size: std::num::NonZeroU8::new(7).unwrap(),
        },
    };

    let mut out = Vec::new();
    let mut input = gix::diff::blob::intern::InternedInput::new(&[][..], &[][..]);
    let resolution = gix::merge::blob::builtin_driver::text(
        &mut out, &mut input, labels, &ours, &base, &theirs, options,
    );

    assert_eq!(
        resolution,
        Resolution::Complete,
        "gix Myers blob merge should resolve cleanly (upstream bug: \
         https://github.com/GitoxideLabs/gitoxide/issues/2475)"
    );
}

/// Sanity check: the Histogram algorithm already resolves the same input
/// without conflicts.  This test must always pass.
#[test]
fn histogram_blob_merge_resolves_large_insertion_and_adjacent_deletion() {
    let dir = fixtures();
    let base = std::fs::read(dir.join("base.txt")).unwrap();
    let ours = std::fs::read(dir.join("ours.txt")).unwrap();
    let theirs = std::fs::read(dir.join("theirs.txt")).unwrap();

    let labels = Labels {
        ancestor: Some("base".into()),
        current: Some("ours".into()),
        other: Some("theirs".into()),
    };
    let options = Options {
        diff_algorithm: gix::diff::blob::Algorithm::Histogram,
        conflict: Conflict::Keep {
            style: ConflictStyle::Merge,
            marker_size: std::num::NonZeroU8::new(7).unwrap(),
        },
    };

    let mut out = Vec::new();
    let mut input = gix::diff::blob::intern::InternedInput::new(&[][..], &[][..]);
    let resolution = gix::merge::blob::builtin_driver::text(
        &mut out, &mut input, labels, &ours, &base, &theirs, options,
    );

    assert_eq!(
        resolution,
        Resolution::Complete,
        "Histogram should merge without conflicts"
    );
    assert!(
        !String::from_utf8_lossy(&out).contains("<<<<<<<"),
        "Histogram merge output should contain no conflict markers"
    );
}
