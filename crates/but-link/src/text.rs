//! Pure text processing utilities for path matching, ack/closure detection, and discovery analysis.
//!
//! All functions in this module are side-effect free and do not depend on the database.

use serde_json::Value;

#[cfg(test)]
use serde::Serialize;

#[cfg(test)]
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DiscoveryBlocker {
    pub agent_id: String,
    pub created_at_ms: i64,
    pub kind: String,
    pub body: Value,
    pub text: String,
}

#[cfg(test)]
#[allow(dead_code)]
const DISCOVERY_BLOCK_KEYWORDS: [&str; 10] = [
    "please avoid",
    "avoid ",
    "avoid.",
    "avoid!",
    "blocked",
    "blocking",
    "do not",
    "don't touch",
    "skip",
    "refactor",
];

fn is_path_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/')
}

/// For boundary checks: treat "." as a continuation only when it starts an extension-like suffix.
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

/// Whole-token path match: avoids substring false positives like `src/app.txt.bak`.
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

/// Build a set of search needles for a path, including all ancestor directories.
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
    if let Some(t) = body_v.get("text").and_then(|v| v.as_str()) {
        return t.to_owned();
    }
    if let Some(s) = body_v.as_str() {
        return s.to_owned();
    }
    body_json.to_owned()
}

/// Parse a stored message body JSON and return (parsed object, extracted text).
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
/// Strip common list-item prefixes like `- `, `* `, `1. `, `- [x] `.
pub(crate) fn strip_common_list_prefix(line: &str) -> &str {
    let b = line.as_bytes();
    if b.is_empty() {
        return line;
    }

    let mut i: usize = 0;
    if matches!(b[0], b'-' | b'*' | b'+') {
        i = 1;
    }

    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }

    // Optional numeric list marker (supports nested patterns like "- 1. ...").
    let mut j = i;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    if j > i
        && j < b.len()
        && (b[j] == b'.' || b[j] == b')')
        && j + 1 < b.len()
        && (b[j + 1] == b' ' || b[j + 1] == b'\t')
    {
        i = j + 1;
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
    }

    if i == 0 {
        return line;
    }

    // GitHub-style task list marker: "- [x] ..." / "- [ ] ..."
    if i + 3 <= b.len()
        && b[i] == b'['
        && (b[i + 1] == b' ' || b[i + 1] == b'x' || b[i + 1] == b'X')
        && b[i + 2] == b']'
    {
        i += 3;
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
    }
    &line[i..]
}

#[cfg(test)]
#[allow(dead_code)]
/// Strip leading markdown emphasis (`**`, `*`, `_`) when directly followed by `@`.
pub(crate) fn strip_leading_markdown_emphasis(line: &str) -> &str {
    let b = line.as_bytes();
    if b.is_empty() {
        return line;
    }

    let mut i: usize = 0;
    while i < b.len() && (b[i] == b'*' || b[i] == b'_') {
        i += 1;
    }
    if i == 0 || i > 3 || i >= b.len() || b[i] != b'@' {
        return line;
    }
    &line[i..]
}

