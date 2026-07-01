use bstr::{BStr, BString, ByteSlice};

use super::Headers;

/// The prefix prepended to the commit subject line to mark a conflicted commit.
const CONFLICT_MESSAGE_PREFIX: &[u8] = b"[conflict] ";

/// The git trailer token used to identify GitButler-managed conflicted commits.
/// The description explaining the conflict is embedded as the multi-line trailer
/// value, with continuation lines indented by 3 spaces per git convention.
const CONFLICT_TRAILER_TOKEN: &[u8] = b"GitButler-Conflict";

/// The full multi-line git trailer appended to conflicted commit messages.
/// The description is the trailer value; continuation lines are indented with
/// 3 spaces so standard git trailer tools can parse and manipulate them.
const CONFLICT_TRAILER: &str = concat!(
    "GitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved\n",
    "   using the \"ours\" side. The commit tree contains additional directories:\n",
    "     .conflict-side-0  \u{2014} our tree\n",
    "     .conflict-side-1  \u{2014} their tree\n",
    "     .conflict-base-0  \u{2014} the merge base tree\n",
    "     .auto-resolution  \u{2014} the auto-resolved tree\n",
    "     .conflict-files   \u{2014} metadata about conflicted files\n",
    "   To manually resolve, check out this commit, remove the directories\n",
    "   listed above, resolve the conflicts, and amend the commit."
);

/// Add conflict markers to a commit `message`: prepend `[conflict] ` to the
/// subject and append the `GitButler-Conflict` multi-line trailer after any
/// existing trailers, and return the new message.
/// The `message` is returned unchanged if it already contained conflict markers.
///
/// A single trailing newline is trimmed from the input before markers are
/// added; callers that need byte-exact round-trips should account for this.
pub fn add_conflict_markers(message: &BStr) -> BString {
    if message_is_conflicted(message) {
        return message.into();
    }
    let trimmed = strip_single_trailing_newline(message.as_bytes());
    let clean = trimmed
        .strip_prefix(CONFLICT_MESSAGE_PREFIX)
        .unwrap_or(trimmed);
    let line_ending = line_ending_for(message.as_bytes());
    let has_trailer_block = gix::objs::commit::MessageRef::from_bytes(clean)
        .body()
        .is_some_and(|body| body.trailers().next().is_some());

    let mut result = BString::default();
    result.extend_from_slice(CONFLICT_MESSAGE_PREFIX);
    result.extend_from_slice(clean);

    // Keep trailers in the last paragraph if the message already has a trailer block.
    result.extend_from_slice(line_ending);
    if !has_trailer_block {
        result.extend_from_slice(line_ending);
    }
    push_with_line_endings(&mut result, CONFLICT_TRAILER.as_bytes(), line_ending);
    result.extend_from_slice(line_ending);
    result
}

/// Strip conflict markers from a commit message.
/// Returns the message unchanged when it is not conflicted (per
/// [`message_is_conflicted`]).
///
/// Strips the `[conflict] ` subject prefix (if present) and the
/// `GitButler-Conflict` trailer line together with all its indented
/// continuation lines.
///
/// Note: the returned message may not be byte-identical to the original —
/// trailing newlines are not preserved.
pub fn strip_conflict_markers(message: &BStr) -> BString {
    let msg_bytes = message.as_bytes();
    let line_ending = line_ending_for(msg_bytes);

    // Strip the subject prefix if present.
    let without_prefix = msg_bytes
        .strip_prefix(CONFLICT_MESSAGE_PREFIX)
        .unwrap_or(msg_bytes);
    let message = gix::objs::commit::MessageRef::from_bytes(without_prefix);
    let trailer_bytes = conflict_trailer_bytes(line_ending);
    let trailer_start = message.body.and_then(|body| {
        let body_offset = subslice_offset(without_prefix, body.as_bytes());
        gix::objs::commit::message::BodyRef::from_bytes(body.as_bytes())
            .trailers()
            .find(|trailer| trailer.token.eq_ignore_ascii_case(CONFLICT_TRAILER_TOKEN))
            .map(|trailer| body_offset + subslice_offset(body.as_bytes(), trailer.token.as_bytes()))
    });

    if let Some(trailer_start) = trailer_start {
        if without_prefix[trailer_start..].starts_with(trailer_bytes.as_slice()) {
            let before = &without_prefix[..trailer_start];
            let after = &without_prefix[trailer_start + trailer_bytes.len()..];
            let mut out = BString::from(before);
            out.extend_from_slice(after);
            return trim_trailing_line_endings(out.as_ref()).into();
        }

        return trim_trailing_line_endings(&without_prefix[..trailer_start]).into();
    }

    without_prefix.find(trailer_bytes.as_slice()).map_or_else(
        || msg_bytes.into(),
        |trailer_start| {
            let before = trim_trailing_line_endings(&without_prefix[..trailer_start]);
            let after = &without_prefix[trailer_start + trailer_bytes.len()..];
            let mut out = BString::from(before);
            out.extend_from_slice(after);
            out
        },
    )
}

