use std::collections::VecDeque;

use crate::utils::{assert_ignored_tests_have_linear_ticket, make_absolute};

mod command;
mod journey;
pub mod utils;

#[test]
fn ignored_tests_have_linear_tickets() {
    assert_ignored_tests_have_linear_ticket(file!());

    let this_file = make_absolute(file!());
    let mut todo = VecDeque::from([make_absolute(this_file.parent().unwrap())]);
    while let Some(dir) = todo.pop_front() {
        for entry in dir.read_dir().unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                todo.push_back(path);
                continue;
            }

            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }

            assert_ignored_tests_have_linear_ticket(path);
        }
    }
}
