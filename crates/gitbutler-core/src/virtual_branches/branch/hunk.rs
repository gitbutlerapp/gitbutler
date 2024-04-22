use std::{fmt::Display, ops::RangeInclusive, str::FromStr};

use anyhow::{anyhow, Context, Result};
use bstr::{BStr, ByteSlice};

use crate::git::diff;

pub type HunkHash = md5::Digest;

#[derive(Debug, Eq, Clone)]
pub struct Hunk {
    pub hash: Option<HunkHash>,
    pub timestamp_ms: Option<u128>,
    pub start: u32,
    pub end: u32,
}

impl From<&diff::GitHunk> for Hunk {
    fn from(hunk: &diff::GitHunk) -> Self {
        Hunk {
            start: hunk.new_start,
            end: hunk.new_start + hunk.new_lines,
            hash: Some(Hunk::hash_diff(hunk.diff_lines.as_ref())),
            timestamp_ms: None,
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

impl From<RangeInclusive<u32>> for Hunk {
    fn from(range: RangeInclusive<u32>) -> Self {
        Hunk {
            start: *range.start(),
            end: *range.end(),
            hash: None,
            timestamp_ms: None,
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

        let timestamp_ms = if let Some(raw_timestamp_ms) = range.next() {
            Some(
                raw_timestamp_ms
                    .parse::<u128>()
                    .context(format!("failed to parse timestamp_ms of range: {}", s))?,
            )
        } else {
            None
        };

        Hunk::new(start, end, hash, timestamp_ms)
    }
}

impl Display for Hunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)?;
        match (self.hash.as_ref(), self.timestamp_ms.as_ref()) {
            (Some(hash), Some(timestamp_ms)) => write!(f, "-{:x}-{}", hash, timestamp_ms),
            (Some(hash), None) => write!(f, "-{:x}", hash),
            (None, Some(timestamp_ms)) => write!(f, "--{}", timestamp_ms),
            (None, None) => Ok(()),
        }
    }
}

impl Hunk {
    pub fn new(
        start: u32,
        end: u32,
        hash: Option<HunkHash>,
        timestamp_ms: Option<u128>,
    ) -> Result<Self> {
        if start > end {
            Err(anyhow!("invalid range: {}-{}", start, end))
        } else {
            Ok(Hunk {
                hash,
                timestamp_ms,
                start,
                end,
            })
        }
    }

    pub fn with_hash(mut self, hash: HunkHash) -> Self {
        self.hash = Some(hash);
        self
    }

    pub fn with_timestamp(mut self, timestamp_ms: u128) -> Self {
        self.timestamp_ms = Some(timestamp_ms);
        self
    }

    pub fn timestam_ms(&self) -> Option<u128> {
        self.timestamp_ms
    }

    pub fn contains(&self, line: u32) -> bool {
        self.start <= line && self.end >= line
    }

    pub fn intersects(&self, another: &diff::GitHunk) -> bool {
        self.contains(another.new_start)
            || self.contains(another.new_start + another.new_lines)
            || another.contains(self.start)
            || another.contains(self.end)
    }

    pub fn shallow_eq(&self, other: &diff::GitHunk) -> bool {
        self.start == other.new_start && self.end == other.new_start + other.new_lines
    }

    /// Produce a hash from `diff` as hex-string, which is **assumed to have a one-line diff header**!
    /// `diff` can also be entirely empty, or not contain a diff header which is when it will just be hashed
    /// with [`Self::hash()`].
    ///
    /// ### Notes on Persistence
    /// Note that there is danger in changing the hash function as this information is persisted
    /// in the virtual-branch toml file. Even if it can still be parsed or decoded,
    /// these values have to remain consistent.
    pub fn hash_diff(diff: &BStr) -> HunkHash {
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
    pub fn hash(input: &[u8]) -> HunkHash {
        md5::compute(input)
    }
}
