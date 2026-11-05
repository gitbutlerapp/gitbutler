//! What's new in a version against the one read before it, word by word, so whoever read that
//! one reads what changed and nothing else.

use crate::ui::markdown;

/// How many unchanged words between changed ones are tinted with them.
const MOST_WORDS_BRIDGED: usize = 2;

/// Whether `paragraph` is still among `now`, as it was or edited: enough of its words are in one of
/// them, as with [`mark_what_is_new`].
pub(super) fn still_there(paragraph: &str, now: &[String]) -> bool {
    if now.iter().any(|one| one == paragraph) {
        return true;
    }
    let words = word_spans(paragraph);
    now.iter().any(|one| {
        let theirs: Vec<&str> = word_spans(one)
            .into_iter()
            .filter_map(|span| one.get(span))
            .collect();
        let kept = kept_words(paragraph, &words, &theirs);
        kept.iter().filter(|kept| **kept).count() * 5 >= words.len() * 2
    })
}

/// Mark in `blocks` the words that weren't there in the paragraphs `old`: all of a paragraph new
/// to it, or just what's different from the one it's most like. A reader who read the last
/// version reads these and nothing else.
pub(super) fn mark_what_is_new(blocks: &mut [markdown::Block], old: &[String]) {
    for block in blocks {
        if block.text.trim().is_empty() || old.contains(&block.text) {
            continue;
        }
        let words = word_spans(&block.text);
        let closest = (old.iter())
            .map(|paragraph| {
                let theirs: Vec<&str> = word_spans(paragraph)
                    .into_iter()
                    .filter_map(|span| paragraph.get(span))
                    .collect();
                let kept = kept_words(&block.text, &words, &theirs);
                (kept.iter().filter(|kept| **kept).count(), kept)
            })
            .max_by_key(|(same, _)| *same);
        let kept = closest
            .filter(|(same, _)| *same * 3 >= words.len())
            .map(|(_, kept)| kept)
            .unwrap_or_else(|| vec![false; words.len()]);
        // Most of it new, it's a new sentence that happens to share words, and patches of tint
        // over it would read as noise.
        let kept_count = kept.iter().filter(|kept| **kept).count();
        if kept_count * 5 < words.len() * 2 {
            let all = 0..block.text.len();
            block.styles.push((
                all,
                markdown::Style {
                    tint: Some(markdown::Tint::Fresh),
                    ..Default::default()
                },
            ));
            continue;
        }
        // Neighbouring new words go in one run, with the space between them, and so do those a
        // word or two apart, for the same reason.
        let mut run: Option<std::ops::Range<usize>> = None;
        let mut runs = Vec::new();
        let mut kept_since = 0;
        for (span, kept) in words.iter().zip(&kept) {
            match (kept, &mut run) {
                (false, Some(open)) => {
                    open.end = span.end;
                    kept_since = 0;
                }
                (false, None) => run = Some(span.clone()),
                (true, Some(_)) if kept_since < MOST_WORDS_BRIDGED => kept_since += 1,
                (true, _) => {
                    runs.extend(run.take());
                    kept_since = 0;
                }
            }
        }
        runs.extend(run);
        for range in runs {
            block.styles.push((
                range,
                markdown::Style {
                    tint: Some(markdown::Tint::Fresh),
                    ..Default::default()
                },
            ));
        }
    }
}

/// Of `blocks`, those with anything new, each under the heading it comes under, and in a quote
/// with what the quote starts with, as that can say what kind of quote it is.
pub(super) fn what_is_new(blocks: &[markdown::Block]) -> Vec<markdown::Block> {
    let mut heading = None;
    let mut new = Vec::new();
    for (ix, block) in blocks.iter().enumerate() {
        if matches!(block.kind, markdown::Kind::Heading(_)) {
            heading = Some(block);
        }
        if block
            .styles
            .iter()
            .any(|(_, style)| style.tint == Some(markdown::Tint::Fresh))
        {
            new.extend(heading.take().filter(|heading| *heading != block).cloned());
            let before = ix.checked_sub(1).and_then(|before| blocks.get(before));
            new.extend(
                before
                    .filter(|before| block.quoted && before.quoted && !new.contains(*before))
                    .cloned(),
            );
            new.push(block.clone());
        }
    }
    new
}