/// Returns `true` when the commit is conflicted either by message marker
/// (current encoding) or by the legacy `gitbutler-conflicted` header.
pub fn is_conflicted(message: &BStr, headers: Option<&Headers>) -> bool {
    message_is_conflicted(message) || headers.is_some_and(Headers::is_conflicted)
}

/// Returns `true` when the commit message contains a `GitButler-Conflict:`
/// trailer. The `[conflict] ` subject prefix is informational and
/// not required for detection.
///
/// Trailing blank lines are skipped so that messages edited by users or tools
/// that append newlines are still detected correctly.
pub fn message_is_conflicted(message: &BStr) -> bool {
    let message = gix::objs::commit::MessageRef::from_bytes(message.as_bytes());
    let Some(body) = message.body() else {
        return false;
    };
    body.trailers()
        .any(|trailer| trailer.token.eq_ignore_ascii_case(CONFLICT_TRAILER_TOKEN))
}

/// If `old_message` is conflicted but `new_message` is not, re-apply the
/// conflict markers to `new_message`. This is used during reword and squash
/// so that editing a conflicted commit's message doesn't silently drop the
/// conflict state.
///
/// Strips any existing partial markers from `new_message` before re-adding
/// to avoid double-prefixing or duplicate trailers.
pub fn rewrite_conflict_markers_on_message_change(
    old_message: &BStr,
    new_message: BString,
) -> BString {
    if message_is_conflicted(old_message) && !message_is_conflicted(new_message.as_ref()) {
        add_conflict_markers(new_message.as_ref())
    } else {
        new_message
    }
}

fn line_ending_for(message: &[u8]) -> &'static [u8] {
    if message.find(b"\r\n").is_some() {
        b"\r\n"
    } else {
        b"\n"
    }
}

fn strip_single_trailing_newline(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_suffix(b"\r\n")
        .or_else(|| bytes.strip_suffix(b"\n"))
        .unwrap_or(bytes)
}

fn subslice_offset(base: &[u8], subslice: &[u8]) -> usize {
    subslice.as_ptr() as usize - base.as_ptr() as usize
}

fn trim_trailing_line_endings(mut bytes: &[u8]) -> &[u8] {
    while let Some(stripped) = bytes
        .strip_suffix(b"\r\n")
        .or_else(|| bytes.strip_suffix(b"\n"))
        .or_else(|| bytes.strip_suffix(b"\r"))
    {
        bytes = stripped;
    }
    bytes
}

fn conflict_trailer_bytes(line_ending: &[u8]) -> BString {
    let mut out = BString::default();
    push_with_line_endings(&mut out, CONFLICT_TRAILER.as_bytes(), line_ending);
    out.extend_from_slice(line_ending);
    out
}

fn push_with_line_endings(out: &mut BString, text: &[u8], line_ending: &[u8]) {
    let mut start = 0;
    for (idx, byte) in text.iter().enumerate() {
        if *byte == b'\n' {
            out.extend_from_slice(&text[start..idx]);
            out.extend_from_slice(line_ending);
            start = idx + 1;
        }
    }
    out.extend_from_slice(&text[start..]);
}

