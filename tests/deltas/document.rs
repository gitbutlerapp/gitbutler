use gitbutler::{
    deltas::{operations::Operation, Delta, Document},
    reader,
};

#[test]
fn new() {
    let document = Document::new(
        Some(&reader::Content::UTF8("hello world".to_string())),
        vec![],
    );
    assert!(document.is_ok());
    let document = document.unwrap();
    assert_eq!(document.to_string(), "hello world");
    assert_eq!(document.get_deltas().len(), 0);
}

#[test]
fn update() {
    let document = Document::new(
        Some(&reader::Content::UTF8("hello world".to_string())),
        vec![],
    );
    assert!(document.is_ok());
    let mut document = document.unwrap();
    document
        .update(Some(&reader::Content::UTF8("hello world!".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "hello world!");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((11, "!".to_string()))
    );
}

#[test]
fn empty() {
    let document = Document::new(None, vec![]);
    assert!(document.is_ok());
    let mut document = document.unwrap();
    document
        .update(Some(&reader::Content::UTF8("hello world!".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "hello world!");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((0, "hello world!".to_string()))
    );
}

#[test]
fn from_deltas() {
    let document = Document::new(
        None,
        vec![
            Delta {
                timestamp_ms: 0,
                operations: vec![Operation::Insert((0, "hello".to_string()))],
            },
            Delta {
                timestamp_ms: 1,
                operations: vec![Operation::Insert((5, " world".to_string()))],
            },
            Delta {
                timestamp_ms: 2,
                operations: vec![
                    Operation::Delete((3, 7)),
                    Operation::Insert((4, "!".to_string())),
                ],
            },
        ],
    );
    assert!(document.is_ok());
    let document = document.unwrap();
    assert_eq!(document.to_string(), "held!");
}

#[test]
fn complex_line() {
    let document = Document::new(None, vec![]);
    assert!(document.is_ok());
    let mut document = document.unwrap();

    document
        .update(Some(&reader::Content::UTF8("hello".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "hello");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((0, "hello".to_string()))
    );

    document
        .update(Some(&reader::Content::UTF8("hello world".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "hello world");
    assert_eq!(document.get_deltas().len(), 2);
    assert_eq!(document.get_deltas()[1].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[1].operations[0],
        Operation::Insert((5, " world".to_string()))
    );

    document
        .update(Some(&reader::Content::UTF8("held!".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "held!");
    assert_eq!(document.get_deltas().len(), 3);
    assert_eq!(document.get_deltas()[2].operations.len(), 2);
    assert_eq!(
        document.get_deltas()[2].operations[0],
        Operation::Delete((3, 7))
    );
    assert_eq!(
        document.get_deltas()[2].operations[1],
        Operation::Insert((4, "!".to_string())),
    );
}

#[test]
fn multiline_add() {
    let document = Document::new(None, vec![]);
    assert!(document.is_ok());
    let mut document = document.unwrap();

    document
        .update(Some(&reader::Content::UTF8("first".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "first");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((0, "first".to_string()))
    );

    document
        .update(Some(&reader::Content::UTF8("first\ntwo".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "first\ntwo");
    assert_eq!(document.get_deltas().len(), 2);
    assert_eq!(document.get_deltas()[1].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[1].operations[0],
        Operation::Insert((5, "\ntwo".to_string()))
    );

    document
        .update(Some(&reader::Content::UTF8(
            "first line\nline two".to_string(),
        )))
        .unwrap();
    assert_eq!(document.to_string(), "first line\nline two");
    assert_eq!(document.get_deltas().len(), 3);
    assert_eq!(document.get_deltas()[2].operations.len(), 2);
    assert_eq!(
        document.get_deltas()[2].operations[0],
        Operation::Insert((5, " line".to_string()))
    );
    assert_eq!(
        document.get_deltas()[2].operations[1],
        Operation::Insert((11, "line ".to_string()))
    );
}

#[test]
fn multiline_remove() {
    let document = Document::new(None, vec![]);
    assert!(document.is_ok());
    let mut document = document.unwrap();

    document
        .update(Some(&reader::Content::UTF8(
            "first line\nline two".to_string(),
        )))
        .unwrap();
    assert_eq!(document.to_string(), "first line\nline two");
    assert_eq!(document.get_deltas().len(), 1);
    assert_eq!(document.get_deltas()[0].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[0].operations[0],
        Operation::Insert((0, "first line\nline two".to_string()))
    );

    document
        .update(Some(&reader::Content::UTF8("first\ntwo".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "first\ntwo");
    assert_eq!(document.get_deltas().len(), 2);
    assert_eq!(document.get_deltas()[1].operations.len(), 2);
    assert_eq!(
        document.get_deltas()[1].operations[0],
        Operation::Delete((5, 5))
    );
    assert_eq!(
        document.get_deltas()[1].operations[1],
        Operation::Delete((6, 5))
    );

    document
        .update(Some(&reader::Content::UTF8("first".to_string())))
        .unwrap();
    assert_eq!(document.to_string(), "first");
    assert_eq!(document.get_deltas().len(), 3);
    assert_eq!(document.get_deltas()[2].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[2].operations[0],
        Operation::Delete((5, 4))
    );

    document.update(None).unwrap();
    assert_eq!(document.to_string(), "");
    assert_eq!(document.get_deltas().len(), 4);
    assert_eq!(document.get_deltas()[3].operations.len(), 1);
    assert_eq!(
        document.get_deltas()[3].operations[0],
        Operation::Delete((0, 5))
    );
}

#[test]
fn binary_to_text() {
    let latest = reader::Content::Binary;
    let current = reader::Content::UTF8("test".to_string());
    let mut document = Document::new(Some(&latest), vec![]).unwrap();
    let new_deltas = document.update(Some(&current)).unwrap();
    assert!(new_deltas.is_some());
    assert_eq!(document.to_string(), "test");
}

#[test]
fn binary_to_binary() {
    let latest = reader::Content::Binary;
    let current = reader::Content::Binary;
    let mut document = Document::new(Some(&latest), vec![]).unwrap();
    let new_deltas = document.update(Some(&current)).unwrap();
    assert!(new_deltas.is_some());
    assert_eq!(document.to_string(), "");
}

#[test]
fn text_to_binary() {
    let latest = reader::Content::UTF8("text".to_string());
    let current = reader::Content::Binary;
    let mut document = Document::new(Some(&latest), vec![]).unwrap();
    let new_deltas = document.update(Some(&current)).unwrap();
    assert!(new_deltas.is_some());
    assert_eq!(document.to_string(), "");
}

#[test]
fn unicode() {
    let latest = reader::Content::UTF8("\u{1f31a}".to_string());
    let current = reader::Content::UTF8("\u{1f31d}".to_string());
    let mut document = Document::new(Some(&latest), vec![]).unwrap();
    document.update(Some(&current)).unwrap();
    assert_eq!(document.to_string(), "\u{1f31d}");
}