/// Where the words of `text` are.
fn word_spans(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(from)) => {
                spans.push(from..at);
                start = None;
            }
            (false, None) => start = Some(at),
            _ => {}
        }
    }
    spans.extend(start.map(|from| from..text.len()));
    spans
}

/// How many pairs of words two paragraphs can have and still be compared, as the table for it
/// takes that much memory. A fenced block can be thousands of words, and past this it reads as new.
const MOST_WORDS_COMPARED: usize = 1_000_000;

/// For each of `words` of `text`, whether it's kept from `theirs`: on a longest common run of
/// words.
fn kept_words(text: &str, words: &[std::ops::Range<usize>], theirs: &[&str]) -> Vec<bool> {
    let ours: Vec<&str> = (words.iter())
        .filter_map(|span| text.get(span.clone()))
        .collect();
    let (n, m) = (ours.len(), theirs.len());
    if n.saturating_mul(m) > MOST_WORDS_COMPARED {
        return vec![false; n];
    }
    let at = |i: usize, j: usize| i * (m + 1) + j;
    let length = |lengths: &[u32], i: usize, j: usize| lengths.get(at(i, j)).copied().unwrap_or(0);
    let mut lengths = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let longest = match ours.get(i) == theirs.get(j) {
                true => length(&lengths, i + 1, j + 1) + 1,
                false => length(&lengths, i + 1, j).max(length(&lengths, i, j + 1)),
            };
            if let Some(cell) = lengths.get_mut(at(i, j)) {
                *cell = longest;
            }
        }
    }
    let mut kept = vec![false; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if ours.get(i) == theirs.get(j) {
            if let Some(word) = kept.get_mut(i) {
                *word = true;
            }
            (i, j) = (i + 1, j + 1);
        } else if length(&lengths, i + 1, j) >= length(&lengths, i, j + 1) {
            i += 1;
        } else {
            j += 1;
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_paragraph_is_still_there_and_a_gone_one_is_not() {
        let now = ["The dialog states the policy in about these words".to_owned()];
        assert!(still_there("The dialog states the policy plainly", &now));
        assert!(!still_there("Readers see a tombstone", &now));
    }

    fn tinted(earlier: &str, now: &str) -> Vec<String> {
        let old = vec![earlier.to_owned()];
        let mut blocks = markdown::parse(now);
        mark_what_is_new(&mut blocks, &old);
        (blocks.iter())
            .flat_map(|block| {
                (block.styles.iter())
                    .filter(|(_, style)| style.tint == Some(markdown::Tint::Fresh))
                    .map(|(range, _)| block.text[range.clone()].to_owned())
            })
            .collect()
    }

    #[test]
    fn only_the_words_that_changed_are_marked() {
        assert_eq!(
            tinted(
                "Screenshots stay stored for thirty days and then they go away",
                "Screenshots stay stored for ninety days and then they go away"
            ),
            ["ninety"],
        );
        assert!(
            tinted("Same words here.", "Same words here.").is_empty(),
            "a paragraph as it was is left alone"
        );
    }

    #[test]
    fn a_paragraph_mostly_new_is_marked_whole() {
        assert_eq!(
            tinted(
                "The dialog says one thing",
                "A wholly different sentence about the dialog now"
            ),
            ["A wholly different sentence about the dialog now"],
            "patches over a new sentence would read as noise"
        );
    }

    #[test]
    fn changed_words_close_together_are_marked_as_one() {
        assert_eq!(
            tinted(
                "one two three four five six seven eight nine ten",
                "one two three four FIVE six SEVEN eight nine ten"
            ),
            ["FIVE six SEVEN"],
        );
    }
}
