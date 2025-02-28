use std::{fmt::Display, ops::RangeInclusive, str::FromStr};

use anyhow::{anyhow, Context, Result};
use bstr::ByteSlice;

use crate::diff;

pub type HunkHash = md5::Digest;

#[derive(Debug, Eq, Clone)]
pub struct Hunk {
    /// A hash over the actual lines of the hunk, including the newlines between them
    /// (i.e. the first character of the first line to the last character of the last line in the input buffer)
    pub hash: Option<HunkHash>,
    /// The index of the first line this hunk is representing.
    pub start: u32,
    /// The index of *one past* the last line this hunk is representing.
    pub end: u32,
}

impl From<&diff::GitHunk> for Hunk {
    fn from(hunk: &diff::GitHunk) -> Self {
        Hunk {
            start: hunk.new_start,
            end: hunk.new_start + hunk.new_lines,
            hash: Some(Hunk::hash_diff(&hunk.diff_lines)),
        }
    }
}

impl PartialEq for Hunk {
    fn eq(&self, other: &Self) -> bool {
        if self.hash.is_some() && other.hash.is_some() {
            self.hash == other.hash && self.start == other.start && self.end == other.end
        } else {
            self.start == other.start && self.end == other.end
        }
    }
}

impl PartialEq<diff::GitHunk> for Hunk {
    fn eq(&self, other: &diff::GitHunk) -> bool {
        self.start == other.new_start && self.end == other.new_start + other.new_lines
    }
}

impl From<RangeInclusive<u32>> for Hunk {
    fn from(range: RangeInclusive<u32>) -> Self {
        Hunk {
            start: *range.start(),
            end: *range.end(),
            hash: None,
        }
    }
}

impl FromStr for Hunk {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut range = s.split('-');
        let start = if let Some(raw_start) = range.next() {
            raw_start
                .parse::<u32>()
                .context(format!("failed to parse start of range: {}", s))
        } else {
            Err(anyhow!("invalid range: {}", s))
        }?;

        let end = if let Some(raw_end) = range.next() {
            raw_end
                .parse::<u32>()
                .context(format!("failed to parse end of range: {}", s))
        } else {
            Err(anyhow!("invalid range: {}", s))
        }?;

        let hash = if let Some(raw_hash) = range.next() {
            if raw_hash.is_empty() {
                None
            } else {
                let mut buf = [0u8; 16];
                hex::decode_to_slice(raw_hash, &mut buf)?;
                Some(md5::Digest(buf))
            }
        } else {
            None
        };

        Hunk::new(start, end, hash)
    }
}

impl Display for Hunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)?;
        match &self.hash {
            Some(hash) => write!(f, "-{:x}", hash),
            None => Ok(()),
        }
    }
}

/// Instantiation
impl Hunk {
    pub fn new(start: u32, end: u32, hash: Option<HunkHash>) -> Result<Self> {
        if start > end {
            Err(anyhow!("invalid range: {}-{}", start, end))
        } else {
            Ok(Hunk { hash, start, end })
        }
    }

    pub fn with_hash(mut self, hash: HunkHash) -> Self {
        self.hash = Some(hash);
        self
    }
}

/// Access
impl Hunk {
    fn contains_line(&self, line: u32) -> bool {
        self.start <= line && self.end >= line
    }

    pub fn intersects(&self, another: &diff::GitHunk) -> bool {
        self.contains_line(another.new_start)
            || self.contains_line(another.new_start + another.new_lines)
            || another.contains(self.start)
            || another.contains(self.end)
    }

    pub fn is_null(&self) -> bool {
        self.start == self.end && self.start == 0
    }
}

/// Hashing
impl Hunk {
    /// Produce a hash from `diff` as hex-string, which is **assumed to have a one-line diff header**!
    /// `diff` can also be entirely empty, or not contain a diff header which is when it will just be hashed
    /// with [`Self::hash()`].
    ///
    /// ### Notes on Persistence
    /// Note that there is danger in changing the hash function as this information is persisted
    /// in the virtual-branch toml file. Even if it can still be parsed or decoded,
    /// these values have to remain consistent.
    pub fn hash_diff<S: AsRef<[u8]>>(diff: S) -> HunkHash {
        let diff = diff.as_ref();
        if !diff.starts_with(b"@@") {
            return Self::hash(diff);
        }
        let mut ctx = md5::Context::new();
        diff.lines_with_terminator()
            .skip(1) // skip the first line which is the diff header.
            .for_each(|line| ctx.consume(line));
        ctx.compute()
    }

    /// Produce a hash of `input` using the same function as [`Self::hash_diff()`], but without any assumptions.
    #[inline]
    pub fn hash<S: AsRef<[u8]>>(input: S) -> HunkHash {
        md5::compute(input.as_ref())
    }
}
