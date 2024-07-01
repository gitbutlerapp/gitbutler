use bstr::{BStr, BString, ByteSlice, ByteVec};
use core::str;

use super::CommitHeadersV2;

pub struct CommitBuffer {
    heading: Vec<(BString, BString)>,
    message: BString,
}

impl CommitBuffer {
    pub fn new(buffer: &[u8]) -> Self {
        let buffer = BStr::new(buffer);
        if let Some((heading, message)) = buffer.split_once_str("\n\n") {
            let heading = heading
                .lines()
                .filter_map(|line| line.split_once_str(" "))
                .map(|(key, value)| (key.into(), value.into()))
                .collect();

            Self {
                heading,
                message: message.into(),
            }
        } else {
            Self {
                heading: vec![],
                message: buffer.into(),
            }
        }
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        let mut set_heading = false;
        self.heading.iter_mut().for_each(|(k, v)| {
            if k == key {
                *v = value.into();
                set_heading = true;
            }
        });

        if !set_heading {
            self.heading.push((key.into(), value.into()));
        }
    }

    /// Defers to the CommitHeadersV2 struct about which headers should be injected.
    /// If `commit_headers: None` is provided, a default set of headers, including a generated change-id will be used
    pub fn set_gitbutler_headers(&mut self, commit_headers: Option<CommitHeadersV2>) {
        if let Some(commit_headers) = commit_headers {
            commit_headers.inject_into(self)
        } else {
            CommitHeadersV2::inject_default(self)
        }
    }

    pub fn as_bstring(&self) -> BString {
        let mut output = BString::new(vec![]);

        for (key, value) in &self.heading {
            output.push_str(key);
            output.push_str(" ");
            output.push_str(value);
            output.push_str("\n");
        }

        output.push_str("\n");

        output.push_str(&self.message);

        output
    }
}

impl From<git2::Buf> for CommitBuffer {
    fn from(git2_buffer: git2::Buf) -> Self {
        Self::new(&git2_buffer)
    }
}

impl From<BString> for CommitBuffer {
    fn from(s: BString) -> Self {
        Self::new(s.as_bytes())
    }
}

impl From<CommitBuffer> for BString {
    fn from(buffer: CommitBuffer) -> BString {
        buffer.as_bstring()
    }
}
