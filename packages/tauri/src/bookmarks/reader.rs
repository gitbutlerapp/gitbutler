use std::path;

use anyhow::Result;
use serde_jsonlines::JsonLinesReader;

use crate::{
    reader::{self, Reader},
    sessions,
};

use super::Bookmark;

pub struct BookmarksReader<'reader> {
    session_reader: &'reader sessions::Reader<'reader>,
}

impl<'reader> BookmarksReader<'reader> {
    pub fn new(session_reader: &'reader sessions::Reader) -> Self {
        Self { session_reader }
    }

    pub fn read(&self) -> Result<Vec<Bookmark>> {
        match self
            .session_reader
            .read(&path::Path::new("session/bookmarks.jsonl").to_path_buf())
        {
            Ok(reader::Content::UTF8(content)) => {
                let iter = JsonLinesReader::new(content.as_bytes()).read_all::<Bookmark>();
                let mut bookmarks = vec![];
                for result in iter {
                    if result.is_err() {
                        continue;
                    }
                    bookmarks.push(result.unwrap());
                }
                Ok(bookmarks)
            }
            Ok(_) => Err(anyhow::anyhow!("bookmarks.jsonl is not UTF8 encoded")),
            Err(reader::Error::NotFound) => Ok(vec![]),
            Err(err) => Err(err.into()),
        }
    }
}
