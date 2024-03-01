use std::collections::HashSet;

pub fn dedup(existing: &[&str], new: &str) -> String {
    dedup_fmt(existing, new, " ")
}

/// Makes sure that _new_ is not in _existing_ by adding a number to it.
/// the number is increased until the name is unique.
pub fn dedup_fmt(existing: &[&str], new: &str, separator: &str) -> String {
    let used_numbers = existing
        .iter()
        .filter(|x| x.starts_with(new))
        .filter_map(|x| {
            x.strip_prefix(new)
                .and_then(|x| x.strip_prefix(separator).or(Some(x)))
                .map(|x| x.parse::<i32>().unwrap_or(-1_i32))
        })
        .collect::<HashSet<_>>();
    if used_numbers.is_empty() || !used_numbers.contains(&-1_i32) {
        new.to_string()
    } else {
        // pick first unused number
        let mut number = 1_i32;
        while used_numbers.contains(&number) {
            number += 1_i32;
        }
        format!("{}{}{}", new, separator, number)
    }
}

#[test]
fn test_dedup() {
    for (existing, new, expected) in [
        (vec!["foo", "foo 2"], "foo", "foo 1"),
        (vec!["foo", "foo 1", "foo 2"], "foo", "foo 3"),
        (vec!["foo", "foo 1", "foo 2"], "foo 1", "foo 1 1"),
        (vec!["foo", "foo 1", "foo 2"], "foo 2", "foo 2 1"),
        (vec!["foo", "foo 1", "foo 2"], "foo 3", "foo 3"),
        (vec!["foo 2"], "foo", "foo"),
        (vec!["foo", "foo 1", "foo 2", "foo 4"], "foo", "foo 3"),
    ] {
        assert_eq!(dedup(&existing, new), expected.to_string());
    }
}
