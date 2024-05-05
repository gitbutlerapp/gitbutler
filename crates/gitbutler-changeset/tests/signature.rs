use gitbutler_changeset::Signature;

macro_rules! assert_score {
    ($sig:ident, $s:expr, $e:expr) => {
        let score = $sig.score_str($s);
        if (score - $e).abs() >= 0.1 {
            panic!(
                "expected score of {} for string {:?}, got {}",
                $e, $s, score
            );
        }
    };
}

#[test]
fn score_signature() {
    let sig = Signature::from("hello world");

    // NOTE: The scores here are not exact, but are close enough
    // to be useful for testing purposes, hence why some have the same
    // "score" but different strings.
    assert_score!(sig, "hello world", 1.0);
    assert_score!(sig, "hello world!", 0.95);
    assert_score!(sig, "hello world!!", 0.9);
    assert_score!(sig, "hello world!!!", 0.85);
    assert_score!(sig, "hello world!!!!", 0.8);
    assert_score!(sig, "hello world!!!!!", 0.75);
    assert_score!(sig, "hello world!!!!!!", 0.7);
    assert_score!(sig, "hello world!!!!!!!", 0.65);
    assert_score!(sig, "hello world!!!!!!!!", 0.62);
    assert_score!(sig, "hello world!!!!!!!!!", 0.6);
    assert_score!(sig, "hello world!!!!!!!!!!", 0.55);
}

#[test]
fn score_ignores_whitespace() {
    let sig = Signature::from("hello world");

    assert_score!(sig, "hello world", 1.0);
    assert_score!(sig, "hello world ", 1.0);
    assert_score!(sig, "hello\nworld ", 1.0);
    assert_score!(sig, "hello\n\tworld ", 1.0);
    assert_score!(sig, "\t\t  hel lo\n\two rld \t\t", 1.0);
}

const TEXT1: &str = include_str!("fixtures/text1.txt");
const TEXT2: &str = include_str!("fixtures/text2.txt");
const TEXT3: &str = include_str!("fixtures/text3.txt");
const CODE1: &str = include_str!("fixtures/code1.txt");
const CODE2: &str = include_str!("fixtures/code2.txt");
const CODE3: &str = include_str!("fixtures/code3.txt");
const CODE4: &str = include_str!("fixtures/code4.txt");
const LARGE1: &str = include_str!("fixtures/large1.txt");
const LARGE2: &str = include_str!("fixtures/large2.txt");

macro_rules! real_test {
    ($a: ident, $b: ident, are_similar) => {
        paste::paste! {
            #[test]
            #[allow(non_snake_case)]
            fn [<$a _ $b _are_similar>]() {
                let a = Signature::from($a);
                let b = Signature::from($b);
                assert!(a.score_str($b) >= 0.95);
                assert!(b.score_str($a) >= 0.95);
            }
        }
    };
    ($a: ident, $b: ident, are_not_similar) => {
        paste::paste! {
            #[test]
            #[allow(non_snake_case)]
            fn [<$a _ $b _are_not_similar>]() {
                let a = Signature::from($a);
                let b = Signature::from($b);
                assert!(a.score_str($b) < 0.95);
                assert!(b.score_str($a) < 0.95);
            }
        }
    };
}

// Only similar pairs:
// - TEXT1, TEXT2
// - CODE1, CODE2
// - LARGE1, LARGE2
real_test!(TEXT1, TEXT2, are_similar);
real_test!(CODE1, CODE2, are_similar);
real_test!(LARGE1, LARGE2, are_similar);

// Check all other combos
real_test!(TEXT1, TEXT3, are_not_similar);
real_test!(TEXT1, CODE1, are_not_similar);
real_test!(TEXT1, CODE2, are_not_similar);
real_test!(TEXT1, CODE3, are_not_similar);
real_test!(TEXT1, CODE4, are_not_similar);
real_test!(TEXT1, LARGE1, are_not_similar);
real_test!(TEXT1, LARGE2, are_not_similar);
real_test!(TEXT2, TEXT3, are_not_similar);
real_test!(TEXT2, CODE1, are_not_similar);
real_test!(TEXT2, CODE2, are_not_similar);
real_test!(TEXT2, CODE3, are_not_similar);
real_test!(TEXT2, CODE4, are_not_similar);
real_test!(TEXT2, LARGE1, are_not_similar);
real_test!(TEXT2, LARGE2, are_not_similar);
real_test!(TEXT3, CODE1, are_not_similar);
real_test!(TEXT3, CODE2, are_not_similar);
real_test!(TEXT3, CODE3, are_not_similar);
real_test!(TEXT3, CODE4, are_not_similar);
real_test!(TEXT3, LARGE1, are_not_similar);
real_test!(TEXT3, LARGE2, are_not_similar);
real_test!(CODE1, CODE3, are_not_similar);
real_test!(CODE1, CODE4, are_not_similar);
real_test!(CODE1, LARGE1, are_not_similar);
real_test!(CODE1, LARGE2, are_not_similar);
real_test!(CODE2, CODE3, are_not_similar);
real_test!(CODE2, CODE4, are_not_similar);
real_test!(CODE2, LARGE1, are_not_similar);
real_test!(CODE2, LARGE2, are_not_similar);
real_test!(CODE3, CODE4, are_not_similar);
real_test!(CODE3, LARGE1, are_not_similar);
real_test!(CODE3, LARGE2, are_not_similar);
real_test!(CODE4, LARGE1, are_not_similar);
real_test!(CODE4, LARGE2, are_not_similar);