#[cfg(test)]
#[allow(dead_code)]
/// Strip leading wrapper characters (`(`, `[`, `{`, `` ` ``) when preceding `@`.
pub(crate) fn strip_leading_wrappers(line: &str) -> &str {
    let b = line.as_bytes();
    if b.is_empty() {
        return line;
    }

    let mut i: usize = 0;
    let mut stripped: usize = 0;
    while i < b.len() && stripped < 3 {
        match b[i] {
            b'(' | b'[' | b'{' => {
                i += 1;
                stripped += 1;
                while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
                    i += 1;
                }
            }
            b'`' => {
                if i + 1 < b.len() && b[i + 1] == b'@' {
                    i += 1;
                    stripped += 1;
                    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    &line[i..]
}

#[cfg(test)]
#[allow(dead_code)]
/// Detect indented `@`-mentions that are likely quoted context, not direct mentions.
fn is_indented_at_mention(line: &str) -> bool {
    let b = line.as_bytes();
    if b.is_empty() {
        return false;
    }
    let mut i: usize = 0;
    let mut spaces: usize = 0;
    let mut saw_tab = false;
    while i < b.len() {
        match b[i] {
            b' ' => {
                spaces += 1;
                i += 1;
            }
            b'\t' => {
                saw_tab = true;
                i += 1;
            }
            _ => break,
        }
    }
    if i >= b.len() || b[i] != b'@' {
        return false;
    }
    saw_tab || spaces >= 4
}

#[cfg(test)]
#[allow(dead_code)]
fn build_ack_needles(ack_to_me_prefix: &str) -> (Vec<String>, Vec<String>) {
    let ack_to_me_prefix_lower = ack_to_me_prefix.to_ascii_lowercase();
    let ack_to_me_prefix_space_lower = ack_to_me_prefix_lower.replacen(": ack:", " ack:", 1);
    let mut ack_needles_lower: Vec<String> = Vec::with_capacity(12);
    let mut ack_bare_lower: Vec<String> = Vec::with_capacity(4);
    for n in [
        ack_to_me_prefix_lower.clone(),
        ack_to_me_prefix_space_lower.clone(),
        ack_to_me_prefix_lower.replacen(": ack:", ": acknowledged", 1),
        ack_to_me_prefix_space_lower.replacen(" ack:", " acknowledged", 1),
        ack_to_me_prefix_lower.replacen(": ack:", ": thanks", 1),
        ack_to_me_prefix_space_lower.replacen(" ack:", " thanks", 1),
        ack_to_me_prefix_lower.replacen(": ack:", ": got it", 1),
        ack_to_me_prefix_space_lower.replacen(" ack:", " got it", 1),
    ] {
        ack_needles_lower.push(n.clone());
        if let Some(base) = n.strip_suffix(':') {
            ack_bare_lower.push(base.to_owned());
            for p in ['.', '!', '?', ','] {
                ack_needles_lower.push(format!("{base}{p}"));
            }
        }
    }
    (ack_needles_lower, ack_bare_lower)
}

#[cfg(test)]
#[allow(dead_code)]
fn build_closure_needles(closure_to_me_prefixes_lower: [&str; 3]) -> (Vec<String>, Vec<String>) {
    let mut closure_needles: Vec<String> = Vec::with_capacity(12);
    for n in closure_to_me_prefixes_lower {
        closure_needles.push(n.to_owned());
        if let Some(base) = n.strip_suffix(':') {
            for p in ['.', '!', ','] {
                closure_needles.push(format!("{base}{p}"));
            }
        }
    }
    let mut closure_bare: Vec<String> = Vec::with_capacity(4);
    for n in closure_to_me_prefixes_lower {
        if let Some(base) = n.strip_suffix(':') {
            closure_bare.push(base.to_owned());
        }
    }
    (closure_needles, closure_bare)
}

#[cfg(test)]
#[allow(dead_code)]
/// Check whether a message text contains an explicit closure directed at a specific agent.
///
/// Recognizes `@<me>: ack:`, `@<me>: resolve:`, `@<me>: resolved:`, `@<me>: released:`,
/// and common variations with punctuation, markdown, and list formatting.
pub(crate) fn is_explicit_closure_to_me(
    text: &str,
    ack_to_me_prefix: &str,
    closure_to_me_prefixes_lower: [&str; 3],
) -> bool {
    let (ack_needles_lower, ack_bare_lower) = build_ack_needles(ack_to_me_prefix);
    let (closure_needles, closure_bare) = build_closure_needles(closure_to_me_prefixes_lower);

    let mut scanned_bytes: usize = 0;
    let mut in_fenced_code_block = false;
    for line in text.lines().take(16) {
        scanned_bytes = scanned_bytes.saturating_add(line.len());
        if scanned_bytes > 1024 {
            break;
        }

        if is_indented_at_mention(line) {
            continue;
        }

        let l = line.trim_start();
        if l.starts_with("```") {
            in_fenced_code_block = !in_fenced_code_block;
            continue;
        }
        if in_fenced_code_block || l.is_empty() {
            continue;
        }

        let l_lower = l.to_ascii_lowercase();
        let l_md = strip_leading_markdown_emphasis(l);
        let l_md_lower = l_md.to_ascii_lowercase();
        let l_list_md = strip_leading_markdown_emphasis(strip_common_list_prefix(l));
        let l_list_md_lower = l_list_md.to_ascii_lowercase();

        let l_wrap = strip_leading_wrappers(l);
        let l_wrap_lower = l_wrap.to_ascii_lowercase();
        let l_md_wrap = strip_leading_wrappers(l_md);
        let l_md_wrap_lower = l_md_wrap.to_ascii_lowercase();
        let l_list_md_wrap = strip_leading_wrappers(l_list_md);
        let l_list_md_wrap_lower = l_list_md_wrap.to_ascii_lowercase();

        let is_ack_line = |raw: &str, lower: &str| {
            if ack_needles_lower.iter().any(|n| lower.starts_with(n)) {
                return true;
            }
            for base in &ack_bare_lower {
                if lower.starts_with(base) {
                    let after = raw.as_bytes().get(base.len()).copied();
                    match after {
                        None | Some(b'?' | b' ' | b'\t' | b'.' | b'!' | b',' | b':') => {
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            false
        };

        if is_ack_line(l, &l_lower)
            || is_ack_line(l_md, &l_md_lower)
            || is_ack_line(l_list_md, &l_list_md_lower)
            || is_ack_line(l_wrap, &l_wrap_lower)
            || is_ack_line(l_md_wrap, &l_md_wrap_lower)
            || is_ack_line(l_list_md_wrap, &l_list_md_wrap_lower)
        {
            return true;
        }

        let candidates: [(&str, &str); 6] = [
            (l, &l_lower),
            (l_md, &l_md_lower),
            (l_list_md, &l_list_md_lower),
            (l_wrap, &l_wrap_lower),
            (l_md_wrap, &l_md_wrap_lower),
            (l_list_md_wrap, &l_list_md_wrap_lower),
        ];
        for (c_raw, c_lower) in candidates {
            if matches_closure_needle(c_raw, c_lower, &closure_needles, &closure_bare) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
#[allow(dead_code)]
/// Check whether a line matches any closure needle (resolve/resolved/released).
fn matches_closure_needle(
    raw: &str,
    lower: &str,
    closure_needles: &[String],
    closure_bare: &[String],
) -> bool {
    for needle in closure_needles {
        if let Some(idx) = lower.find(needle.as_str()) {
            if idx > 64 {
                continue;
            }
            if !is_valid_closure_position(raw, idx) {
                continue;
            }
            return true;
        }
    }
    for needle in closure_bare {
        if let Some(idx) = lower.find(needle.as_str()) {
            if idx > 64 {
                continue;
            }
            if !is_valid_closure_position(raw, idx) {
                continue;
            }
            let after = raw.as_bytes().get(idx + needle.len()).copied();
            match after {
                None | Some(b' ' | b'\t' | b'.' | b'!' | b',' | b':') => return true,
                _ => {}
            }
        }
    }
    false
}

#[cfg(test)]
#[allow(dead_code)]
/// Validate that a closure needle at `idx` is in a valid position (not quoted/indented).
fn is_valid_closure_position(raw: &str, idx: usize) -> bool {
    if idx > 0 {
        let prev = raw.as_bytes().get(idx - 1).copied().unwrap_or(b' ');
        if prev != b' ' && prev != b':' && prev != b'(' && prev != b'[' && prev != b'{' {
            return false;
        }
    }
    let prefix = &raw[..idx];
    !(prefix.contains('"') || prefix.contains('\'') || prefix.contains('`') || prefix.contains('>'))
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn discovery_block_window_ms() -> i64 {
    let default_s: i64 = 10 * 60;
    let s = std::env::var("DISCOVERY_BLOCK_SECONDS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 0)
        .unwrap_or(default_s);
    s.saturating_mul(1000)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn is_discovery_block_text(txt: &str) -> bool {
    let lower = txt.to_ascii_lowercase();
    // Scrub "unblocked"/"unblocking" so they don't false-positive on "blocked"/"blocking".
    let scrubbed = lower.replace("unblocked", "").replace("unblocking", "");
    DISCOVERY_BLOCK_KEYWORDS
        .iter()
        .any(|k| scrubbed.contains(k))
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
            "src/foo.rs.bak is a backup",
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
    fn relevant_needles_includes_parents() {
        let needles = relevant_needles_for_path("src/lib/foo.rs");
        assert!(needles.contains(&"src/lib/foo.rs".to_owned()));
        assert!(needles.contains(&"src/lib".to_owned()));
        assert!(needles.contains(&"src".to_owned()));
    }

    #[test]
    fn is_discovery_block_text_positive() {
        assert!(is_discovery_block_text("I'm refactoring src/types.rs"));
        assert!(is_discovery_block_text("please avoid src/format.rs"));
        assert!(is_discovery_block_text(
            "src/types.rs is blocked while I work"
        ));
        assert!(is_discovery_block_text("skip this file for now"));
    }

    #[test]
    fn is_discovery_block_text_unblocked_not_false_positive() {
        assert!(!is_discovery_block_text("src/types.rs is now unblocked"));
        assert!(!is_discovery_block_text("I just unblocked the pipeline"));
    }

    #[test]
    fn is_discovery_block_text_negative() {
        assert!(!is_discovery_block_text("I finished editing src/foo.rs"));
        assert!(!is_discovery_block_text("All good, no issues"));
    }

    #[test]
    fn strip_common_list_prefix_basic() {
        assert_eq!(strip_common_list_prefix("- item"), "item");
        assert_eq!(strip_common_list_prefix("* item"), "item");
        assert_eq!(strip_common_list_prefix("1. item"), "item");
        assert_eq!(strip_common_list_prefix("plain text"), "plain text");
    }

    #[test]
    fn strip_common_list_prefix_task_list() {
        assert_eq!(strip_common_list_prefix("- [x] done"), "done");
        assert_eq!(strip_common_list_prefix("- [ ] todo"), "todo");
    }

    #[test]
    fn parse_body_text_field() {
        let (_, text) = parse_body(r#"{"text":"hello world"}"#);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn parse_body_raw_string() {
        // A bare JSON string gets wrapped as {"raw": "..."}, so extract_message_text
        // falls through to the raw JSON representation.
        let (obj, _text) = parse_body(r#""just a string""#);
        assert_eq!(obj.get("raw").unwrap().as_str().unwrap(), "just a string");
    }

    #[test]
    fn parse_body_malformed_json() {
        let (_, text) = parse_body("not json at all");
        assert_eq!(text, "not json at all");
    }
}
