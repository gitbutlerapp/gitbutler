use bstr::BString;
use but_core::{DiffSpec, HunkHeader};

use super::*;

#[test]
fn empty() {
    let input = vec![];
    let result = flatten_diff_specs(input);
    assert!(result.is_empty());
}

#[test]
fn single() {
    let spec = DiffSpec {
        path: BString::from("file.txt"),
        previous_path: None,
        hunk_headers: vec![HunkHeader {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 3,
        }],
    };
    let input = vec![spec.clone()];
    let result = flatten_diff_specs(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result.first().unwrap(), &spec);
}

#[test]
fn different_files() {
    let spec1 = DiffSpec {
        path: BString::from("file1.txt"),
        previous_path: None,
        hunk_headers: vec![HunkHeader {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 3,
        }],
    };
    let spec2 = DiffSpec {
        path: BString::from("file2.txt"),
        previous_path: None,
        hunk_headers: vec![HunkHeader {
            old_start: 5,
            old_lines: 1,
            new_start: 5,
            new_lines: 2,
        }],
    };
    let input = vec![spec1.clone(), spec2.clone()];
    let result = flatten_diff_specs(input);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&spec1));
    assert!(result.contains(&spec2));
}

#[test]
fn same_file_merge_hunks() {
    let hunk1 = HunkHeader {
        old_start: 1,
        old_lines: 2,
        new_start: 1,
        new_lines: 3,
    };
    let hunk2 = HunkHeader {
        old_start: 10,
        old_lines: 1,
        new_start: 11,
        new_lines: 2,
    };

    let spec1 = DiffSpec {
        path: BString::from("file.txt"),
        previous_path: None,
        hunk_headers: vec![hunk1],
    };
    let spec2 = DiffSpec {
        path: BString::from("file.txt"),
        previous_path: None,
        hunk_headers: vec![hunk2],
    };

    let input = vec![spec1, spec2];
    let result = flatten_diff_specs(input);

    assert_eq!(result.len(), 1);
    assert_eq!(result.first().unwrap().path, BString::from("file.txt"));
    assert_eq!(result.first().unwrap().previous_path, None);
    assert_eq!(result.first().unwrap().hunk_headers.len(), 2);
    assert!(result.first().unwrap().hunk_headers.contains(&hunk1));
    assert!(result.first().unwrap().hunk_headers.contains(&hunk2));
}

#[test]
fn duplicate_hunks_are_collapsed() {
    let hunk = HunkHeader {
        old_start: 1,
        old_lines: 2,
        new_start: 1,
        new_lines: 3,
    };
    let spec = DiffSpec {
        path: "file.txt".into(),
        previous_path: None,
        hunk_headers: vec![hunk],
    };

    assert_eq!(flatten_diff_specs([spec.clone(), spec.clone()]), vec![spec]);
}

#[test]
fn whole_file_supersedes_hunks_for_the_same_path() {
    let hunk = DiffSpec {
        path: "file.txt".into(),
        previous_path: None,
        hunk_headers: vec![HunkHeader {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
        }],
    };
    let whole_file = DiffSpec {
        path: "file.txt".into(),
        previous_path: None,
        hunk_headers: Vec::new(),
    };

    for input in [
        vec![hunk.clone(), whole_file.clone()],
        vec![whole_file.clone(), hunk.clone()],
    ] {
        let result = flatten_diff_specs(input);
        assert_eq!(result, vec![whole_file.clone()]);
    }
}

#[test]
fn with_previous_path() {
    let spec1 = DiffSpec {
        path: BString::from("new_file.txt"),
        previous_path: Some(BString::from("old_file.txt")),
        hunk_headers: vec![HunkHeader {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 3,
        }],
    };
    let spec2 = DiffSpec {
        path: BString::from("new_file.txt"),
        previous_path: None,
        hunk_headers: vec![HunkHeader {
            old_start: 5,
            old_lines: 1,
            new_start: 5,
            new_lines: 2,
        }],
    };

    let input = vec![spec1.clone(), spec2.clone()];
    let result = flatten_diff_specs(input);

    // These should remain separate because they have different previous_path values
    assert_eq!(result.len(), 2);
    assert!(result.contains(&spec1));
    assert!(result.contains(&spec2));
}

#[test]
fn paths_containing_separators_do_not_collide() {
    let path_with_separator = DiffSpec {
        path: "a:b".into(),
        previous_path: None,
        hunk_headers: Vec::new(),
    };
    let rename = DiffSpec {
        path: "a".into(),
        previous_path: Some("b".into()),
        hunk_headers: vec![HunkHeader {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
        }],
    };

    assert_eq!(
        flatten_diff_specs([path_with_separator.clone(), rename.clone()]),
        vec![path_with_separator, rename]
    );
}

#[test]
fn same_previous_path() {
    let hunk1 = HunkHeader {
        old_start: 1,
        old_lines: 2,
        new_start: 1,
        new_lines: 3,
    };
    let hunk2 = HunkHeader {
        old_start: 10,
        old_lines: 1,
        new_start: 11,
        new_lines: 2,
    };

    let spec1 = DiffSpec {
        path: BString::from("new_file.txt"),
        previous_path: Some(BString::from("old_file.txt")),
        hunk_headers: vec![hunk1],
    };
    let spec2 = DiffSpec {
        path: BString::from("new_file.txt"),
        previous_path: Some(BString::from("old_file.txt")),
        hunk_headers: vec![hunk2],
    };

    let input = vec![spec1, spec2];
    let result = flatten_diff_specs(input);

    assert_eq!(result.len(), 1);
    assert_eq!(result.first().unwrap().path, BString::from("new_file.txt"));
    assert_eq!(
        result.first().unwrap().previous_path,
        Some(BString::from("old_file.txt"))
    );
    assert_eq!(result.first().unwrap().hunk_headers.len(), 2);
    assert!(result.first().unwrap().hunk_headers.contains(&hunk1));
    assert!(result.first().unwrap().hunk_headers.contains(&hunk2));
}