#[cfg(test)]
mod tests {
    use crate::commit::{
        Headers, add_conflict_markers, is_conflicted, message_is_conflicted,
        rewrite_conflict_markers_on_message_change, strip_conflict_markers,
    };
    use bstr::{BStr, BString, ByteSlice};

    use super::CONFLICT_TRAILER;

    const CONFLICT_MESSAGE_PREFIX: &str = "[conflict] ";
    const CONFLICT_TRAILER_TOKEN: &str = "GitButler-Conflict";

    fn marked(msg: &str) -> String {
        String::from_utf8(add_conflict_markers(BStr::new(msg)).into()).unwrap()
    }

    fn stripped(msg: &str) -> String {
        String::from_utf8(strip_conflict_markers(BStr::new(msg)).into()).unwrap()
    }

    fn message_is_marked_conflicted(msg: &str) -> bool {
        message_is_conflicted(BStr::new(msg))
    }

    fn assert_only_crlf_line_endings(message: &[u8]) {
        for (idx, byte) in message.iter().enumerate() {
            if *byte == b'\n' {
                assert_eq!(
                    idx.checked_sub(1).and_then(|idx| message.get(idx)),
                    Some(&b'\r'),
                    "found bare LF at byte index {idx} in {message:?}"
                );
            }
        }
    }

    /// Round-trip: add then strip returns the original (modulo the trailing
    /// newline that `add_conflict_markers` always trims).
    #[test]
    fn simple_subject_roundtrip() {
        let original = "fix the bug";
        let result = stripped(&marked(original));
        assert_eq!(result, original);
        assert!(message_is_marked_conflicted(&marked(original)));
    }

    #[test]
    fn trailing_newline_is_trimmed_by_add() {
        // add_conflict_markers trims a trailing newline; strip reflects that.
        assert_eq!(stripped(&marked("fix the bug\n")), "fix the bug");
    }

    #[test]
    fn subject_and_body_roundtrip() {
        let original = "fix the bug\n\nDetailed explanation here.";
        assert_eq!(stripped(&marked(original)), original);
    }

    #[test]
    fn existing_trailers_are_preserved_and_ours_comes_last() {
        let original = "fix the bug\n\nChange-Id: I1234567\nSigned-off-by: User <u@e.com>";
        let result = marked(original);
        assert!(message_is_marked_conflicted(&result));

        // Existing trailers must still be present
        assert!(result.contains("Change-Id: I1234567\n"));
        assert!(result.contains("Signed-off-by: User <u@e.com>\n"));

        // Our trailer must come after the existing ones
        let signed_pos = result.find("Signed-off-by:").unwrap();
        let conflict_pos = result.find(CONFLICT_TRAILER_TOKEN).unwrap();
        assert!(
            conflict_pos > signed_pos,
            "GitButler-Conflict trailer must follow existing trailers"
        );

        // Roundtrip
        assert_eq!(stripped(&result), original);
    }

    #[test]
    fn subject_with_only_trailers_roundtrip() {
        let original = "fix the bug\n\nChange-Id: I1234567";
        assert_eq!(stripped(&marked(original)), original);
    }

    #[test]
    fn body_and_trailers_roundtrip() {
        let original =
            "fix the bug\n\nSome explanation.\n\nChange-Id: I1234567\nSigned-off-by: A <a@b.com>";
        assert_eq!(stripped(&marked(original)), original);
    }

    #[test]
    fn description_is_the_trailer_value_not_a_separate_paragraph() {
        let result = marked("subject");
        // The description must appear on the same line as GitButler-Conflict:
        // (or as indented continuation lines), not as a separate paragraph.
        let trailer_start = format!("{CONFLICT_TRAILER_TOKEN}:");
        let conflict_line = result
            .lines()
            .find(|l| l.starts_with(&trailer_start))
            .expect("trailer line must exist");
        assert!(
            conflict_line.len() > trailer_start.len(),
            "trailer token must have an inline value, got: {conflict_line:?}"
        );
    }

    #[test]
    fn prefix_without_trailer_is_not_conflicted() {
        assert!(!message_is_marked_conflicted(
            "[conflict] looks real but no trailer"
        ));
    }

