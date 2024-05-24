use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    time::Duration,
};

use file_id::FileId;
use mock_instant::Instant;
use notify::{
    event::{
        AccessKind, AccessMode, CreateKind, DataChange, Flag, MetadataKind, ModifyKind, RemoveKind,
        RenameMode,
    },
    Error, ErrorKind, Event, EventKind, RecursiveMode,
};

use crate::{DebounceDataInner, DebouncedEvent, FileIdCache, Queue};

pub(crate) use schema::TestCase;

mod schema {
    use std::collections::HashMap;

    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(crate) struct Error {
        /// The error kind is parsed by `into_notify_error`
        pub kind: String,

        /// The error paths
        #[serde(default)]
        pub paths: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(crate) struct Event {
        /// The timestamp the event occurred
        #[serde(default)]
        pub time: u64,

        /// The event kind is parsed by `into_notify_event`
        pub kind: String,

        /// The event paths
        #[serde(default)]
        pub paths: Vec<String>,

        /// The event flags
        #[serde(default)]
        pub flags: Vec<String>,

        /// The event tracker
        pub tracker: Option<usize>,

        /// The event info
        pub info: Option<String>,

        /// The file id for the file associated with the event
        ///
        /// Only used for the rename event.
        pub file_id: Option<u64>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(crate) struct Queue {
        pub events: Vec<Event>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(crate) struct State {
        /// Timeout for the debouncer
        ///
        /// Only used for the initial state.
        pub timeout: Option<u64>,

        /// The event queues for each file
        #[serde(default)]
        pub queues: HashMap<String, Queue>,

        /// Cached file ids
        #[serde(default)]
        pub cache: HashMap<String, u64>,

        /// A map of file ids, used instead of accessing the file system
        #[serde(default)]
        pub file_system: HashMap<String, u64>,

        /// Current rename event
        pub rename_event: Option<Event>,

        /// Current rescan event
        pub rescan_event: Option<Event>,

        /// Debounced events
        ///
        /// Only used for the expected state.
        #[serde(default)]
        pub events: HashMap<String, Vec<Event>>,

        /// Errors
        #[serde(default)]
        pub errors: Vec<Error>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(crate) struct TestCase {
        /// Initial state
        pub state: State,

        /// Events that are added during the test
        #[serde(default)]
        pub events: Vec<Event>,

        /// Errors that are added during the test
        #[serde(default)]
        pub errors: Vec<Error>,

        /// Expected state after the test
        pub expected: State,
    }
}

impl schema::Error {
    pub fn into_notify_error(self) -> Error {
        let kind = match &*self.kind {
            "path-not-found" => ErrorKind::PathNotFound,
            "watch-not-found" => ErrorKind::WatchNotFound,
            "max-files-watch" => ErrorKind::MaxFilesWatch,
            _ => panic!("unknown error type `{}`", self.kind),
        };
        let mut error = Error::new(kind);

        for p in self.paths {
            error = error.add_path(PathBuf::from(p));
        }

        error
    }
}

impl schema::Event {
    #[rustfmt::skip]
    pub fn into_debounced_event(self, time: Instant, path: Option<&str>) -> DebouncedEvent {
        let kind = match &*self.kind {
            "any" => EventKind::Any,
            "other" => EventKind::Other,
            "access-any" => EventKind::Access(AccessKind::Any),
            "access-read" => EventKind::Access(AccessKind::Read),
            "access-open-any" => EventKind::Access(AccessKind::Open(AccessMode::Any)),
            "access-open-execute" => EventKind::Access(AccessKind::Open(AccessMode::Execute)),
            "access-open-read" => EventKind::Access(AccessKind::Open(AccessMode::Read)),
            "access-open-write" => EventKind::Access(AccessKind::Open(AccessMode::Write)),
            "access-open-other" => EventKind::Access(AccessKind::Open(AccessMode::Other)),
            "access-close-any" => EventKind::Access(AccessKind::Close(AccessMode::Any)),
            "access-close-execute" => EventKind::Access(AccessKind::Close(AccessMode::Execute)),
            "access-close-read" => EventKind::Access(AccessKind::Close(AccessMode::Read)),
            "access-close-write" => EventKind::Access(AccessKind::Close(AccessMode::Write)),
            "access-close-other" => EventKind::Access(AccessKind::Close(AccessMode::Other)),
            "access-other" => EventKind::Access(AccessKind::Other),
            "create-any" => EventKind::Create(CreateKind::Any),
            "create-file" => EventKind::Create(CreateKind::File),
            "create-folder" => EventKind::Create(CreateKind::Folder),
            "create-other" => EventKind::Create(CreateKind::Other),
            "modify-any" => EventKind::Modify(ModifyKind::Any),
            "modify-other" => EventKind::Modify(ModifyKind::Other),
            "modify-data-any" => EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            "modify-data-size" => EventKind::Modify(ModifyKind::Data(DataChange::Size)),
            "modify-data-content" => EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            "modify-data-other" => EventKind::Modify(ModifyKind::Data(DataChange::Other)),
            "modify-metadata-any" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
            "modify-metadata-access-time" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)),
            "modify-metadata-write-time" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::WriteTime)),
            "modify-metadata-permissions" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)),
            "modify-metadata-ownership" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::Ownership)),
            "modify-metadata-extended" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::Extended)),
            "modify-metadata-other" => EventKind::Modify(ModifyKind::Metadata(MetadataKind::Other)),
            "rename-any" => EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            "rename-from" => EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            "rename-to" => EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            "rename-both" => EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            "rename-other" => EventKind::Modify(ModifyKind::Name(RenameMode::Other)),
            "remove-any" => EventKind::Remove(RemoveKind::Any),
            "remove-file" => EventKind::Remove(RemoveKind::File),
            "remove-folder" => EventKind::Remove(RemoveKind::Folder),
            "remove-other" => EventKind::Remove(RemoveKind::Other),
            _ => panic!("unknown event type `{}`", self.kind),
        };
        let mut event = Event::new(kind);

        for p in self.paths {
            event = event.add_path(if p == "*" {
                PathBuf::from(path.expect("cannot replace `*`"))
            } else {
                PathBuf::from(p)
            });

            if let Some(tracker) = self.tracker {
                event = event.set_tracker(tracker);
            }

            if let Some(info) = &self.info {
                event = event.set_info(info.as_str());
            }
        }

        for f in self.flags {
            let flag = match &*f {
                "rescan" => Flag::Rescan,
                _ => panic!("unknown event flag `{f}`"),
            };

            event = event.set_flag(flag);
        }

        DebouncedEvent { event, time: time + Duration::from_millis(self.time) }
    }
}

