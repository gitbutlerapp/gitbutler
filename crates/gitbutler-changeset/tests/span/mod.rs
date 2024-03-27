use gitbutler_changeset::LineSpan;

#[test]
fn new() {
    for s in 0..20 {
        for e in s + 1..=20 {
            let span = LineSpan::new(s, e);
            assert_eq!(span.start(), s);
            assert_eq!(span.end(), e);
        }
    }
}

#[test]
fn extract() {
    let lines = [
        "Hello, world!",
        "This is a test.",
        "This is another test.\r",
        "This is a third test.\r",
        "This is a fourth test.",
        "This is a fifth test.\r",
        "This is a sixth test.",
        "This is a seventh test.\r",
        "This is an eighth test.",
        "This is a ninth test.\r",
        "This is a tenth test.", // note no newline at end
    ];

    let full_text = lines.join("\n");

    // calculate the known character offsets of each line
    let mut offsets = vec![];
    let mut start = 0;
    for (i, line) in lines.iter().enumerate() {
        // If it's not the last line, add 1 for the newline character.
        let end = start + line.len() + (i != (lines.len() - 1)) as usize;
        offsets.push((start, end));
        start = end;
    }

    // Test single-line extraction
    for i in 0..lines.len() - 1 {
        let span = LineSpan::new(i, i + 1);
        let expected = &full_text[offsets[i].0..offsets[i].1];
        let (extracted, start_offset, end_offset) = span.extract(&full_text).unwrap();

        assert_eq!(extracted, expected);
        assert_eq!((start_offset, end_offset), (offsets[i].0, offsets[i].1));
    }

    // Test multi-line extraction
    for i in 0..lines.len() {
        for j in i..=lines.len() {
            let span = LineSpan::new(i, j);

            assert!(span.line_count() == (j - i));

            if i == j {
                assert!(span.is_empty());
                continue;
            }

            let expected_start = offsets[i].0;
            let expected_end = offsets[j - 1].1;
            let expected_text = &full_text[expected_start..expected_end];

            let (extracted, start_offset, end_offset) = span.extract(&full_text).unwrap();
            assert_eq!(extracted, expected_text);
            assert_eq!((start_offset, end_offset), (expected_start, expected_end));
        }
    }
}

#[test]
fn intersects() {
    let span = LineSpan::new(5, 11); // Exclusive end

    assert!(span.intersects(&LineSpan::new(10, 11))); // Intersect at start
    assert!(span.intersects(&LineSpan::new(0, 11))); // Fully contained
    assert!(span.intersects(&LineSpan::new(10, 15))); // Partial overlap
    assert!(span.intersects(&LineSpan::new(4, 6))); // Intersect at end
    assert!(span.intersects(&LineSpan::new(5, 6))); // Exact match start
    assert!(span.intersects(&LineSpan::new(0, 6))); // Overlap at end
    assert!(span.intersects(&LineSpan::new(0, 8))); // Overlap middle
    assert!(span.intersects(&LineSpan::new(0, 10))); // Overlap up to end
    assert!(span.intersects(&LineSpan::new(9, 10))); // Overlap at single point
    assert!(span.intersects(&LineSpan::new(7, 9))); // Overlap inside

    // Test cases where there should be no intersection due to exclusive end
    assert!(!span.intersects(&LineSpan::new(0, 5))); // Before start
    assert!(!span.intersects(&LineSpan::new(11, 20))); // After end
    assert!(!span.intersects(&LineSpan::new(11, 12))); // Just after end
}
