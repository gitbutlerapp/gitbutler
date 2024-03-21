use std::{fmt::Display, ops::RangeInclusive, str::FromStr};

use anyhow::{anyhow, Context, Result};

use crate::git::diff;

#[derive(Debug, Eq, Clone)]
pub struct Hunk {
    pub hash: Option<String>,
    pub timestamp_ms: Option<u128>,
    pub start: u32,
    pub end: u32,
}

impl From<&diff::GitHunk> for Hunk {
    fn from(hunk: &diff::GitHunk) -> Self {
        Hunk {
            start: hunk.new_start,
            end: hunk.new_start + hunk.new_lines,
            hash: Some(Hunk::hash(&hunk.diff)),
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
                Some(raw_hash.to_string())
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
            (Some(hash), Some(timestamp_ms)) => write!(f, "-{}-{}", hash, timestamp_ms),
            (Some(hash), None) => write!(f, "-{}", hash),
            (None, Some(timestamp_ms)) => write!(f, "--{}", timestamp_ms),
            (None, None) => Ok(()),
        }
    }
}

impl Hunk {
    pub fn new(
        start: u32,
        end: u32,
        hash: Option<String>,
        timestamp_ms: Option<u128>,
    ) -> Result<Self> {
        if start > end {
            Err(anyhow!("invalid range: {}-{}", start, end))
        } else {
            Ok(Hunk {
                start,
                end,
                hash,
                timestamp_ms,
            })
        }
    }

    pub fn with_hash(&self, hash: &str) -> Self {
        Hunk {
            start: self.start,
            end: self.end,
            hash: Some(hash.to_string()),
            timestamp_ms: self.timestamp_ms,
        }
    }

    pub fn with_timestamp(&self, timestamp_ms: u128) -> Self {
        Hunk {
            start: self.start,
            end: self.end,
            hash: self.hash.clone(),
            timestamp_ms: Some(timestamp_ms),
        }
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

    pub fn hash(diff: &str) -> String {
        let addition = diff
            .lines()
            .skip(1) // skip the first line which is the diff header
            .filter(|line| line.starts_with('+') || line.starts_with('-')) // exclude context lines
            .collect::<Vec<_>>()
            .join("\n");
        format!("{:x}", md5::compute(addition))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_from_string() {
        let hunk = "1-2".parse::<Hunk>().unwrap();
        assert_eq!("1-2", hunk.to_string());
    }

    #[test]
    fn parse_invalid() {
        "3-2".parse::<Hunk>().unwrap_err();
    }

    #[test]
    fn parse_with_hash() {
        assert_eq!(
            "2-3-hash".parse::<Hunk>().unwrap(),
            Hunk::new(2, 3, Some("hash".to_string()), None).unwrap()
        );
    }

    #[test]
    fn parse_with_timestamp() {
        assert_eq!(
            "2-3--123".parse::<Hunk>().unwrap(),
            Hunk::new(2, 3, None, Some(123)).unwrap()
        );
    }

    #[test]
    fn parse_invalid_2() {
        "3-2".parse::<Hunk>().unwrap_err();
    }

    #[test]
    fn to_string_no_hash() {
        assert_eq!(
            "1-2--123",
            Hunk::new(1, 2, None, Some(123)).unwrap().to_string()
        );
    }

    #[test]
    fn test_eq() {
        for (a, b, expected) in vec![
            (
                "1-2".parse::<Hunk>().unwrap(),
                "1-2".parse::<Hunk>().unwrap(),
                true,
            ),
            (
                "1-2".parse::<Hunk>().unwrap(),
                "2-3".parse::<Hunk>().unwrap(),
                false,
            ),
            (
                "1-2-abc".parse::<Hunk>().unwrap(),
                "1-2-abc".parse::<Hunk>().unwrap(),
                true,
            ),
            (
                "1-2-abc".parse::<Hunk>().unwrap(),
                "2-3-abc".parse::<Hunk>().unwrap(),
                false,
            ),
            (
                "1-2".parse::<Hunk>().unwrap(),
                "1-2-abc".parse::<Hunk>().unwrap(),
                true,
            ),
            (
                "1-2-abc".parse::<Hunk>().unwrap(),
                "1-2".parse::<Hunk>().unwrap(),
                true,
            ),
            (
                "1-2-abc".parse::<Hunk>().unwrap(),
                "1-2-bcd".parse::<Hunk>().unwrap(),
                false,
            ),
            (
                "1-2-abc".parse::<Hunk>().unwrap(),
                "2-3-bcd".parse::<Hunk>().unwrap(),
                false,
            ),
        ] {
            assert_eq!(a == b, expected, "comapring {} and {}", a, b);
        }
    }
}
