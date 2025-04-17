pub fn dedup(existing: &[&str], new: &str) -> String {
    dedup_fmt(existing, new, " ")
}

/// Makes sure that _new_ is not in _existing_ by adding a number to it.
/// the number is increased until the name is unique.
pub fn dedup_fmt(existing: &[&str], new: &str, separator: &str) -> String {
    existing
        .iter()
        .filter_map(|x| {
            x.strip_prefix(new)
                .and_then(|x| x.strip_prefix(separator).or(Some("")))
                .and_then(|x| {
                    if x.is_empty() {
                        Some(0_i32)
                    } else {
                        x.parse::<i32>().ok()
                    }
                })
        })
        .max()
        .map_or_else(
            || new.to_string(),
            |x| format!("{new}{separator}{}", x + 1_i32),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup() {
        for (existing, new, expected) in [
            (vec!["bar", "baz"], "foo", "foo"),
            (vec!["foo", "bar", "baz"], "foo", "foo 1"),
            (vec!["foo", "foo 2"], "foo", "foo 3"),
            (vec!["foo", "foo 1", "foo 2"], "foo", "foo 3"),
            (vec!["foo", "foo 1", "foo 2"], "foo 1", "foo 1 1"),
            (vec!["foo", "foo 1", "foo 2"], "foo 2", "foo 2 1"),
            (vec!["foo", "foo 1", "foo 2"], "foo 3", "foo 3"),
            (vec!["foo 2"], "foo", "foo 3"),
            (vec!["foo", "foo 1", "foo 2", "foo 4"], "foo", "foo 5"),
            (vec!["foo", "foo 0"], "foo", "foo 1"),
            (vec!["foo 0"], "foo", "foo 1"),
        ] {
            assert_eq!(dedup(&existing, new), expected);
        }
    }
}