impl schema::State {
    pub(crate) fn into_debounce_data_inner(self, time: Instant) -> DebounceDataInner<TestCache> {
        let queues = self
            .queues
            .into_iter()
            .map(|(path, queue)| {
                let queue = Queue {
                    events: queue
                        .events
                        .into_iter()
                        .map(|event| event.into_debounced_event(time, Some(&path)))
                        .collect::<VecDeque<_>>(),
                };
                (path.into(), queue)
            })
            .collect();

        let cache = self
            .cache
            .into_iter()
            .map(|(path, id)| {
                let path = PathBuf::from(path);
                let id = FileId::new_inode(id, id);
                (path, id)
            })
            .collect::<HashMap<_, _>>();

        let file_system = self
            .file_system
            .into_iter()
            .map(|(path, id)| {
                let path = PathBuf::from(path);
                let id = FileId::new_inode(id, id);
                (path, id)
            })
            .collect::<HashMap<_, _>>();

        let cache = TestCache::new(cache, file_system);

        let rename_event = self.rename_event.map(|e| {
            let file_id = e.file_id.map(|id| FileId::new_inode(id, id));
            let event = e.into_debounced_event(time, None);
            (event, file_id)
        });

        let rescan_event = self
            .rescan_event
            .map(|e| e.into_debounced_event(time, None));

        DebounceDataInner {
            queues,
            roots: Vec::new(),
            cache,
            rename_event,
            rescan_event,
            errors: Vec::new(),
            timeout: Duration::from_millis(self.timeout.unwrap_or(50)),
        }
    }
}

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

    fn add_path(&mut self, path: &Path, recursive_mode: RecursiveMode) {
        for (file_path, file_id) in &self.file_system {
            if file_path == path
                || (file_path.starts_with(path) && recursive_mode == RecursiveMode::Recursive)
            {
                self.paths.insert(file_path.clone(), *file_id);
            }
        }
    }

    fn remove_path(&mut self, path: &Path) {
        self.paths.remove(path);
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    use mock_instant::MockClock;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use testing::TestCase;

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
            fs::read_to_string(Path::new(&format!("./test_cases/{file_name}.hjson"))).unwrap();
        let mut test_case = deser_hjson::from_str::<TestCase>(&file_content).unwrap();

        MockClock::set_time(Duration::default());

        let time = Instant::now();

        let mut state = test_case.state.into_debounce_data_inner(time);
        state.roots = vec![(PathBuf::from("/"), RecursiveMode::Recursive)];

        for event in test_case.events {
            let event = event.into_debounced_event(time, None);
            MockClock::set_time(event.time - time);
            state.add_event(event.event);
        }

        for error in test_case.errors {
            let error = error.into_notify_error();
            state.add_error(error);
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
                .map(|e| format!("{:?}", e))
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
            state.queues = backup_queues.clone();

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
                state.debounced_events(),
                events,
                "debounced events after a `{delay}` delay"
            );
        }
    }
}
