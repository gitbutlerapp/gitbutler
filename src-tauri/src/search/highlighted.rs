use std::{collections::HashSet, ops::Range};

use tantivy::Snippet;

// this is similar to Snippet.to_html, but only extracts the highlighted parts
pub fn get_highlighted(snippet: &Snippet) -> Vec<String> {
    let mut result = HashSet::new();

    for item in collapse_overlapped_ranges(&snippet.highlighted()) {
        result.insert(snippet.fragment()[item.clone()].to_string());
    }

    let mut vec = result.into_iter().collect::<Vec<String>>();
    vec.sort();
    vec
}

// copied from tantivy::Snippet
fn collapse_overlapped_ranges(ranges: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut ranges_it = ranges.iter();

    let mut current = match ranges_it.next() {
        Some(range) => range.clone(),
        None => return result,
    };

    for range in ranges {
        if current.end > range.start {
            current = current.start..std::cmp::max(current.end, range.end);
        } else {
            result.push(current);
            current = range.clone();
        }
    }

    result.push(current);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_overlapped_ranges() {
        assert_eq!(&collapse_overlapped_ranges(&[0..1, 2..3,]), &[0..1, 2..3]);
        assert_eq!(&collapse_overlapped_ranges(&[0..1, 1..2,]), &[0..1, 1..2]);
        assert_eq!(&collapse_overlapped_ranges(&[0..2, 1..2,]), &[0..2]);
        assert_eq!(&collapse_overlapped_ranges(&[0..2, 1..3,]), &[0..3]);
        assert_eq!(&collapse_overlapped_ranges(&[0..3, 1..2,]), &[0..3]);
    }

    // #[test]
    // fn test_snippet_with_overlapped_highlighted_ranges() {
    //     let text = "abc";

    //     let mut terms = BTreeMap::new();
    //     terms.insert(String::from("ab"), 0.9);
    //     terms.insert(String::from("bc"), 1.0);

    //     let fragments = search_fragments(
    //         &From::from(NgramTokenizer::all_ngrams(2, 2)),
    //         text,
    //         &terms,
    //         3,
    //     );

    //     assert_eq!(fragments.len(), 1);
    //     {
    //         let first = &fragments[0];
    //         assert_eq!(first.score, 1.9);
    //         assert_eq!(first.start_offset, 0);
    //         assert_eq!(first.stop_offset, 3);
    //     }

    //     let snippet = select_best_fragment_combination(&fragments[..], text);
    //     assert_eq!(snippet.fragment, "abc");
    //     assert_eq!(snippet.to_html(), "<b>abc</b>");
    // }
}
