use std::collections::HashSet;

// dedup makes sure that _new_ is not in _existing_ by adding a number to it.
// the number is increased until the name is unique.
pub fn dedup(existing: &[&str], new: &str) -> String {
    let used_numbers = existing
        .iter()
        .filter(|x| x.starts_with(new))
        .filter_map(|x| {
            x.strip_prefix(new)
                .map(|x| x.trim_start())
                .map(|x| x.parse::<i32>().unwrap_or(-1))
        })
        .collect::<HashSet<_>>();
    if used_numbers.is_empty() || !used_numbers.contains(&-1) {
        new.to_string()
    } else {
        // pick first unused number
        let mut number = 1;
        while used_numbers.contains(&number) {
            number += 1;
        }
        format!("{} {}", new, number)
    }
}

#[test]
fn test_dedup() {
    vec![
        (vec!["foo", "foo 2"], "foo", "foo 1"),
        (vec!["foo", "foo 1", "foo 2"], "foo", "foo 3"),
        (vec!["foo", "foo 1", "foo 2"], "foo 1", "foo 1 1"),
        (vec!["foo", "foo 1", "foo 2"], "foo 2", "foo 2 1"),
        (vec!["foo", "foo 1", "foo 2"], "foo 3", "foo 3"),
        (vec!["foo 2"], "foo", "foo"),
        (vec!["foo", "foo 1", "foo 2", "foo 4"], "foo", "foo 3"),
    ]
    .iter()
    .enumerate()
    .for_each(|(i, (existing, new, expected))| {
        assert_eq!(dedup(existing, new), expected.to_string(), "test {}", i + 1);
    });
}
