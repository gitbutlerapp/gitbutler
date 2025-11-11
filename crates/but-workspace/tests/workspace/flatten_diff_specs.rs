use super::*;
use bstr::BString;
use but_core::{DiffSpec, HunkHeader};

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
