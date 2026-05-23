use serde_json::Value;

const REDACTION: &str = "[REDACTED:entropy]";
const MIN_CANDIDATE_LEN: usize = 20;
const ENTROPY_THRESHOLD: f64 = 4.0;
const HEX_ENTROPY_THRESHOLD: f64 = 3.0;

pub(crate) fn redact_value(mut value: Value) -> Value {
    redact_value_in_place(&mut value, false);
    value
}

pub(crate) fn redact_text(value: &str) -> String {
    redact_string(value).unwrap_or_else(|| value.to_owned())
}

fn redact_value_in_place(value: &mut Value, sensitive_key: bool) {
    match value {
        Value::String(text) => {
            if sensitive_key && !text.is_empty() {
                *text = REDACTION.to_string();
            } else if let Some(redacted_text) = redact_string(text) {
                *text = redacted_text;
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_value_in_place(value, sensitive_key);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                redact_value_in_place(value, sensitive_key || is_sensitive_key(key));
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    let normalized = lower.replace(['-', '_'], "");
    matches!(
        normalized.as_str(),
        "accesskey"
            | "auth"
            | "branch"
            | "cookie"
            | "credential"
            | "credentials"
            | "cwd"
            | "path"
            | "setcookie"
            | "uuid"
            | "worktree"
    ) || normalized.ends_with("apikey")
        || normalized.ends_with("authorization")
        || normalized.ends_with("credential")
        || normalized.ends_with("credentials")
        || normalized.ends_with("password")
        || normalized.ends_with("passphrase")
        || normalized.ends_with("privatekey")
        || normalized == "id"
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("path")
        || lower.ends_with("_id")
        || lower.ends_with("-id")
        || key.ends_with("Id")
        || lower.ends_with("_uuid")
        || lower.ends_with("-uuid")
        || key.ends_with("Uuid")
        || lower.ends_with("_branch")
        || lower.ends_with("-branch")
        || key.ends_with("Branch")
}

fn redact_string(text: &str) -> Option<String> {
    let redacted = redact_named_secret_values(text).unwrap_or_else(|| text.to_owned());
    let named_secret_changed = redacted != text;
    let text = redacted.as_str();
    let bytes = text.as_bytes();
    let mut entropy_redacted = String::new();
    let mut entropy_changed = false;
    let mut last_written = 0;
    let mut index = 0;

    while index < bytes.len() {
        if !is_candidate_byte(bytes[index]) {
            index += 1;
            continue;
        }

        let start = index;
        while index < bytes.len() && is_candidate_byte(bytes[index]) {
            index += 1;
        }

        if index - start < MIN_CANDIDATE_LEN {
            continue;
        }

        let candidate = &text[start..index];
        if should_redact(candidate) {
            entropy_redacted.push_str(&text[last_written..start]);
            entropy_redacted.push_str(REDACTION);
            last_written = index;
            entropy_changed = true;
        }
    }

    if !entropy_changed {
        return named_secret_changed.then_some(redacted);
    }

    entropy_redacted.push_str(&text[last_written..]);
    Some(entropy_redacted)
}

fn redact_named_secret_values(text: &str) -> Option<String> {
    const NAMES: &[&str] = &[
        "access_key",
        "apikey",
        "api_key",
        "auth",
        "authorization",
        "cookie",
        "credential",
        "credentials",
        "password",
        "secret",
        "set-cookie",
        "token",
    ];

    let mut redacted = text.to_owned();
    let mut lower = redacted.to_ascii_lowercase();
    let mut changed = false;
    for name in NAMES {
        let mut search_start = 0;
        while let Some(relative_start) = lower[search_start..].find(name) {
            let start = search_start + relative_start;
            if !has_name_boundary(&lower, start, name.len()) {
                search_start = start + name.len();
                continue;
            }
            let mut separator = start + name.len();
            if matches!(lower.as_bytes().get(separator), Some(b'"' | b'\'')) {
                separator += 1;
            }
            while lower
                .as_bytes()
                .get(separator)
                .is_some_and(u8::is_ascii_whitespace)
            {
                separator += 1;
            }
            let Some(&separator_byte) = lower.as_bytes().get(separator) else {
                break;
            };
            if !matches!(separator_byte, b':' | b'=') {
                search_start = separator;
                continue;
            }

            let mut value_start = separator + 1;
            while lower
                .as_bytes()
                .get(value_start)
                .is_some_and(u8::is_ascii_whitespace)
            {
                value_start += 1;
            }
            let quote = lower
                .as_bytes()
                .get(value_start)
                .copied()
                .filter(|byte| matches!(byte, b'"' | b'\''));
            if quote.is_some() {
                value_start += 1;
            }
            let value_end = quote
                .and_then(|quote| quoted_value_end(&lower, value_start, quote))
                .unwrap_or_else(|| secret_value_end(&lower, value_start, separator_byte, name));
            if value_start == value_end {
                search_start = value_start;
                continue;
            }

            redacted.replace_range(value_start..value_end, REDACTION);
            // REDACTION is ASCII, so `lower` stays byte-aligned with `redacted`
            // without rebuilding the whole lowercase copy on every match.
            lower.replace_range(value_start..value_end, REDACTION);
            search_start = value_start + REDACTION.len();
            changed = true;
        }
    }

    changed.then_some(redacted)
}

fn has_name_boundary(text: &str, start: usize, len: usize) -> bool {
    let bytes = text.as_bytes();
    let left = start
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .is_none_or(|byte| !is_name_byte(*byte));
    let right = bytes
        .get(start + len)
        .is_none_or(|byte| !is_name_byte(*byte) || matches!(byte, b'"' | b'\''));
    left && right
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn quoted_value_end(text: &str, start: usize, quote: u8) -> Option<usize> {
    text.as_bytes()[start..]
        .iter()
        .position(|byte| *byte == quote)
        .map(|offset| start + offset)
}

fn secret_value_end(text: &str, start: usize, separator: u8, name: &str) -> usize {
    let bytes = text.as_bytes();
    let mut end = start;
    let header_value =
        separator == b':' && matches!(name, "authorization" | "cookie" | "set-cookie");
    while end < bytes.len() {
        let byte = bytes[end];
        let value_ended = matches!(byte, b'\n' | b'\r' | b',' | b';')
            || !header_value && byte.is_ascii_whitespace();
        if value_ended {
            break;
        }
        end += 1;
    }
    end
}

fn should_redact(candidate: &str) -> bool {
    if contains_uuid(candidate) {
        return true;
    }

    let entropy = shannon_entropy(candidate);
    is_random_hex(candidate, entropy) || entropy > ENTROPY_THRESHOLD
}

fn is_candidate_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'+' | b'/' | b'='
    )
}

fn is_random_hex(candidate: &str, entropy: f64) -> bool {
    candidate.len() >= 32
        && candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
        && entropy > HEX_ENTROPY_THRESHOLD
}

fn contains_uuid(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() < 36 || !bytes.contains(&b'-') {
        return false;
    }

    bytes.windows(36).any(is_uuid_bytes)
}

fn is_uuid_bytes(bytes: &[u8]) -> bool {
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| {
        matches!(index, 8 | 13 | 18 | 23) && *byte == b'-'
            || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
    })
}

fn shannon_entropy(candidate: &str) -> f64 {
    let bytes = candidate.as_bytes();
    let mut counts = [0usize; 256];
    for byte in bytes {
        counts[*byte as usize] += 1;
    }

    let len = bytes.len() as f64;
    counts
        .into_iter()
        .filter(|count| *count > 0)
        .map(|count| {
            let probability = count as f64 / len;
            -probability * probability.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn redacts_token_shaped_text() {
        for (input, expected) in [
            (
                "token: Nf9K2pLm8QwEr7TyUi4OzXa3Bv6Cn0Md done",
                "token: [REDACTED:entropy] done",
            ),
            ("550e8400-e29b-41d4-a716-446655440000", "[REDACTED:entropy]"),
            (
                "session_id=550e8400-e29b-41d4-a716-446655440000",
                "[REDACTED:entropy]",
            ),
            (
                "TOKEN=0123456789abcdef0123456789abcdef01234567; git commit -m fix",
                "TOKEN=[REDACTED:entropy]; git commit -m fix",
            ),
            (
                "token AbCdEfGhIjKl/MnOpQrSt/UvWxYz12+34= done",
                "token [REDACTED:entropy] done",
            ),
            ("password=hunter2 done", "password=[REDACTED:entropy] done"),
            (
                "Cookie: sid=dev token=abc\nnext",
                "Cookie: [REDACTED:entropy]\nnext",
            ),
            (
                "Authorization: Bearer short-token\nnext",
                "Authorization: [REDACTED:entropy]\nnext",
            ),
            (
                r#"{"api_key": "hunter2", "notoken": "keep"}"#,
                r#"{"api_key": "[REDACTED:entropy]", "notoken": "keep"}"#,
            ),
            (
                "secret\t=\tshort secretary=keep",
                "secret\t=\t[REDACTED:entropy] secretary=keep",
            ),
        ] {
            assert_eq!(redact_text(input), expected);
        }
    }

    #[test]
    fn keeps_obvious_non_secrets() {
        assert_eq!(
            redact_text("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(redact_text("2026-05-07T09-30-00Z"), "2026-05-07T09-30-00Z");
    }

    #[test]
    fn keeps_short_high_entropy_runs_below_candidate_length() {
        // Known limit: secrets shorter than MIN_CANDIDATE_LEN with no adjacent
        // key name are not entropy-redacted.
        assert_eq!(redact_text("k7Qz9Xy2Wp"), "k7Qz9Xy2Wp");
    }

    #[test]
    fn redacts_uuid_embedded_in_surrounding_text() {
        assert_eq!(
            redact_text("trace 550e8400-e29b-41d4-a716-446655440000 ok"),
            "trace [REDACTED:entropy] ok"
        );
    }

    #[test]
    fn redacts_domain_sensitive_keys() {
        let value = json!({
            "branch": "feature/login",
            "cwd": "/home/alice/project",
            "path": "/etc/passwd",
            "keep": "plain value",
        });

        let redacted = redact_value(value);

        assert_eq!(redacted["branch"], "[REDACTED:entropy]");
        assert_eq!(redacted["cwd"], "[REDACTED:entropy]");
        assert_eq!(redacted["path"], "[REDACTED:entropy]");
        assert_eq!(redacted["keep"], "plain value");
    }

    #[test]
    fn redacts_sensitive_key_values_recursively() {
        let value = json!({
            "api_key": "short",
            "openai_api_key": "short",
            "db_password": "short",
            "cookie": "sid=short",
            "credentials": "short",
            "access_key": "short",
            "token": {
                "parts": ["alpha", "beta"],
            },
            "nested": {
                "secret": "Zx8Cv7Bn6Mm5Ll4Kk3Jj2Hh1Gg0Ff9Dd8",
            },
            "checksum": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "note": {
                "value": "short",
            },
        });

        let redacted = redact_value(value);

        assert_eq!(redacted["api_key"], "[REDACTED:entropy]");
        assert_eq!(redacted["openai_api_key"], "[REDACTED:entropy]");
        assert_eq!(redacted["db_password"], "[REDACTED:entropy]");
        assert_eq!(redacted["cookie"], "[REDACTED:entropy]");
        assert_eq!(redacted["credentials"], "[REDACTED:entropy]");
        assert_eq!(redacted["access_key"], "[REDACTED:entropy]");
        assert_eq!(redacted["token"]["parts"][0], "[REDACTED:entropy]");
        assert_eq!(redacted["token"]["parts"][1], "[REDACTED:entropy]");
        assert_eq!(redacted["nested"]["secret"], "[REDACTED:entropy]");
        assert_eq!(redacted["checksum"], "[REDACTED:entropy]");
        assert_eq!(redacted["note"]["value"], "short");
    }
}
