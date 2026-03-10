//! Pure text processing utilities for path matching and message-body extraction.

use serde_json::Value;

/// Return whether a byte belongs to a path-like token.
fn is_path_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/')
}

/// Treat `.` as a continuation only when it starts an extension-like suffix.
fn continues_path_token(hs: &[u8], idx: usize) -> bool {
    if idx >= hs.len() {
        return false;
    }
    let b = hs[idx];
    if b == b'.' {
        return idx + 1 < hs.len() && hs[idx + 1].is_ascii_alphanumeric();
    }
    is_path_token_byte(b)
}

/// Whole-token path match that avoids substring false positives.
pub(crate) fn contains_path_token(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() || haystack.is_empty() {
        return false;
    }
    let hs = haystack.as_bytes();
    let mut start = 0usize;
    while let Some(rel) = haystack.get(start..).and_then(|s| s.find(needle)) {
        let i = start + rel;
        let j = i + needle.len();

        let before_ok = i == 0 || !is_path_token_byte(hs[i - 1]);
        let after_ok = j == hs.len() || !continues_path_token(hs, j);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
        if start >= hs.len() {
            break;
        }
    }
    false
}

/// Build search needles for a path, including ancestor directories.
pub(crate) fn relevant_needles_for_path(path: &str) -> Vec<String> {
    let mut needles = vec![
        path.to_owned(),
        format!("{path}/"),
        format!("./{path}"),
        format!("./{path}/"),
    ];
    let mut cur = path;
    while let Some((parent, _)) = cur.rsplit_once('/') {
        if parent.is_empty() {
            break;
        }
        needles.push(parent.to_owned());
        needles.push(format!("{parent}/"));
        needles.push(format!("./{parent}"));
        needles.push(format!("./{parent}/"));
        cur = parent;
    }
    needles
}

/// Extract the human-readable text from a stored message body.
pub(crate) fn extract_message_text(body_v: &Value, body_json: &str) -> String {
    if let Some(t) = body_v.get("text").and_then(Value::as_str) {
        return t.to_owned();
    }
    if let Some(s) = body_v.as_str() {
        return s.to_owned();
    }
    body_json.to_owned()
}

/// Parse a stored message body JSON and return the parsed object plus extracted text.
pub(crate) fn parse_body(body_json: &str) -> (Value, String) {
    let parsed: Value =
        serde_json::from_str(body_json).unwrap_or(Value::String(body_json.to_owned()));
    let obj = match parsed {
        Value::Object(m) => Value::Object(m),
        other => serde_json::json!({ "raw": other }),
    };
    let text = extract_message_text(&obj, body_json);
    (obj, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_path_token_exact_match() {
        assert!(contains_path_token("editing src/foo.rs now", "src/foo.rs"));
    }

    #[test]
    fn contains_path_token_no_substring_match() {
        assert!(!contains_path_token(
            "editing src/foo.rs.bak now",
            "src/foo.rs"
        ));
    }

    #[test]
    fn contains_path_token_at_boundaries() {
        assert!(contains_path_token("src/foo.rs", "src/foo.rs"));
        assert!(contains_path_token("[src/foo.rs]", "src/foo.rs"));
        assert!(contains_path_token("path: src/foo.rs.", "src/foo.rs"));
    }

    #[test]
    fn contains_path_token_empty() {
        assert!(!contains_path_token("something", ""));
        assert!(!contains_path_token("", "needle"));
    }

    #[test]
    fn relevant_needles_include_ancestors() {
        let needles = relevant_needles_for_path("src/lib/foo.rs");
        assert!(needles.contains(&String::from("src/lib/foo.rs")));
        assert!(needles.contains(&String::from("src/lib")));
        assert!(needles.contains(&String::from("./src")));
    }

    #[test]
    fn parse_body_text_field() {
        let (_, text) = parse_body(r#"{"text":"hello world"}"#);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn parse_body_raw_string() {
        let (obj, _text) = parse_body(r#""just a string""#);
        assert_eq!(obj["raw"], Value::String("just a string".to_owned()));
    }

    #[test]
    fn parse_body_malformed_json() {
        let (_, text) = parse_body("not json at all");
        assert_eq!(text, "not json at all");
    }
}
