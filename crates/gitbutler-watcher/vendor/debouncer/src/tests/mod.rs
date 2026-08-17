// Note that this file contains substantial portions of code
// from https://github.com/notify-rs/notify/blob/main/notify-debouncer-full/src/testing.rs,
// and what follows is a reproduction of its license.
//
// Copyright (c) 2023 Notify Contributors
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
use std::{path::Path, time::Duration};

use mock_instant::thread_local::{Instant, MockClock};
use pretty_assertions::assert_eq;
use rstest::rstest;
pub(crate) use schema::TestCase;

#[rstest]
fn state(
    #[values(
        "add_create_event",
        "add_create_event_after_remove_event",
        "add_create_dir_event_twice",
        "add_modify_content_event_after_create_event",
        "add_rename_from_event",
        "add_rename_from_event_after_create_event",
        "add_rename_from_event_after_modify_event",
        "add_rename_from_event_after_create_and_modify_event",
        "add_rename_from_event_after_rename_from_event",
        "add_rename_to_event",
        "add_rename_to_dir_event",
        "add_rename_from_and_to_event",
        "add_rename_from_and_to_event_after_create",
        "add_rename_from_and_to_event_after_rename",
        "add_rename_from_and_to_event_after_modify_content",
        "add_rename_from_and_to_event_override_created",
        "add_rename_from_and_to_event_override_modified",
        "add_rename_from_and_to_event_override_removed",
        "add_rename_from_and_to_event_with_file_ids",
        "add_rename_from_and_to_event_with_different_file_ids",
        "add_rename_from_and_to_event_with_different_tracker",
        "add_rename_both_event",
        "add_remove_event",
        "add_remove_event_after_create_event",
        "add_remove_event_after_modify_event",
        "add_remove_event_after_create_and_modify_event",
        "add_remove_parent_event_after_remove_child_event",
        "add_errors",
        "emit_continuous_modify_content_events",
        "emit_events_in_chronological_order",
        "emit_events_with_a_prepended_rename_event",
        "emit_close_events_only_once",
        "emit_modify_event_after_close_event",
        "emit_needs_rescan_event",
        "read_file_id_without_create_event"
    )]
    file_name: &str,
) {
    let file_content =
        std::fs::read_to_string(Path::new(&format!("tests/fixtures/{file_name}.hjson"))).unwrap();
    let mut test_case = deser_hjson::from_str::<TestCase>(&file_content).unwrap();

    MockClock::set_time(Duration::default());

    let time = Instant::now();

    let mut state = test_case.state.into_debounce_data_inner(time);

    for event in test_case.events {
        let event = event.into_debounced_event(time, None);
        MockClock::set_time(event.time - time);
        state.add_event(event.event);
    }

    for error in test_case.errors {
        state.add_error(error.into_notify_error());
    }

    let expected_errors = std::mem::take(&mut test_case.expected.errors);
    let expected_events = std::mem::take(&mut test_case.expected.events);
    let expected_state = test_case.expected.into_debounce_data_inner(time);
    assert_eq!(
        state.queues, expected_state.queues,
        "queues not as expected"
    );
    assert_eq!(
        state.rename_event, expected_state.rename_event,
        "rename event not as expected"
    );
    assert_eq!(
        state.rescan_event, expected_state.rescan_event,
        "rescan event not as expected"
    );
    assert_eq!(
        state.cache.paths, expected_state.cache.paths,
        "cache not as expected"
    );

    assert_eq!(
        state
            .errors
            .iter()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>(),
        expected_errors
            .iter()
            .map(|e| format!("{:?}", e.clone().into_notify_error()))
            .collect::<Vec<_>>(),
        "errors not as expected"
    );

    let backup_time = Instant::now().duration_since(time);
    let backup_queues = state.queues.clone();

    for (delay, events) in expected_events {
        MockClock::set_time(backup_time);
        state.queues.clone_from(&backup_queues);

        match delay.as_str() {
            "none" => {}
            "short" => MockClock::advance(Duration::from_millis(10)),
            "long" => MockClock::advance(Duration::from_millis(100)),
            _ => {
                if let Ok(ts) = delay.parse::<u64>() {
                    let ts = time + Duration::from_millis(ts);
                    MockClock::set_time(ts - time);
                }
            }
        }

        let events = events
            .into_iter()
            .map(|event| event.into_debounced_event(time, None))
            .collect::<Vec<_>>();

        assert_eq!(
            state.debounced_events(false),
            events,
            "debounced events after a `{delay}` delay"
        );
    }
}

mod schema;
mod utils {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };

    use file_id::FileId;

    use crate::FileIdCache;

    #[derive(Debug, Clone)]
    pub struct TestCache {
        pub paths: HashMap<PathBuf, FileId>,
        pub file_system: HashMap<PathBuf, FileId>,
    }

    impl TestCache {
        pub fn new(paths: HashMap<PathBuf, FileId>, file_system: HashMap<PathBuf, FileId>) -> Self {
            Self { paths, file_system }
        }
    }

    impl FileIdCache for TestCache {
        fn cached_file_id(&self, path: &Path) -> Option<&FileId> {
            self.paths.get(path)
        }

        fn add_path(&mut self, path: &Path) {
            for (p, file_id) in &self.file_system {
                if p.starts_with(path) {
                    self.paths.insert(p.clone(), *file_id);
                }
            }
        }

        fn remove_path(&mut self, path: &Path) {
            self.paths.remove(path);
        }

        fn rescan(&mut self) {
            self.add_path(Path::new("/"));
        }
    }
}
