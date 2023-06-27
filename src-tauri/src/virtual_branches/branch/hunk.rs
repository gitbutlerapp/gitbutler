use std::{fmt::Display, ops::RangeInclusive};

use anyhow::{anyhow, Context, Result};

static CONTEXT: usize = 3; // default git diff context

#[derive(Debug, Eq, Clone)]
pub struct Hunk {
    start: usize,
    end: usize,
    timestamp_ms: Option<u128>,
}

impl PartialEq for Hunk {
    fn eq(&self, other: &Self) -> bool {
        // ignore timestamp
        self.start == other.start && self.end == other.end
    }
}

impl From<RangeInclusive<usize>> for Hunk {
    fn from(range: RangeInclusive<usize>) -> Self {
        Hunk {
            start: *range.start(),
            end: *range.end(),
            timestamp_ms: None,
        }
    }
}

impl TryFrom<&str> for Hunk {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut range = s.split('-');
        let start = if let Some(raw_start) = range.next() {
            raw_start
                .parse::<usize>()
                .context(format!("failed to parse start of range: {}", s))
        } else {
            Err(anyhow!("invalid range: {}", s))
        }?;

        let end = if let Some(raw_end) = range.next() {
            raw_end
                .parse::<usize>()
                .context(format!("failed to parse end of range: {}", s))
        } else {
            Err(anyhow!("invalid range: {}", s))
        }?;
        let hunk = Hunk::new(start, end)?;

        if let Some(raw_timestamp) = range.next() {
            let timestamp = raw_timestamp
                .parse::<u128>()
                .context(format!("failed to parse timestamp: {}", s))?;
            Ok(hunk.with_timestamp(timestamp))
        } else {
            Ok(hunk)
        }
    }
}

impl Display for Hunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ts) = self.timestamp_ms {
            write!(f, "{}-{}-{}", self.start, self.end, ts)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

impl Hunk {
    pub fn new(start: usize, end: usize) -> Result<Self> {
        if start > end {
            Err(anyhow!("invalid range: {}-{}", start, end))
        } else {
            Ok(Hunk {
                start,
                end,
                timestamp_ms: None,
            })
        }
    }

    pub fn start(&self) -> &usize {
        &self.start
    }

    pub fn timestamp_ms(&self) -> Option<&u128> {
        self.timestamp_ms.as_ref()
    }

    pub fn with_timestamp(&self, timestamp_ms: u128) -> Self {
        Hunk {
            start: self.start,
            end: self.end,
            timestamp_ms: Some(timestamp_ms),
        }
    }

    pub fn contains(&self, line: &usize) -> bool {
        self.start <= *line && self.end >= *line
    }

    pub fn distance(&self, another: &Hunk) -> usize {
        if self.start > another.end {
            self.start - another.end
        } else if another.start > self.end {
            another.start - self.end
        } else {
            0
        }
    }

    pub fn touches(&self, another: &Hunk) -> bool {
        self.distance(another) <= CONTEXT * 2
    }

    pub fn intersects(&self, another: &Hunk) -> bool {
        self.contains(&another.start)
            || self.contains(&another.end)
            || another.contains(&self.start)
            || another.contains(&self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_from_string() {
        vec!["1-2", "1-2-3"].into_iter().for_each(|raw| {
            let hunk = Hunk::try_from(raw).unwrap();
            assert_eq!(raw, hunk.to_string(), "failed to convert {}", raw);
        });
    }

    #[test]
    fn parse_invalid() {
        assert!(Hunk::try_from("3-2-garbage").is_err());
    }

    #[test]
    fn parse_invalid_2() {
        assert!(Hunk::try_from("3-2").is_err());
    }
}
