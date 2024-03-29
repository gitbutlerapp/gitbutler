use gitbutler::deltas::operations::{get_delta_operations, Operation};

#[test]
fn get_delta_operations_insert_end() {
    let initial_text = "hello";
    let final_text = "hello world!";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Insert((5, " world!".to_string())));
}

#[test]
fn get_delta_operations_insert_middle() {
    let initial_text = "helloworld";
    let final_text = "hello, world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Insert((5, ", ".to_string())));
}

#[test]
fn get_delta_operations_insert_begin() {
    let initial_text = "world";
    let final_text = "hello world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Insert((0, "hello ".to_string())));
}

#[test]
fn get_delta_operations_delete_end() {
    let initial_text = "hello world!";
    let final_text = "hello";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Delete((5, 7)));
}

#[test]
fn get_delta_operations_delete_middle() {
    let initial_text = "hello, world";
    let final_text = "helloworld";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Delete((5, 2)));
}

#[test]
fn get_delta_operations_delete_begin() {
    let initial_text = "hello world";
    let final_text = "world";
    let operations = get_delta_operations(initial_text, final_text);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0], Operation::Delete((0, 6)));
}