    #[test]
    fn trailer_without_prefix_is_still_conflicted() {
        let msg = "normal commit\n\nGitButler-Conflict: sneaky";
        // Detection depends only on the trailer, not the prefix.
        assert!(message_is_marked_conflicted(msg));
        // Strip removes the trailer even without the prefix.
        assert_eq!(stripped(msg), "normal commit");
    }

    #[test]
    fn add_is_idempotent() {
        let original = "subject";
        let once = marked(original);
        let twice = marked(&once);

        assert!(message_is_marked_conflicted(&once));
        assert_eq!(twice, once);
        assert_eq!(stripped(&once), stripped(&stripped(&once)));
    }

    #[test]
    fn add_does_not_double_prefix_prefix_only_messages() {
        let partial = format!("{CONFLICT_MESSAGE_PREFIX}subject");
        let result = marked(&partial);

        assert_eq!(result.matches(CONFLICT_MESSAGE_PREFIX).count(), 1);
        assert!(message_is_marked_conflicted(&result));
        assert_eq!(stripped(&result), "subject");
    }

    #[test]
    fn strip_is_idempotent() {
        let original = marked("subject");
        let once = stripped(&original);
        let twice = stripped(&once);

        assert_eq!(twice, once);
    }

    #[test]
    fn trailing_blank_lines_after_trailer_still_detected() {
        let msg = format!("subject\n\n{CONFLICT_TRAILER}\n\n");
        assert!(
            message_is_marked_conflicted(&msg),
            "trailing blank lines must not break detection"
        );
    }

    /// The trailer token appearing in the body (not the last paragraph) must
    /// not be stripped — only the actual trailer in the last paragraph is removed.
    #[test]
    fn strip_only_removes_trailer_from_last_paragraph() {
        let body_with_token = format!(
            "[conflict] subject\n\nGitButler-Conflict: mentioned in body\n\n{CONFLICT_TRAILER}\n"
        );
        assert!(message_is_marked_conflicted(&body_with_token));
        let result = stripped(&body_with_token);
        assert!(
            result.contains("GitButler-Conflict: mentioned in body"),
            "body occurrence must be preserved, got: {result:?}"
        );
        assert!(
            !result.contains("This is a GitButler-managed"),
            "the trailer itself must be removed"
        );
    }

    #[test]
    fn strip_removes_our_trailer_even_with_arbitrary_suffix_after_it() {
        let original = "subject\n\nbody";
        let suffixed = format!(
            "{marked_message}\n\nthis suffix is arbitrary prose, not a trailer",
            marked_message = marked(original)
        );

        assert_eq!(
            stripped(&suffixed),
            "subject\n\nbody\n\nthis suffix is arbitrary prose, not a trailer"
        );
    }

    #[test]
    fn strip_preserves_following_official_trailer_in_same_trailer_block() {
        let original = "subject\n\nbody";
        let suffixed = format!(
            "{marked_message}Signed-off-by: A U Thor <author@example.com>\n",
            marked_message = marked(original)
        );

        assert_eq!(
            stripped(&suffixed),
            "subject\n\nbody\n\nSigned-off-by: A U Thor <author@example.com>"
        );
    }

    #[test]
    fn rewrite_does_not_double_prefix() {
        let original = "fix bug";
        let conflicted = marked(original);
        // Simulate a new message that already has the prefix but no trailer.
        let new_message_with_accidental_prefix = format!("{CONFLICT_MESSAGE_PREFIX}fix bug");
        let result = rewrite_conflict_markers_on_message_change(
            conflicted.as_str().into(),
            new_message_with_accidental_prefix.into(),
        );
        let result_str = std::str::from_utf8(result.as_ref()).unwrap();
        // Must not produce "[conflict] [conflict] fix bug".
        let prefix_count = result_str.matches(CONFLICT_MESSAGE_PREFIX).count();
        assert_eq!(prefix_count, 1, "prefix must appear exactly once");
        assert!(message_is_marked_conflicted(result_str));
    }

