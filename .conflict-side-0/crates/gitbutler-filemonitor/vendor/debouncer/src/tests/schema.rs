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
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    time::Duration,
};

use file_id::FileId;
use mock_instant::thread_local::Instant;
use notify::{
    ErrorKind, EventKind,
    event::{
        AccessKind, AccessMode, CreateKind, DataChange, Flag, MetadataKind, ModifyKind, RemoveKind,
        RenameMode,
    },
};
use serde::Deserialize;

use super::utils::TestCache;
use crate::{DebounceDataInner, DebouncedEvent};

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

impl Error {
    pub fn into_notify_error(self) -> notify::Error {
        let kind = match &*self.kind {
            "path-not-found" => ErrorKind::PathNotFound,
            "watch-not-found" => ErrorKind::WatchNotFound,
            "max-files-watch" => ErrorKind::MaxFilesWatch,
            _ => panic!("unknown error type `{}`", self.kind),
        };
        let mut error = notify::Error::new(kind);

        for p in self.paths {
            error = error.add_path(PathBuf::from(p));
        }

        error
    }
}

impl Event {
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
        let mut event = notify::Event::new(kind);

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

impl State {
    pub(crate) fn into_debounce_data_inner(self, time: Instant) -> DebounceDataInner<TestCache> {
        let queues = self
            .queues
            .into_iter()
            .map(|(path, queue)| {
                let queue = crate::Queue {
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
            cache,
            rename_event,
            rescan_event,
            errors: Vec::new(),
            timeout: Duration::from_millis(self.timeout.unwrap_or(50)),
        }
    }
}