    #[test]
    fn detects_conflicts_from_headers_too() {
        assert!(is_conflicted(
            BStr::new("ordinary message"),
            Some(&Headers {
                change_id: None,
                conflicted: Some(1),
            }),
        ));
        assert!(!is_conflicted(
            BStr::new("ordinary message"),
            Some(&Headers::default()),
        ));
        assert!(!is_conflicted(BStr::new("ordinary message"), None));
    }

    /// The `GitButler-Conflict` trailer must always be the last trailer in
    /// the message so it does not interfere with other trailer-based tools.
    #[test]
    fn conflict_trailer_is_last() {
        let original = "fix bug\n\nSome body.\n\nChange-Id: I123\nSigned-off-by: A <a@b.com>";
        let result = marked(original);

        let msg = gix::objs::commit::MessageRef::from_bytes(BStr::new(&result));
        let body = msg.body().expect("message must have a body");
        let trailers: Vec<_> = body.trailers().collect();
        let last = trailers.last().expect("must have at least one trailer");
        assert_eq!(
            last.token.to_str().unwrap(),
            "GitButler-Conflict",
            "conflict trailer must be the last trailer"
        );
    }

    /// Verify gix sees the `[conflict]` prefix in the title even for
    /// subject-only messages.
    #[test]
    fn subject_only_roundtrip_with_gix() {
        let original = "fix the bug";
        let result = marked(original);

        let msg = gix::objs::commit::MessageRef::from_bytes(BStr::new(&result));
        assert_eq!(
            msg.title.to_str().unwrap(),
            "[conflict] fix the bug",
            "gix must see the prefixed title"
        );

        // Round-trip
        assert_eq!(stripped(&result), original);
    }

    #[test]
    fn mutating_functions_preserve_windows_line_endings() {
        let original = "fix the bug\r\n\r\nDetailed explanation here.\r\n\r\nChange-Id: I1234567\r\nSigned-off-by: A <a@b.com>";
        let marked = add_conflict_markers(BStr::new(original));
        assert_only_crlf_line_endings(marked.as_ref());
        assert!(message_is_conflicted(marked.as_ref()));

        let stripped = strip_conflict_markers(marked.as_ref());
        assert_only_crlf_line_endings(stripped.as_ref());
        assert_eq!(stripped.as_bytes(), original.as_bytes());

        let rewritten_message = BString::from(
            "[conflict] rewritten subject\r\n\r\nUpdated body.\r\n\r\nChange-Id: I7654321",
        );
        let rewritten =
            rewrite_conflict_markers_on_message_change(marked.as_ref(), rewritten_message.clone());
        assert_only_crlf_line_endings(rewritten.as_ref());
        assert!(message_is_conflicted(rewritten.as_ref()));
        assert_eq!(
            strip_conflict_markers(rewritten.as_ref()).as_bytes(),
            rewritten_message
                .as_bytes()
                .strip_prefix(CONFLICT_MESSAGE_PREFIX.as_bytes())
                .unwrap()
        );
    }

    #[test]
    fn mutating_functions_preserve_mixed_line_endings() {
        let original = "fix the bug\r\n\r\nDetailed explanation here.\n\nChange-Id: I1234567\r\nSigned-off-by: A <a@b.com>";
        let marked = add_conflict_markers(BStr::new(original));
        assert!(
            marked[CONFLICT_MESSAGE_PREFIX.len()..].starts_with(original.as_bytes()),
            "existing message content must remain byte-for-byte intact"
        );
        assert!(message_is_conflicted(marked.as_ref()));

        let stripped = strip_conflict_markers(marked.as_ref());
        assert_eq!(stripped.as_bytes(), original.as_bytes());

        let rewritten_message = BString::from(
            "[conflict] rewritten subject\n\r\nUpdated body.\r\n\nChange-Id: I7654321",
        );
        let rewritten =
            rewrite_conflict_markers_on_message_change(marked.as_ref(), rewritten_message.clone());
        assert!(message_is_conflicted(rewritten.as_ref()));
        assert_eq!(
            strip_conflict_markers(rewritten.as_ref()).as_bytes(),
            rewritten_message
                .as_bytes()
                .strip_prefix(CONFLICT_MESSAGE_PREFIX.as_bytes())
                .unwrap()
        );
    }
}
